package backend

import (
	"fmt"
	"personal-pilot/backend/internal/events"
	"personal-pilot/backend/internal/scheduler"
	"time"
)

// ─── Wails-bound types ─────────────────────────────────────────────────────────

// SchedulerTaskInfo is the frontend-facing task info.
type SchedulerTaskInfo struct {
	ID         string                `json:"id"`
	Name       string                `json:"name"`
	Trigger    SchedulerTaskTrigger  `json:"trigger"`
	Actions    []SchedulerTaskAction `json:"actions"`
	MaxRetries int                   `json:"maxRetries"`
	RetryDelay string                `json:"retryDelay"`
	DependsOn  []string              `json:"dependsOn"`
	ProfileID  string                `json:"profileId"`
	Enabled    bool                  `json:"enabled"`
	CreatedAt  string                `json:"createdAt"`
	Status     string                `json:"status"`
	LastRunAt  string                `json:"lastRunAt"`
	LastError  string                `json:"lastError"`
	RetryCount int                   `json:"retryCount"`
}

// SchedulerTaskTrigger is the frontend-facing trigger.
type SchedulerTaskTrigger struct {
	Type     string `json:"type"`
	Cron     string `json:"cron,omitempty"`
	Interval string `json:"interval,omitempty"`
	Event    string `json:"event,omitempty"`
}

// SchedulerTaskAction is the frontend-facing action.
type SchedulerTaskAction struct {
	Type    string `json:"type"`
	Target  string `json:"target"`
	Value   string `json:"value"`
	Timeout int    `json:"timeout"`
}

// SchedulerTaskInput is the frontend-facing task creation input.
type SchedulerTaskInput struct {
	Name       string                `json:"name"`
	Trigger    SchedulerTaskTrigger  `json:"trigger"`
	Actions    []SchedulerTaskAction `json:"actions"`
	MaxRetries int                   `json:"maxRetries"`
	RetryDelay string                `json:"retryDelay"`
	DependsOn  []string              `json:"dependsOn"`
	ProfileID  string                `json:"profileId"`
	Enabled    bool                  `json:"enabled"`
}

// ─── Wails-bound methods ───────────────────────────────────────────────────────

// SchedulerListTasks returns all registered tasks.
func (a *App) SchedulerListTasks() []SchedulerTaskInfo {
	if a.scheduler == nil {
		return []SchedulerTaskInfo{}
	}
	tasks, err := a.scheduler.ListTasks()
	if err != nil {
		return []SchedulerTaskInfo{}
	}
	out := make([]SchedulerTaskInfo, 0, len(tasks))
	for _, t := range tasks {
		out = append(out, taskToInfo(t))
	}
	return out
}

// SchedulerAddTask creates a new task from frontend input.
func (a *App) SchedulerAddTask(input SchedulerTaskInput) (*SchedulerTaskInfo, error) {
	if a.scheduler == nil {
		return nil, fmt.Errorf("scheduler not initialized")
	}
	if input.Name == "" {
		return nil, fmt.Errorf("task name is required")
	}
	if input.Trigger.Type == "" {
		return nil, fmt.Errorf("trigger type is required")
	}

	task := &scheduler.TaskDef{
		ID:   fmt.Sprintf("task-%d", time.Now().UnixNano()),
		Name: input.Name,
		Trigger: scheduler.TaskTrigger{
			Type:     scheduler.TriggerType(input.Trigger.Type),
			Cron:     input.Trigger.Cron,
			Interval: input.Trigger.Interval,
			Event:    input.Trigger.Event,
		},
		MaxRetries: input.MaxRetries,
		RetryDelay: input.RetryDelay,
		DependsOn:  input.DependsOn,
		ProfileID:  input.ProfileID,
		Enabled:    input.Enabled,
		CreatedAt:  time.Now(),
	}

	for _, a := range input.Actions {
		task.Actions = append(task.Actions, scheduler.TaskAction{
			Type:    a.Type,
			Target:  a.Target,
			Value:   a.Value,
			Timeout: a.Timeout,
		})
	}

	if err := a.scheduler.AddTask(task); err != nil {
		return nil, fmt.Errorf("failed to add task: %w", err)
	}

	info := taskToInfo(task)
	return &info, nil
}

// SchedulerRemoveTask deletes a task by ID.
func (a *App) SchedulerRemoveTask(id string) error {
	if a.scheduler == nil {
		return fmt.Errorf("scheduler not initialized")
	}
	return a.scheduler.RemoveTask(id)
}

// SchedulerRunTaskNow triggers immediate execution of a task.
func (a *App) SchedulerRunTaskNow(id string) {
	if a.scheduler == nil {
		return
	}
	a.scheduler.RunTaskNow(id)
}

// SchedulerTriggerEvent injects an event to trigger event-driven tasks.
func (a *App) SchedulerTriggerEvent(eventName string) {
	if a.scheduler == nil {
		return
	}
	a.scheduler.TriggerEvent(eventName)
}

// SchedulerGetEventNames returns all known event names suitable as triggers.
func (a *App) SchedulerGetEventNames() []string {
	return events.AllRegistryNames()
}

// ─── Helpers ───────────────────────────────────────────────────────────────────

func taskToInfo(t *scheduler.TaskDef) SchedulerTaskInfo {
	actions := make([]SchedulerTaskAction, 0, len(t.Actions))
	for _, a := range t.Actions {
		actions = append(actions, SchedulerTaskAction{
			Type:    a.Type,
			Target:  a.Target,
			Value:   a.Value,
			Timeout: a.Timeout,
		})
	}

	lastRunAt := ""
	if lr := t.LastRunAt(); !lr.IsZero() {
		lastRunAt = lr.Format(time.RFC3339)
	}

	return SchedulerTaskInfo{
		ID:   t.ID,
		Name: t.Name,
		Trigger: SchedulerTaskTrigger{
			Type:     string(t.Trigger.Type),
			Cron:     t.Trigger.Cron,
			Interval: t.Trigger.Interval,
			Event:    t.Trigger.Event,
		},
		Actions:    actions,
		MaxRetries: t.MaxRetries,
		RetryDelay: t.RetryDelay,
		DependsOn:  t.DependsOn,
		ProfileID:  t.ProfileID,
		Enabled:    t.Enabled,
		CreatedAt:  t.CreatedAt.Format(time.RFC3339),
		Status:     string(t.Status()),
		LastRunAt:  lastRunAt,
	}
}
