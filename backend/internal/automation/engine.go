package automation

import (
	"context"
	"fmt"
	"math"
	"strconv"
	"strings"
	"sync"
)

// EmitFn is the function signature for emitting events.
type EmitFn func(eventName string, data ...interface{})

// TaskRunner is the function signature for running a scheduler task by ID.
type TaskRunner func(taskID string)

// RuleEngine evaluates automation rules in response to events.
type RuleEngine struct {
	store     RuleStore
	emitFn    EmitFn
	runTaskFn TaskRunner

	mu      sync.RWMutex
	ctx     context.Context
	cancel  context.CancelFunc
	eventCh chan eventMsg
	done    chan struct{}
}

type eventMsg struct {
	eventName string
	payload   map[string]interface{}
}

// NewRuleEngine creates a new rule engine.
func NewRuleEngine(store RuleStore, emitFn EmitFn, runTaskFn TaskRunner) *RuleEngine {
	return &RuleEngine{
		store:     store,
		emitFn:    emitFn,
		runTaskFn: runTaskFn,
		eventCh:   make(chan eventMsg, 256),
		done:      make(chan struct{}),
	}
}

// Start begins the background evaluation loop.
func (e *RuleEngine) Start(ctx context.Context) {
	e.mu.Lock()
	e.ctx, e.cancel = context.WithCancel(ctx)
	ctxLocal := e.ctx
	e.mu.Unlock()

	go func() {
		defer close(e.done)
		for {
			select {
			case <-ctxLocal.Done():
				return
			case msg := <-e.eventCh:
				e.evaluate(msg.eventName, msg.payload)
			}
		}
	}()
}

// Stop gracefully shuts down the engine.
func (e *RuleEngine) Stop() {
	e.mu.Lock()
	if e.cancel != nil {
		e.cancel()
	}
	e.mu.Unlock()
	<-e.done
}

// Evaluate feeds an event into the engine for rule matching (non-blocking).
func (e *RuleEngine) Evaluate(eventName string, payload map[string]interface{}) {
	select {
	case e.eventCh <- eventMsg{eventName: eventName, payload: payload}:
	default:
		// Channel full, drop event to avoid blocking the emitter
	}
}

// evaluate processes a single event against all enabled rules.
func (e *RuleEngine) evaluate(eventName string, payload map[string]interface{}) {
	rules, err := e.store.List()
	if err != nil {
		return
	}

	for _, rule := range rules {
		if !rule.Enabled {
			continue
		}
		if !eventMatches(rule.TriggerEvent, eventName) {
			continue
		}
		if !evaluateCondition(rule.Condition, payload) {
			continue
		}
		if !rule.CanTrigger() {
			continue
		}

		rule.MarkTriggered()
		e.executeAction(rule, payload)
	}
}

// executeAction performs the rule's action.
func (e *RuleEngine) executeAction(rule *AutoRule, payload map[string]interface{}) {
	switch rule.Action {
	case ActionEmitEvent:
		targetEvent, ok := rule.ActionParams["event"].(string)
		if !ok || targetEvent == "" {
			return
		}
		if e.emitFn != nil {
			e.emitFn(targetEvent, payload)
		}
	case ActionRunTask:
		taskID, ok := rule.ActionParams["taskId"].(string)
		if !ok || taskID == "" {
			return
		}
		if e.runTaskFn != nil {
			e.runTaskFn(taskID)
		}
	case ActionNotify:
		// Notify is handled by emitting an "automation:notification" event
		notifyPayload := map[string]interface{}{
			"ruleId":   rule.ID,
			"ruleName": rule.Name,
			"message":  rule.ActionParams["message"],
			"severity": rule.ActionParams["severity"],
			"payload":  payload,
		}
		if e.emitFn != nil {
			e.emitFn("automation:notification", notifyPayload)
		}
	}
}

// eventMatches checks if the event name matches the rule's trigger pattern.
// Supports exact match and wildcard: "risk:proxy:*" matches "risk:proxy:high-latency".
func eventMatches(pattern, eventName string) bool {
	if !strings.Contains(pattern, "*") {
		return pattern == eventName
	}
	prefix := strings.TrimSuffix(pattern, "*")
	return strings.HasPrefix(eventName, prefix)
}

// evaluateCondition evaluates a simple condition string against the event payload.
// Supported operators: >=, <=, >, <, ==, !=, contains
// Examples: "fraudScore>=70", "status==banned", "type!=datacenter", "msg contains error"
func evaluateCondition(condition string, payload map[string]interface{}) bool {
	condition = strings.TrimSpace(condition)
	if condition == "" {
		return true
	}

	// Parse operator
	key, op, expected := splitCondition(condition)
	if key == "" || op == "" {
		return true // malformed condition → always match
	}

	val, ok := getNestedValue(payload, key)
	if !ok {
		return false // field not found → no match
	}

	switch op {
	case ">=", ">", "<", "<=":
		return compareNumeric(val, op, expected)
	case "==":
		return fmt.Sprintf("%v", val) == expected
	case "!=":
		return fmt.Sprintf("%v", val) != expected
	case "contains":
		return strings.Contains(
			strings.ToLower(fmt.Sprintf("%v", val)),
			strings.ToLower(expected),
		)
	default:
		return true
	}
}

// splitCondition splits "key>=value" into ("key", ">=", "value").
func splitCondition(s string) (key, op, value string) {
	ops := []string{">=", "<=", "!=", "==", ">", "<", "contains"}
	for _, o := range ops {
		if idx := strings.Index(s, o); idx >= 0 {
			key = strings.TrimSpace(s[:idx])
			op = o
			value = strings.TrimSpace(s[idx+len(o):])
			return
		}
	}
	return s, "", ""
}

// getNestedValue retrieves a value from the payload, supporting dot-notation keys.
// e.g. "result.score" → payload["result"].(map[string]interface{})["score"].
func getNestedValue(payload map[string]interface{}, key string) (interface{}, bool) {
	parts := strings.Split(key, ".")
	var current interface{} = payload
	for _, part := range parts {
		m, ok := current.(map[string]interface{})
		if !ok {
			return nil, false
		}
		v, ok := m[part]
		if !ok {
			return nil, false
		}
		current = v
	}
	return current, true
}

// compareNumeric compares a numeric value against an operator and expected string.
func compareNumeric(val interface{}, op, expected string) bool {
	a := toFloat64(val)
	b, err := strconv.ParseFloat(expected, 64)
	if err != nil {
		return false
	}
	switch op {
	case ">=":
		return a >= b
	case ">":
		return a > b
	case "<=":
		return a <= b
	case "<":
		return a < b
	default:
		return false
	}
}

// toFloat64 converts a value to float64 for numeric comparison.
func toFloat64(v interface{}) float64 {
	switch n := v.(type) {
	case float64:
		return n
	case float32:
		return float64(n)
	case int:
		return float64(n)
	case int64:
		return float64(n)
	case int32:
		return float64(n)
	case string:
		f, err := strconv.ParseFloat(n, 64)
		if err != nil {
			return math.NaN()
		}
		return f
	default:
		return math.NaN()
	}
}
