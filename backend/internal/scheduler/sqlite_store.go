package scheduler

import (
	"database/sql"
	"encoding/json"
	"fmt"
	"time"
)

// SQLiteTaskStore implements TaskStore using SQLite for persistent task storage.
type SQLiteTaskStore struct {
	db *sql.DB
}

// NewSQLiteTaskStore creates a new SQLite-backed task store.
func NewSQLiteTaskStore(db *sql.DB) *SQLiteTaskStore {
	return &SQLiteTaskStore{db: db}
}

// List returns all tasks from the database.
func (s *SQLiteTaskStore) List() ([]*TaskDef, error) {
	rows, err := s.db.Query(
		`SELECT id, name, trigger_type, trigger_cron, trigger_interval, trigger_event,
		        actions, max_retries, retry_delay, depends_on, profile_id, enabled,
		        created_at, updated_at
		 FROM scheduler_tasks ORDER BY created_at DESC`,
	)
	if err != nil {
		return nil, fmt.Errorf("list tasks: %w", err)
	}
	defer rows.Close()

	var tasks []*TaskDef
	for rows.Next() {
		task, err := scanTask(rows)
		if err != nil {
			return nil, err
		}
		tasks = append(tasks, task)
	}
	if tasks == nil {
		tasks = []*TaskDef{}
	}
	return tasks, rows.Err()
}

// Get returns a single task by ID.
func (s *SQLiteTaskStore) Get(id string) (*TaskDef, error) {
	row := s.db.QueryRow(
		`SELECT id, name, trigger_type, trigger_cron, trigger_interval, trigger_event,
		        actions, max_retries, retry_delay, depends_on, profile_id, enabled,
		        created_at, updated_at
		 FROM scheduler_tasks WHERE id = ?`, id,
	)
	task, err := scanTask(row)
	if err == sql.ErrNoRows {
		return nil, nil
	}
	return task, err
}

// Save inserts or updates a task.
func (s *SQLiteTaskStore) Save(task *TaskDef) error {
	actionsJSON, err := json.Marshal(task.Actions)
	if err != nil {
		return fmt.Errorf("marshal actions: %w", err)
	}
	dependsOnJSON, err := json.Marshal(task.DependsOn)
	if err != nil {
		return fmt.Errorf("marshal depends_on: %w", err)
	}
	if task.CreatedAt.IsZero() {
		task.CreatedAt = time.Now()
	}
	task.UpdatedAt = time.Now()
	enabledInt := 0
	if task.Enabled {
		enabledInt = 1
	}

	_, err = s.db.Exec(
		`INSERT INTO scheduler_tasks
		 (id, name, trigger_type, trigger_cron, trigger_interval, trigger_event,
		  actions, max_retries, retry_delay, depends_on, profile_id, enabled,
		  created_at, updated_at)
		 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
		 ON CONFLICT(id) DO UPDATE SET
		  name=excluded.name, trigger_type=excluded.trigger_type,
		  trigger_cron=excluded.trigger_cron, trigger_interval=excluded.trigger_interval,
		  trigger_event=excluded.trigger_event, actions=excluded.actions,
		  max_retries=excluded.max_retries, retry_delay=excluded.retry_delay,
		  depends_on=excluded.depends_on, profile_id=excluded.profile_id,
		  enabled=excluded.enabled, updated_at=excluded.updated_at`,
		task.ID, task.Name, task.Trigger.Type, task.Trigger.Cron,
		task.Trigger.Interval, task.Trigger.Event,
		string(actionsJSON), task.MaxRetries, task.RetryDelay,
		string(dependsOnJSON), task.ProfileID, enabledInt,
		task.CreatedAt.Format(time.RFC3339), task.UpdatedAt.Format(time.RFC3339),
	)
	if err != nil {
		return fmt.Errorf("save task %s: %w", task.ID, err)
	}
	return nil
}

// Delete removes a task by ID.
func (s *SQLiteTaskStore) Delete(id string) error {
	_, err := s.db.Exec(`DELETE FROM scheduler_tasks WHERE id = ?`, id)
	return err
}

// ─── Scanner ──────────────────────────────────────────────────────────────────

type taskScanner interface {
	Scan(dest ...any) error
}

func scanTask(s taskScanner) (*TaskDef, error) {
	var (
		id, name, triggerType, triggerCron, triggerInterval, triggerEvent string
		actionsJSON, dependsOnJSON                                        string
		maxRetries                                                        int
		retryDelay, profileID                                             string
		enabledInt                                                        int
		createdAtStr, updatedAtStr                                        string
	)

	if err := s.Scan(
		&id, &name, &triggerType, &triggerCron, &triggerInterval, &triggerEvent,
		&actionsJSON, &maxRetries, &retryDelay, &dependsOnJSON, &profileID, &enabledInt,
		&createdAtStr, &updatedAtStr,
	); err != nil {
		return nil, fmt.Errorf("scan task: %w", err)
	}

	var actions []TaskAction
	if actionsJSON != "" && actionsJSON != "[]" {
		_ = json.Unmarshal([]byte(actionsJSON), &actions)
	}
	if actions == nil {
		actions = []TaskAction{}
	}

	var dependsOn []string
	if dependsOnJSON != "" && dependsOnJSON != "[]" {
		_ = json.Unmarshal([]byte(dependsOnJSON), &dependsOn)
	}
	if dependsOn == nil {
		dependsOn = []string{}
	}

	createdAt, _ := time.Parse(time.RFC3339, createdAtStr)
	updatedAt, _ := time.Parse(time.RFC3339, updatedAtStr)

	return &TaskDef{
		ID:   id,
		Name: name,
		Trigger: TaskTrigger{
			Type:     TriggerType(triggerType),
			Cron:     triggerCron,
			Interval: triggerInterval,
			Event:    triggerEvent,
		},
		Actions:    actions,
		MaxRetries: maxRetries,
		RetryDelay: retryDelay,
		DependsOn:  dependsOn,
		ProfileID:  profileID,
		Enabled:    enabledInt != 0,
		CreatedAt:  createdAt,
		UpdatedAt:  updatedAt,
	}, nil
}
