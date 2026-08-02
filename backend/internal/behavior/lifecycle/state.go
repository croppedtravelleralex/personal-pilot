package lifecycle

import (
	"database/sql"
	"encoding/json"
	"time"
)

type Store struct {
	db *sql.DB
}

func NewStore(db *sql.DB) (*Store, error) {
	store := &Store{db: db}
	return store, store.ensureSchema()
}

func (s *Store) ensureSchema() error {
	_, err := s.db.Exec(`CREATE TABLE IF NOT EXISTS behavior_lifecycle_state (
		profile_id TEXT PRIMARY KEY,
		created_at DATETIME NOT NULL,
		last_active_at DATETIME NOT NULL,
		session_count INTEGER NOT NULL DEFAULT 0,
		interest_vector TEXT NOT NULL DEFAULT '{}',
		social_stage TEXT NOT NULL DEFAULT 'only-read',
		risk_score REAL NOT NULL DEFAULT 0,
		exit_region TEXT,
		preferred_locale TEXT
	)`)
	return err
}

func (s *Store) Save(state State) error {
	if state.CreatedAt.IsZero() {
		state.CreatedAt = time.Now().UTC()
	}
	if state.LastActiveAt.IsZero() {
		state.LastActiveAt = state.CreatedAt
	}
	if state.SocialStage == "" {
		state.SocialStage = "only-read"
	}
	rawInterest, err := json.Marshal(state.InterestVector)
	if err != nil {
		return err
	}
	_, err = s.db.Exec(`INSERT INTO behavior_lifecycle_state
		(profile_id, created_at, last_active_at, session_count, interest_vector, social_stage, risk_score, exit_region, preferred_locale)
		VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
		ON CONFLICT(profile_id) DO UPDATE SET
		last_active_at=excluded.last_active_at,
		session_count=excluded.session_count,
		interest_vector=excluded.interest_vector,
		social_stage=excluded.social_stage,
		risk_score=excluded.risk_score,
		exit_region=excluded.exit_region,
		preferred_locale=excluded.preferred_locale`,
		state.ProfileID, state.CreatedAt, state.LastActiveAt, state.SessionCount, string(rawInterest), state.SocialStage, state.RiskScore, state.ExitRegion, state.PreferredLocale)
	return err
}

func (s *Store) Load(profileID string) (State, error) {
	row := s.db.QueryRow(`SELECT profile_id, created_at, last_active_at, session_count, interest_vector, social_stage, risk_score, exit_region, preferred_locale
		FROM behavior_lifecycle_state WHERE profile_id = ?`, profileID)
	var state State
	var rawInterest string
	if err := row.Scan(&state.ProfileID, &state.CreatedAt, &state.LastActiveAt, &state.SessionCount, &rawInterest, &state.SocialStage, &state.RiskScore, &state.ExitRegion, &state.PreferredLocale); err != nil {
		return State{}, err
	}
	if rawInterest != "" {
		_ = json.Unmarshal([]byte(rawInterest), &state.InterestVector)
	}
	if state.InterestVector == nil {
		state.InterestVector = map[string]float64{}
	}
	return state, nil
}

func (s *Store) IncrementSession(profileID string, at time.Time) error {
	_, err := s.db.Exec(`UPDATE behavior_lifecycle_state SET session_count = session_count + 1, last_active_at = ? WHERE profile_id = ?`, at, profileID)
	return err
}
