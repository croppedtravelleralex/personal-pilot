package events

import (
	"database/sql"
	"encoding/json"
	"fmt"
	"strings"
	"time"
)

// EventLogEntry is a persisted event record.
type EventLogEntry struct {
	ID        int64                  `json:"id"`
	EventName string                 `json:"eventName"`
	Namespace string                 `json:"namespace"`
	Severity  string                 `json:"severity"`
	Payload   map[string]interface{} `json:"payload"`
	CreatedAt string                 `json:"createdAt"`
}

// EventLogQuery represents query filters for event logs.
type EventLogQuery struct {
	After     string // RFC3339, optional
	Before    string // RFC3339, optional
	Namespace string // optional
	Severity  string // optional
	EventName string // optional (exact match)
	Limit     int    // default 50
	Offset    int    // default 0
}

// EventLogStore provides persistence for events.
type EventLogStore interface {
	Insert(entry *EventLogEntry) error
	Query(q EventLogQuery) ([]*EventLogEntry, error)
	Count(q EventLogQuery) (int, error)
	Prune(before time.Time) (int64, error)
}

// SQLiteEventLogStore implements EventLogStore using SQLite.
type SQLiteEventLogStore struct {
	db *sql.DB
}

// NewSQLiteEventLogStore creates a new SQLite-based event log store.
func NewSQLiteEventLogStore(db *sql.DB) *SQLiteEventLogStore {
	return &SQLiteEventLogStore{db: db}
}

// Insert writes a single event log entry.
func (s *SQLiteEventLogStore) Insert(entry *EventLogEntry) error {
	payloadJSON := "{}"
	if entry.Payload != nil {
		b, err := json.Marshal(entry.Payload)
		if err == nil {
			payloadJSON = string(b)
		}
	}
	_, err := s.db.Exec(
		`INSERT INTO event_log (event_name, namespace, severity, payload, created_at) VALUES (?, ?, ?, ?, ?)`,
		entry.EventName, entry.Namespace, entry.Severity, payloadJSON, entry.CreatedAt,
	)
	return err
}

// Query returns matching event log entries.
func (s *SQLiteEventLogStore) Query(q EventLogQuery) ([]*EventLogEntry, error) {
	where := []string{"1=1"}
	args := []interface{}{}

	if q.After != "" {
		where = append(where, "created_at > ?")
		args = append(args, q.After)
	}
	if q.Before != "" {
		where = append(where, "created_at < ?")
		args = append(args, q.Before)
	}
	if q.Namespace != "" {
		where = append(where, "namespace = ?")
		args = append(args, q.Namespace)
	}
	if q.Severity != "" {
		where = append(where, "severity = ?")
		args = append(args, q.Severity)
	}
	if q.EventName != "" {
		where = append(where, "event_name = ?")
		args = append(args, q.EventName)
	}

	limit := q.Limit
	if limit <= 0 {
		limit = 50
	}
	offset := q.Offset
	if offset < 0 {
		offset = 0
	}

	rows, err := s.db.Query(
		fmt.Sprintf(
			`SELECT id, event_name, namespace, severity, payload, created_at FROM event_log WHERE %s ORDER BY id DESC LIMIT ? OFFSET ?`,
			strings.Join(where, " AND "),
		),
		append(args, limit, offset)...,
	)
	if err != nil {
		return nil, fmt.Errorf("query event log: %w", err)
	}
	defer rows.Close()

	var entries []*EventLogEntry
	for rows.Next() {
		var e EventLogEntry
		var payloadStr string
		if err := rows.Scan(&e.ID, &e.EventName, &e.Namespace, &e.Severity, &payloadStr, &e.CreatedAt); err != nil {
			return nil, fmt.Errorf("scan event log: %w", err)
		}
		if payloadStr != "" && payloadStr != "{}" {
			var m map[string]interface{}
			if err := json.Unmarshal([]byte(payloadStr), &m); err == nil {
				e.Payload = m
			}
		}
		if e.Payload == nil {
			e.Payload = map[string]interface{}{}
		}
		entries = append(entries, &e)
	}
	if entries == nil {
		entries = []*EventLogEntry{}
	}
	return entries, rows.Err()
}

// Count returns the number of matching event log entries.
func (s *SQLiteEventLogStore) Count(q EventLogQuery) (int, error) {
	where := []string{"1=1"}
	args := []interface{}{}

	if q.After != "" {
		where = append(where, "created_at > ?")
		args = append(args, q.After)
	}
	if q.Before != "" {
		where = append(where, "created_at < ?")
		args = append(args, q.Before)
	}
	if q.Namespace != "" {
		where = append(where, "namespace = ?")
		args = append(args, q.Namespace)
	}
	if q.Severity != "" {
		where = append(where, "severity = ?")
		args = append(args, q.Severity)
	}
	if q.EventName != "" {
		where = append(where, "event_name = ?")
		args = append(args, q.EventName)
	}

	var count int
	err := s.db.QueryRow(
		fmt.Sprintf(`SELECT COUNT(*) FROM event_log WHERE %s`, strings.Join(where, " AND ")),
		args...,
	).Scan(&count)
	return count, err
}

// Prune removes entries older than the given time. Returns number deleted.
func (s *SQLiteEventLogStore) Prune(before time.Time) (int64, error) {
	result, err := s.db.Exec(`DELETE FROM event_log WHERE created_at < ?`, before.Format(time.RFC3339))
	if err != nil {
		return 0, err
	}
	return result.RowsAffected()
}
