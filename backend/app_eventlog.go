package backend

import (
	"ant-chrome/backend/internal/events"
	"encoding/json"
	"fmt"
	"time"
)

// ─── Wails-bound types ─────────────────────────────────────────────────────────

// EventLogQueryInput is the frontend-facing query input.
type EventLogQueryInput struct {
	After     string `json:"after"`
	Before    string `json:"before"`
	Namespace string `json:"namespace"`
	Severity  string `json:"severity"`
	EventName string `json:"eventName"`
	Limit     int    `json:"limit"`
	Offset    int    `json:"offset"`
}

// ─── Wails-bound methods ───────────────────────────────────────────────────────

// EventLogQuery returns matching event log entries.
func (a *App) EventLogQuery(input EventLogQueryInput) ([]events.EventLogEntry, error) {
	store := a.getEventLogStore()
	if store == nil {
		return nil, fmt.Errorf("event log store not initialized")
	}
	entries, err := store.Query(events.EventLogQuery{
		After:     input.After,
		Before:    input.Before,
		Namespace: input.Namespace,
		Severity:  input.Severity,
		EventName: input.EventName,
		Limit:     input.Limit,
		Offset:    input.Offset,
	})
	if err != nil {
		return nil, err
	}
	result := make([]events.EventLogEntry, 0, len(entries))
	for _, e := range entries {
		result = append(result, *e)
	}
	return result, nil
}

// EventLogCount returns the count of matching event log entries.
func (a *App) EventLogCount(input EventLogQueryInput) (int, error) {
	store := a.getEventLogStore()
	if store == nil {
		return 0, fmt.Errorf("event log store not initialized")
	}
	return store.Count(events.EventLogQuery{
		After:     input.After,
		Before:    input.Before,
		Namespace: input.Namespace,
		Severity:  input.Severity,
		EventName: input.EventName,
	})
}

// EventLogPrune deletes log entries older than the given RFC3339 timestamp.
// Returns the number of deleted rows.
func (a *App) EventLogPrune(before string) (int, error) {
	store := a.getEventLogStore()
	if store == nil {
		return 0, fmt.Errorf("event log store not initialized")
	}
	t, err := time.Parse(time.RFC3339, before)
	if err != nil {
		return 0, fmt.Errorf("invalid before timestamp: %w", err)
	}
	n, err := store.Prune(t)
	return int(n), err
}

// EventLogExport returns event log entries as a JSON string for download.
func (a *App) EventLogExport(input EventLogQueryInput) (string, error) {
	store := a.getEventLogStore()
	if store == nil {
		return "", fmt.Errorf("event log store not initialized")
	}
	entries, err := store.Query(events.EventLogQuery{
		After:     input.After,
		Before:    input.Before,
		Namespace: input.Namespace,
		Severity:  input.Severity,
		EventName: input.EventName,
		Limit:     10000, // export up to 10k entries
	})
	if err != nil {
		return "", err
	}
	result := make([]events.EventLogEntry, 0, len(entries))
	for _, e := range entries {
		result = append(result, *e)
	}
	b, err := json.Marshal(result)
	if err != nil {
		return "", err
	}
	return string(b), nil
}

// EmitAndLogEvent emits an event to the frontend AND persists it to the log.
// This can be called from the frontend for custom events.
func (a *App) EmitAndLogEvent(eventName string, payload map[string]interface{}) {
	if a.ctx == nil {
		return
	}
	a.emit(eventName, payload)
}

// ─── Helpers ───────────────────────────────────────────────────────────────────

func (a *App) getEventLogStore() events.EventLogStore {
	if a.eventLogStore != nil {
		return a.eventLogStore
	}
	return nil
}
