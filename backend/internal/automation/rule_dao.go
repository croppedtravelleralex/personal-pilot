package automation

import (
	"database/sql"
	"encoding/json"
	"fmt"
	"time"
)

// SQLiteRuleStore implements RuleStore using SQLite.
type SQLiteRuleStore struct {
	db *sql.DB
}

// NewSQLiteRuleStore creates a new SQLite-backed rule store.
func NewSQLiteRuleStore(db *sql.DB) *SQLiteRuleStore {
	return &SQLiteRuleStore{db: db}
}

// List returns all rules from the database.
func (s *SQLiteRuleStore) List() ([]*AutoRule, error) {
	rows, err := s.db.Query(
		`SELECT id, name, trigger_event, condition, action, action_params,
		        cooldown, enabled, created_at, updated_at
		 FROM automation_rules ORDER BY created_at DESC`,
	)
	if err != nil {
		return nil, fmt.Errorf("list rules: %w", err)
	}
	defer rows.Close()

	var rules []*AutoRule
	for rows.Next() {
		r, err := scanRule(rows)
		if err != nil {
			return nil, err
		}
		rules = append(rules, r)
	}
	if rules == nil {
		rules = []*AutoRule{}
	}
	return rules, rows.Err()
}

// Get returns a single rule by ID.
func (s *SQLiteRuleStore) Get(id string) (*AutoRule, error) {
	row := s.db.QueryRow(
		`SELECT id, name, trigger_event, condition, action, action_params,
		        cooldown, enabled, created_at, updated_at
		 FROM automation_rules WHERE id = ?`, id,
	)
	r, err := scanRule(row)
	if err == sql.ErrNoRows {
		return nil, nil
	}
	return r, err
}

// Save inserts or updates a rule.
func (s *SQLiteRuleStore) Save(rule *AutoRule) error {
	paramsJSON, err := json.Marshal(rule.ActionParams)
	if err != nil {
		return fmt.Errorf("marshal action_params: %w", err)
	}
	if paramsJSON == nil {
		paramsJSON = []byte("{}")
	}
	if rule.CreatedAt == "" {
		rule.CreatedAt = time.Now().Format(time.RFC3339)
	}
	rule.UpdatedAt = time.Now().Format(time.RFC3339)
	enabledInt := 0
	if rule.Enabled {
		enabledInt = 1
	}

	_, err = s.db.Exec(
		`INSERT INTO automation_rules
		 (id, name, trigger_event, condition, action, action_params,
		  cooldown, enabled, created_at, updated_at)
		 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
		 ON CONFLICT(id) DO UPDATE SET
		  name=excluded.name, trigger_event=excluded.trigger_event,
		  condition=excluded.condition, action=excluded.action,
		  action_params=excluded.action_params, cooldown=excluded.cooldown,
		  enabled=excluded.enabled, updated_at=excluded.updated_at`,
		rule.ID, rule.Name, rule.TriggerEvent, rule.Condition,
		rule.Action, string(paramsJSON), rule.Cooldown,
		enabledInt, rule.CreatedAt, rule.UpdatedAt,
	)
	if err != nil {
		return fmt.Errorf("save rule %s: %w", rule.ID, err)
	}
	return nil
}

// Delete removes a rule by ID.
func (s *SQLiteRuleStore) Delete(id string) error {
	_, err := s.db.Exec(`DELETE FROM automation_rules WHERE id = ?`, id)
	return err
}

// ─── Scanner ──────────────────────────────────────────────────────────────────

type ruleScanner interface {
	Scan(dest ...any) error
}

func scanRule(s ruleScanner) (*AutoRule, error) {
	var (
		id, name, triggerEvent, condition, action, paramsJSON string
		cooldown                                              string
		enabledInt                                            int
		createdAt, updatedAt                                  string
	)

	if err := s.Scan(
		&id, &name, &triggerEvent, &condition, &action, &paramsJSON,
		&cooldown, &enabledInt, &createdAt, &updatedAt,
	); err != nil {
		return nil, fmt.Errorf("scan rule: %w", err)
	}

	var actionParams map[string]interface{}
	if paramsJSON != "" && paramsJSON != "{}" {
		_ = json.Unmarshal([]byte(paramsJSON), &actionParams)
	}
	if actionParams == nil {
		actionParams = make(map[string]interface{})
	}

	return &AutoRule{
		ID:           id,
		Name:         name,
		TriggerEvent: triggerEvent,
		Condition:    condition,
		Action:       action,
		ActionParams: actionParams,
		Cooldown:     cooldown,
		Enabled:      enabledInt != 0,
		CreatedAt:    createdAt,
		UpdatedAt:    updatedAt,
	}, nil
}
