package automation

import (
	"sync"
	"time"
)

// Action types for auto-response rules.
const (
	ActionEmitEvent = "emit_event"
	ActionRunTask   = "run_task"
	ActionNotify    = "notify"
)

// AutoRule defines an automatic response rule triggered by events.
type AutoRule struct {
	ID           string                 `json:"id"`
	Name         string                 `json:"name"`
	TriggerEvent string                 `json:"triggerEvent"`
	Condition    string                 `json:"condition,omitempty"`
	Action       string                 `json:"action"`
	ActionParams map[string]interface{} `json:"actionParams,omitempty"`
	Cooldown     string                 `json:"cooldown"`
	Enabled      bool                   `json:"enabled"`
	CreatedAt    string                 `json:"createdAt"`
	UpdatedAt    string                 `json:"updatedAt"`

	// Runtime state (not persisted)
	lastTriggered time.Time
	mu            sync.Mutex
}

// CanTrigger returns true if the cooldown period has elapsed since the last trigger.
func (r *AutoRule) CanTrigger() bool {
	if r.Cooldown == "" {
		return true
	}
	d, err := time.ParseDuration(r.Cooldown)
	if err != nil {
		return true // invalid cooldown → always allow
	}
	r.mu.Lock()
	defer r.mu.Unlock()
	return time.Since(r.lastTriggered) >= d
}

// MarkTriggered records the current time as the last trigger time.
func (r *AutoRule) MarkTriggered() {
	r.mu.Lock()
	defer r.mu.Unlock()
	r.lastTriggered = time.Now()
}

// RuleStore provides persistence for automation rules.
type RuleStore interface {
	List() ([]*AutoRule, error)
	Get(id string) (*AutoRule, error)
	Save(rule *AutoRule) error
	Delete(id string) error
}
