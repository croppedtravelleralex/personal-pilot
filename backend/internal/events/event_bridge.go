package events

import (
	"context"
	"sync"
	"time"

	"github.com/wailsapp/wails/v2/pkg/runtime"
)

// eventLogStore is the singleton event log store, set at startup.
var (
	eventLogStore   EventLogStore
	eventLogStoreMu sync.RWMutex
)

type FrontendEmitter func(eventName string, data ...interface{})

type wailsEventRuntime interface {
	Emit(string, ...interface{})
}

var (
	frontendEmitter   FrontendEmitter
	frontendEmitterMu sync.RWMutex
)

func SetFrontendEmitter(emitter FrontendEmitter) {
	frontendEmitterMu.Lock()
	defer frontendEmitterMu.Unlock()
	frontendEmitter = emitter
}

func HasFrontendEmitter() bool {
	frontendEmitterMu.RLock()
	defer frontendEmitterMu.RUnlock()
	return frontendEmitter != nil
}

func EmitFrontend(ctx context.Context, eventName string, data ...interface{}) {
	frontendEmitterMu.RLock()
	emitter := frontendEmitter
	frontendEmitterMu.RUnlock()
	if emitter != nil {
		emitter(eventName, data...)
		return
	}
	if hasWailsEvents(ctx) {
		runtime.EventsEmit(ctx, eventName, data...)
	}
}

func hasWailsEvents(ctx context.Context) bool {
	if ctx == nil {
		return false
	}
	_, ok := ctx.Value("events").(wailsEventRuntime)
	return ok
}

// RuleEvaluator is the interface for the automation rule engine.
type RuleEvaluator interface {
	Evaluate(eventName string, payload map[string]interface{})
}

// ruleEngine is the singleton rule engine reference.
var (
	ruleEngine   RuleEvaluator
	ruleEngineMu sync.RWMutex
)

// SetEventLogStore sets the global event log store for persistence.
func SetEventLogStore(store EventLogStore) {
	eventLogStoreMu.Lock()
	defer eventLogStoreMu.Unlock()
	eventLogStore = store
}

// SetRuleEngine sets the global rule engine for event evaluation.
func SetRuleEngine(re RuleEvaluator) {
	ruleEngineMu.Lock()
	defer ruleEngineMu.Unlock()
	ruleEngine = re
}

// EmitAndLog emits an event to the Wails frontend and asynchronously persists it.
// This is the canonical function for all event emission throughout the app.
func EmitAndLog(ctx context.Context, eventName string, data ...interface{}) {
	// Emit to frontend
	EmitFrontend(ctx, eventName, data...)

	// Extract payload for persistence and rule evaluation
	payload := extractPayload(data)

	// Persist asynchronously (best-effort, don't block the emitter)
	go persistEvent(eventName, payload)

	// Feed to rule engine (non-blocking, via buffered channel)
	ruleEngineMu.RLock()
	re := ruleEngine
	ruleEngineMu.RUnlock()
	if re != nil {
		re.Evaluate(eventName, payload)
	}
}

func extractPayload(data []interface{}) map[string]interface{} {
	if len(data) > 0 {
		if m, ok := data[0].(map[string]interface{}); ok {
			return m
		}
	}
	return map[string]interface{}{}
}

func persistEvent(eventName string, payload map[string]interface{}) {
	eventLogStoreMu.RLock()
	store := eventLogStore
	eventLogStoreMu.RUnlock()

	if store == nil {
		return
	}

	ns := eventNamespace(eventName)
	sev := severityFromRegistry(eventName)

	entry := &EventLogEntry{
		EventName: eventName,
		Namespace: ns,
		Severity:  sev,
		Payload:   payload,
		CreatedAt: time.Now().Format(time.RFC3339),
	}
	_ = store.Insert(entry)
}

// eventNamespace extracts the two-level namespace from an event name.
// e.g. "account:login:success" → "account:login"
func eventNamespace(name string) string {
	idx := nameWithColon(name, 0)
	if idx == -1 {
		return name
	}
	idx2 := nameWithColon(name, idx+1)
	if idx2 == -1 {
		return name[:idx]
	}
	return name[:idx2]
}

func nameWithColon(s string, start int) int {
	for i := start; i < len(s); i++ {
		if s[i] == ':' {
			return i
		}
	}
	return -1
}

// severityFromRegistry returns the severity from the Registry, falling back to "info".
func severityFromRegistry(eventName string) string {
	if def, ok := Registry[eventName]; ok {
		return def.Severity
	}
	return "info"
}
