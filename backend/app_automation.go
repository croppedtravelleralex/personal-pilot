package backend

import (
	"fmt"
	"personal-pilot/backend/internal/automation"
	"time"
)

// ─── Wails-bound types ─────────────────────────────────────────────────────────

// AutomationRuleInfo is the frontend-facing rule info.
type AutomationRuleInfo struct {
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
}

// AutomationRuleInput is the frontend-facing rule creation/update input.
type AutomationRuleInput struct {
	Name         string                 `json:"name"`
	TriggerEvent string                 `json:"triggerEvent"`
	Condition    string                 `json:"condition,omitempty"`
	Action       string                 `json:"action"`
	ActionParams map[string]interface{} `json:"actionParams,omitempty"`
	Cooldown     string                 `json:"cooldown"`
	Enabled      bool                   `json:"enabled"`
}

// ─── Wails-bound methods ───────────────────────────────────────────────────────

// AutomationRuleList returns all automation rules.
func (a *App) AutomationRuleList() []AutomationRuleInfo {
	if a.ruleStore == nil {
		return []AutomationRuleInfo{}
	}
	rules, err := a.ruleStore.List()
	if err != nil {
		return []AutomationRuleInfo{}
	}
	out := make([]AutomationRuleInfo, 0, len(rules))
	for _, r := range rules {
		out = append(out, ruleToInfo(r))
	}
	return out
}

// AutomationRuleCreate creates a new automation rule.
func (a *App) AutomationRuleCreate(input AutomationRuleInput) (*AutomationRuleInfo, error) {
	if a.ruleStore == nil {
		return nil, fmt.Errorf("automation rule store not initialized")
	}
	if input.Name == "" {
		return nil, fmt.Errorf("rule name is required")
	}
	if input.TriggerEvent == "" {
		return nil, fmt.Errorf("trigger event is required")
	}
	if input.Action == "" {
		return nil, fmt.Errorf("action is required")
	}

	rule := &automation.AutoRule{
		ID:           fmt.Sprintf("rule-%d", time.Now().UnixNano()),
		Name:         input.Name,
		TriggerEvent: input.TriggerEvent,
		Condition:    input.Condition,
		Action:       input.Action,
		ActionParams: input.ActionParams,
		Cooldown:     input.Cooldown,
		Enabled:      input.Enabled,
		CreatedAt:    time.Now().Format(time.RFC3339),
	}

	if rule.ActionParams == nil {
		rule.ActionParams = make(map[string]interface{})
	}
	if rule.Cooldown == "" {
		rule.Cooldown = "5m"
	}

	if err := a.ruleStore.Save(rule); err != nil {
		return nil, fmt.Errorf("failed to create rule: %w", err)
	}

	info := ruleToInfo(rule)
	return &info, nil
}

// AutomationRuleUpdate updates an existing automation rule.
func (a *App) AutomationRuleUpdate(id string, input AutomationRuleInput) (*AutomationRuleInfo, error) {
	if a.ruleStore == nil {
		return nil, fmt.Errorf("automation rule store not initialized")
	}

	existing, err := a.ruleStore.Get(id)
	if err != nil || existing == nil {
		return nil, fmt.Errorf("rule not found: %s", id)
	}

	existing.Name = input.Name
	existing.TriggerEvent = input.TriggerEvent
	existing.Condition = input.Condition
	existing.Action = input.Action
	existing.ActionParams = input.ActionParams
	existing.Cooldown = input.Cooldown
	existing.Enabled = input.Enabled

	if existing.ActionParams == nil {
		existing.ActionParams = make(map[string]interface{})
	}
	if existing.Cooldown == "" {
		existing.Cooldown = "5m"
	}

	if err := a.ruleStore.Save(existing); err != nil {
		return nil, fmt.Errorf("failed to update rule: %w", err)
	}

	info := ruleToInfo(existing)
	return &info, nil
}

// AutomationRuleDelete removes a rule by ID.
func (a *App) AutomationRuleDelete(id string) error {
	if a.ruleStore == nil {
		return fmt.Errorf("automation rule store not initialized")
	}
	return a.ruleStore.Delete(id)
}

// AutomationRuleToggle enables or disables a rule.
func (a *App) AutomationRuleToggle(id string, enabled bool) error {
	if a.ruleStore == nil {
		return fmt.Errorf("automation rule store not initialized")
	}

	rule, err := a.ruleStore.Get(id)
	if err != nil || rule == nil {
		return fmt.Errorf("rule not found: %s", id)
	}

	rule.Enabled = enabled
	return a.ruleStore.Save(rule)
}

// AutomationRuleTestFire manually triggers a rule's action with a test payload.
func (a *App) AutomationRuleTestFire(id string) error {
	if a.ruleStore == nil {
		return fmt.Errorf("automation rule store not initialized")
	}
	if a.ruleEngine == nil {
		return fmt.Errorf("automation rule engine not initialized")
	}

	rule, err := a.ruleStore.Get(id)
	if err != nil || rule == nil {
		return fmt.Errorf("rule not found: %s", id)
	}

	// Evaluate the rule with an empty test payload (condition must pass)
	a.ruleEngine.Evaluate(rule.TriggerEvent, map[string]interface{}{
		"_testFire": true,
	})
	return nil
}

// ─── Helpers ───────────────────────────────────────────────────────────────────

func ruleToInfo(r *automation.AutoRule) AutomationRuleInfo {
	params := r.ActionParams
	if params == nil {
		params = make(map[string]interface{})
	}
	return AutomationRuleInfo{
		ID:           r.ID,
		Name:         r.Name,
		TriggerEvent: r.TriggerEvent,
		Condition:    r.Condition,
		Action:       r.Action,
		ActionParams: params,
		Cooldown:     r.Cooldown,
		Enabled:      r.Enabled,
		CreatedAt:    r.CreatedAt,
		UpdatedAt:    r.UpdatedAt,
	}
}
