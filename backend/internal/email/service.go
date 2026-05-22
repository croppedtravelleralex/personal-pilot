package email

import (
	"context"
	"database/sql"
	"fmt"
	"sync"
	"time"

	"personal-pilot/backend/internal/logger"
)

type Session struct {
	ID           string
	Email        string
	Password     string
	Provider     string
	Status       string // active / released / expired
	MessageCount int
	CreatedAt    time.Time
	ExpiresAt    time.Time
}

type SessionStore struct {
	db *sql.DB
	mu sync.Mutex
}

func NewSessionStore(db *sql.DB) *SessionStore {
	s := &SessionStore{db: db}
	s.ensureTable()
	return s
}

func (s *SessionStore) ensureTable() {
	q := `CREATE TABLE IF NOT EXISTS email_sessions (
		id TEXT PRIMARY KEY,
		email TEXT NOT NULL,
		password TEXT,
		provider TEXT NOT NULL,
		status TEXT NOT NULL DEFAULT 'active',
		message_count INTEGER DEFAULT 0,
		profile_id TEXT,
		task_id TEXT,
		created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
		expires_at DATETIME,
		released_at DATETIME
	)`
	s.db.Exec(q)
}

func (s *SessionStore) Save(session *Session) error {
	s.mu.Lock()
	defer s.mu.Unlock()
	q := `INSERT OR REPLACE INTO email_sessions (id, email, password, provider, status, message_count, created_at, expires_at)
		  VALUES (?, ?, ?, ?, ?, ?, ?, ?)`
	_, err := s.db.Exec(q, session.ID, session.Email, session.Password, session.Provider,
		session.Status, session.MessageCount, session.CreatedAt, session.ExpiresAt)
	return err
}

func (s *SessionStore) Get(id string) (*Session, error) {
	q := `SELECT id, email, password, provider, status, message_count, created_at, expires_at
		  FROM email_sessions WHERE id = ?`
	row := s.db.QueryRow(q, id)
	sess := &Session{}
	var expiresAt sql.NullTime
	err := row.Scan(&sess.ID, &sess.Email, &sess.Password, &sess.Provider,
		&sess.Status, &sess.MessageCount, &sess.CreatedAt, &expiresAt)
	if err != nil {
		return nil, err
	}
	if expiresAt.Valid {
		sess.ExpiresAt = expiresAt.Time
	}
	return sess, nil
}

func (s *SessionStore) List() ([]*Session, error) {
	q := `SELECT id, email, password, provider, status, message_count, created_at, expires_at
		  FROM email_sessions WHERE status = 'active' ORDER BY created_at DESC`
	rows, err := s.db.Query(q)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	var sessions []*Session
	for rows.Next() {
		sess := &Session{}
		var expiresAt sql.NullTime
		if err := rows.Scan(&sess.ID, &sess.Email, &sess.Password, &sess.Provider,
			&sess.Status, &sess.MessageCount, &sess.CreatedAt, &expiresAt); err != nil {
			return nil, err
		}
		if expiresAt.Valid {
			sess.ExpiresAt = expiresAt.Time
		}
		sessions = append(sessions, sess)
	}
	return sessions, nil
}

func (s *SessionStore) UpdateStatus(id, status string) error {
	q := `UPDATE email_sessions SET status = ?, released_at = CURRENT_TIMESTAMP WHERE id = ?`
	_, err := s.db.Exec(q, status, id)
	return err
}

type EmailService struct {
	primary   Provider
	fallback  Provider
	store     *SessionStore
	credStore *CredStore
}

func NewEmailService(primary, fallback Provider, db *sql.DB, credStore *CredStore) *EmailService {
	return &EmailService{
		primary:   primary,
		fallback:  fallback,
		store:     NewSessionStore(db),
		credStore: credStore,
	}
}

func (s *EmailService) CreateInbox(ctx context.Context) (*Session, error) {
	log := logger.New("EmailService")

	addr, err := s.primary.CreateAddress(ctx)
	if err != nil {
		log.Warn("primary provider failed, trying fallback", logger.F("error", err.Error()))
		addr, err = s.fallback.CreateAddress(ctx)
		if err != nil {
			return nil, fmt.Errorf("all email providers exhausted: %w", err)
		}
	}
	if addr == nil || addr.Email == "" {
		return nil, fmt.Errorf("email provider returned no active address")
	}

	session := &Session{
		ID:        fmt.Sprintf("inbox-%x", time.Now().UnixNano()),
		Email:     addr.Email,
		Password:  addr.Password,
		Provider:  addr.Provider,
		Status:    "active",
		CreatedAt: time.Now(),
		ExpiresAt: addr.ExpiresAt,
	}

	if err := s.store.Save(session); err != nil {
		log.Error("failed to persist email session", logger.F("error", err.Error()))
	}

	return session, nil
}

func (s *EmailService) WaitForCode(ctx context.Context, sessionID string, filter *MailFilter, timeout time.Duration) (string, error) {
	sess, err := s.store.Get(sessionID)
	if err != nil {
		return "", fmt.Errorf("session not found: %w", err)
	}

	var provider Provider
	if sess.Provider == s.primary.Name() {
		provider = s.primary
	} else {
		provider = s.fallback
	}

	return provider.WaitForCode(ctx, filter, timeout)
}

func (s *EmailService) ListInboxes(ctx context.Context) ([]*Session, error) {
	return s.store.List()
}

func (s *EmailService) ReleaseInbox(ctx context.Context, sessionID string) error {
	return s.store.UpdateStatus(sessionID, "released")
}

func (s *EmailService) GetStore() *SessionStore {
	return s.store
}
