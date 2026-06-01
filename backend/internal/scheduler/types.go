package scheduler

import (
	"sync"
	"time"
)

// TriggerType defines how a task is triggered.
type TriggerType string

const (
	TriggerCron     TriggerType = "cron"
	TriggerInterval TriggerType = "interval"
	TriggerEvent    TriggerType = "event"
)

// TaskStatus represents the current state of a task.
type TaskStatus string

const (
	StatusIdle    TaskStatus = "idle"
	StatusRunning TaskStatus = "running"
	StatusPaused  TaskStatus = "paused"
	StatusFailed  TaskStatus = "failed"
	StatusDone    TaskStatus = "done"
)

// TaskTrigger defines when a task executes.
type TaskTrigger struct {
	Type     TriggerType `json:"type"`
	Cron     string      `json:"cron,omitempty"`     // simple cron: "*/5m", "0 */1h", "daily@09:00"
	Interval string      `json:"interval,omitempty"` // "5m", "1h", "30s"
	Event    string      `json:"event,omitempty"`    // event name e.g. "risk:node:banned"
}

// TaskAction defines a single step in a task.
type TaskAction struct {
	Type    string `json:"type"`    // "cdp", "navigate", "click", "wait", "extract"
	Target  string `json:"target"`  // CSS selector, URL, or CDP method
	Value   string `json:"value"`   // additional parameter
	Timeout int    `json:"timeout"` // milliseconds, 0 = default 30s
}

// TaskDef is a complete task definition.
type TaskDef struct {
	ID         string       `json:"id"`
	Name       string       `json:"name"`
	Trigger    TaskTrigger  `json:"trigger"`
	Actions    []TaskAction `json:"actions"`
	MaxRetries int          `json:"maxRetries"`
	RetryDelay string       `json:"retryDelay"` // "5s", "1m"
	DependsOn  []string     `json:"dependsOn,omitempty"`
	ProfileID  string       `json:"profileId,omitempty"`
	Enabled    bool         `json:"enabled"`
	CreatedAt  time.Time    `json:"createdAt"`
	UpdatedAt  time.Time    `json:"updatedAt"`

	// Runtime state (not persisted)
	status     TaskStatus
	lastRunAt  time.Time
	lastError  string
	retryCount int
	mu         sync.Mutex
}

// Status returns the current task status.
func (t *TaskDef) Status() TaskStatus {
	t.mu.Lock()
	defer t.mu.Unlock()
	if t.status == "" {
		return StatusIdle
	}
	return t.status
}

// SetStatus updates the task status.
func (t *TaskDef) SetStatus(s TaskStatus) {
	t.mu.Lock()
	defer t.mu.Unlock()
	t.status = s
}

// LastRunAt returns the last run time.
func (t *TaskDef) LastRunAt() time.Time {
	t.mu.Lock()
	defer t.mu.Unlock()
	return t.lastRunAt
}

// LastError returns the last execution error.
func (t *TaskDef) LastError() string {
	t.mu.Lock()
	defer t.mu.Unlock()
	return t.lastError
}

// RetryCount returns the current retry attempt count.
func (t *TaskDef) RetryCount() int {
	t.mu.Lock()
	defer t.mu.Unlock()
	return t.retryCount
}

// SetRuntimeState restores persisted runtime state.
func (t *TaskDef) SetRuntimeState(status TaskStatus, lastRunAt time.Time, lastError string, retryCount int) {
	t.mu.Lock()
	defer t.mu.Unlock()
	t.status = status
	t.lastRunAt = lastRunAt
	t.lastError = lastError
	t.retryCount = retryCount
}

// ClearLastError marks the latest execution as clean.
func (t *TaskDef) ClearLastError() {
	t.mu.Lock()
	defer t.mu.Unlock()
	t.lastError = ""
	t.retryCount = 0
}

// TaskStore provides persistence for task definitions.
type TaskStore interface {
	List() ([]*TaskDef, error)
	Get(id string) (*TaskDef, error)
	Save(task *TaskDef) error
	Delete(id string) error
}

// MemoryStore is an in-memory task store.
type MemoryStore struct {
	mu    sync.RWMutex
	tasks map[string]*TaskDef
}

// NewMemoryStore creates a new in-memory task store.
func NewMemoryStore() *MemoryStore {
	return &MemoryStore{tasks: make(map[string]*TaskDef)}
}

func (s *MemoryStore) List() ([]*TaskDef, error) {
	s.mu.RLock()
	defer s.mu.RUnlock()
	out := make([]*TaskDef, 0, len(s.tasks))
	for _, t := range s.tasks {
		out = append(out, t)
	}
	return out, nil
}

func (s *MemoryStore) Get(id string) (*TaskDef, error) {
	s.mu.RLock()
	defer s.mu.RUnlock()
	t, ok := s.tasks[id]
	if !ok {
		return nil, nil
	}
	return t, nil
}

func (s *MemoryStore) Save(task *TaskDef) error {
	s.mu.Lock()
	defer s.mu.Unlock()
	task.UpdatedAt = time.Now()
	s.tasks[task.ID] = task
	return nil
}

func (s *MemoryStore) Delete(id string) error {
	s.mu.Lock()
	defer s.mu.Unlock()
	delete(s.tasks, id)
	return nil
}

// ParseInterval parses a duration or cron-like string into a time.Duration.
// Supported formats: "30s", "5m", "1h", or "" for none.
func ParseInterval(s string) (time.Duration, error) {
	if s == "" {
		return 0, nil
	}
	return time.ParseDuration(s)
}
