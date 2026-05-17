package backend

import (
	"context"
	"encoding/json"
	"strings"
	"testing"
	"time"
)

type fakeBrowserActionExecutor struct {
	navigatedURL string
	clicked      bool
	typed        struct {
		selector string
		value    string
	}
	evaluatedRaw string
	evaluatedJS  string
	movedTo      struct {
		x float64
		y float64
	}
}

func (f *fakeBrowserActionExecutor) Navigate(url string) error {
	f.navigatedURL = url
	return nil
}

func (f *fakeBrowserActionExecutor) ExecuteHumanizedClick(selector string) error {
	f.clicked = true
	f.typed.selector = selector
	return nil
}

func (f *fakeBrowserActionExecutor) ExecuteHumanizedType(selector, value string) error {
	f.typed.selector = selector
	f.typed.value = value
	return nil
}

func (f *fakeBrowserActionExecutor) EvaluateRaw(script string) ([]byte, error) {
	f.evaluatedRaw = script
	return []byte(`{"ok":true}`), nil
}

func (f *fakeBrowserActionExecutor) EvaluateJS(script string) (string, error) {
	f.evaluatedJS = script
	return "done", nil
}

func (f *fakeBrowserActionExecutor) MoveMouseTo(x, y float64) error {
	f.movedTo.x = x
	f.movedTo.y = y
	return nil
}

func (f *fakeBrowserActionExecutor) Click() error {
	f.clicked = true
	return nil
}

func TestExecuteBrowserActionNavigateUsesURLFallback(t *testing.T) {
	executor := &fakeBrowserActionExecutor{}

	_, err := executeBrowserAction(context.Background(), executor, BrowserAction{Type: " navigate ", Value: "https://example.test"})
	if err != nil {
		t.Fatalf("executeBrowserAction: %v", err)
	}
	if executor.navigatedURL != "https://example.test" {
		t.Fatalf("navigatedURL = %q, want fallback value URL", executor.navigatedURL)
	}
}

func TestExecuteBrowserActionClickCoordinates(t *testing.T) {
	x := 12.5
	y := 33.0
	executor := &fakeBrowserActionExecutor{}

	_, err := executeBrowserAction(context.Background(), executor, BrowserAction{Type: BrowserActionClick, X: &x, Y: &y})
	if err != nil {
		t.Fatalf("executeBrowserAction: %v", err)
	}
	if executor.movedTo.x != 12.5 || executor.movedTo.y != 33 {
		t.Fatalf("movedTo = (%v,%v), want (12.5,33)", executor.movedTo.x, executor.movedTo.y)
	}
	if !executor.clicked {
		t.Fatal("expected coordinate click")
	}
}

func TestExecuteBrowserActionTypeRequiresSelector(t *testing.T) {
	_, err := executeBrowserAction(context.Background(), &fakeBrowserActionExecutor{}, BrowserAction{Type: BrowserActionType, Value: "hello"})
	if err == nil || !strings.Contains(err.Error(), "requires selector") {
		t.Fatalf("error = %v, want selector requirement", err)
	}
}

func TestExecuteBrowserActionScrollReturnsRawResult(t *testing.T) {
	executor := &fakeBrowserActionExecutor{}

	result, err := executeBrowserAction(context.Background(), executor, BrowserAction{Type: BrowserActionScroll, DeltaX: 4, DeltaY: 10})
	if err != nil {
		t.Fatalf("executeBrowserAction: %v", err)
	}
	if executor.evaluatedRaw != "window.scrollBy(4, 10)" {
		t.Fatalf("evaluatedRaw = %q, want scroll script", executor.evaluatedRaw)
	}
	raw, ok := result.(json.RawMessage)
	if !ok {
		t.Fatalf("result type = %T, want json.RawMessage", result)
	}
	if string(raw) != `{"ok":true}` {
		t.Fatalf("result = %s, want raw result", raw)
	}
}

func TestExecuteBrowserActionEvaluateReturnsValue(t *testing.T) {
	executor := &fakeBrowserActionExecutor{}

	result, err := executeBrowserAction(context.Background(), executor, BrowserAction{Type: BrowserActionEval, Script: "document.title"})
	if err != nil {
		t.Fatalf("executeBrowserAction: %v", err)
	}
	if executor.evaluatedJS != "document.title" {
		t.Fatalf("evaluatedJS = %q, want document.title", executor.evaluatedJS)
	}
	if result != "done" {
		t.Fatalf("result = %v, want done", result)
	}
}

func TestExecuteBrowserActionWaitHonorsContext(t *testing.T) {
	ctx, cancel := context.WithTimeout(context.Background(), time.Millisecond)
	defer cancel()

	_, err := executeBrowserAction(ctx, &fakeBrowserActionExecutor{}, BrowserAction{Type: BrowserActionWait, WaitMs: 1000})
	if err == nil || !strings.Contains(err.Error(), "context deadline exceeded") {
		t.Fatalf("error = %v, want context deadline", err)
	}
}

func TestExecuteBrowserActionRejectsRawCDP(t *testing.T) {
	_, err := executeBrowserAction(context.Background(), &fakeBrowserActionExecutor{}, BrowserAction{Type: "rawcdp"})
	if err == nil || !strings.Contains(err.Error(), "unsupported action type") {
		t.Fatalf("error = %v, want unsupported rawcdp", err)
	}
}
