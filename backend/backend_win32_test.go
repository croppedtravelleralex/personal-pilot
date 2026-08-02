//go:build windows

package backend

import (
	"encoding/json"
	"strings"
	"sync"
	"testing"
	"time"
)

// ─── Test 9: Win32 Registration Functions ───────────────────────────────────────

func TestWin32RegistrationFunctions(t *testing.T) {
	t.Run("getPageMetrics with mock CDP", func(t *testing.T) {
		handler := mockCDPHandler(func(method string, params json.RawMessage) map[string]interface{} {
			if method == "Runtime.evaluate" {
				return map[string]interface{}{
					"result": map[string]interface{}{
						"type":  "string",
						"value": `{"vw":1920,"vh":1080,"dpr":1.5}`,
					},
				}
			}
			return map[string]interface{}{}
		})
		port, close := newMockCDPServer(t, handler)
		defer close()

		executor, err := connectCDPExecutor(port)
		if err != nil {
			t.Fatalf("connectCDPExecutor: %v", err)
		}
		defer executor.Close()

		metrics, err := getPageMetrics(executor)
		if err != nil {
			t.Fatalf("getPageMetrics: %v", err)
		}
		if metrics.ViewportW != 1920 {
			t.Errorf("ViewportW = %d, want 1920", metrics.ViewportW)
		}
		if metrics.ViewportH != 1080 {
			t.Errorf("ViewportH = %d, want 1080", metrics.ViewportH)
		}
		if metrics.DPR != 1.5 {
			t.Errorf("DPR = %f, want 1.5", metrics.DPR)
		}
	})

	t.Run("getElementCenter returns element coordinates", func(t *testing.T) {
		handler := mockCDPHandler(func(method string, params json.RawMessage) map[string]interface{} {
			if method == "Runtime.evaluate" {
				return map[string]interface{}{
					"result": map[string]interface{}{
						"type":  "string",
						"value": `{"x":500,"y":300}`,
					},
				}
			}
			return map[string]interface{}{}
		})
		port, close := newMockCDPServer(t, handler)
		defer close()

		executor, err := connectCDPExecutor(port)
		if err != nil {
			t.Fatalf("connectCDPExecutor: %v", err)
		}
		defer executor.Close()

		x, y, err := getElementCenter(executor, "#email")
		if err != nil {
			t.Fatalf("getElementCenter: %v", err)
		}
		if x != 500 || y != 300 {
			t.Errorf("got (%.0f, %.0f), want (500, 300)", x, y)
		}
	})

	t.Run("getElementCenter with timeout retry", func(t *testing.T) {
		var callCount int
		var mu sync.Mutex
		handler := mockCDPHandler(func(method string, params json.RawMessage) map[string]interface{} {
			if method == "Runtime.evaluate" {
				mu.Lock()
				callCount++
				cc := callCount
				mu.Unlock()
				if cc < 3 {
					return map[string]interface{}{
						"result": map[string]interface{}{"type": "string", "value": "null"},
					}
				}
				return map[string]interface{}{
					"result": map[string]interface{}{
						"type":  "string",
						"value": `{"x":400,"y":250}`,
					},
				}
			}
			return map[string]interface{}{}
		})
		port, close := newMockCDPServer(t, handler)
		defer close()

		executor, err := connectCDPExecutor(port)
		if err != nil {
			t.Fatalf("connectCDPExecutor: %v", err)
		}
		defer executor.Close()

		x, y, err := getElementCenter(executor, "#email")
		if err != nil {
			t.Fatalf("getElementCenter after retry: %v", err)
		}
		if x != 400 || y != 250 {
			t.Errorf("got (%.0f, %.0f), want (400, 250)", x, y)
		}
		mu.Lock()
		if callCount < 3 {
			t.Errorf("expected at least 3 calls, got %d", callCount)
		}
		mu.Unlock()
	})

	t.Run("findButtonCenter with keyword", func(t *testing.T) {
		var capturedExpression string
		handler := mockCDPHandler(func(method string, params json.RawMessage) map[string]interface{} {
			if method == "Runtime.evaluate" {
				var p struct {
					Expression string `json:"expression"`
				}
				json.Unmarshal(params, &p)
				capturedExpression = p.Expression

				return map[string]interface{}{
					"result": map[string]interface{}{
						"type":  "string",
						"value": `{"x":600,"y":350}`,
					},
				}
			}
			return map[string]interface{}{}
		})
		port, close := newMockCDPServer(t, handler)
		defer close()

		executor, err := connectCDPExecutor(port)
		if err != nil {
			t.Fatalf("connectCDPExecutor: %v", err)
		}
		defer executor.Close()

		x, y, err := findButtonCenter(executor, "send")
		if err != nil {
			t.Fatalf("findButtonCenter: %v", err)
		}
		if x != 600 || y != 350 {
			t.Errorf("got (%.0f, %.0f), want (600, 350)", x, y)
		}
		if !strings.Contains(capturedExpression, "send") {
			t.Errorf("expected expression to contain keyword 'send', got: %s", capturedExpression)
		}
	})

	t.Run("findButtonCenter returns fallback position when no keyword match", func(t *testing.T) {
		handler := mockCDPHandler(func(method string, params json.RawMessage) map[string]interface{} {
			if method == "Runtime.evaluate" {
				return map[string]interface{}{
					"result": map[string]interface{}{
						"type":  "string",
						"value": `{"x":200,"y":150}`,
					},
				}
			}
			return map[string]interface{}{}
		})
		port, close := newMockCDPServer(t, handler)
		defer close()

		executor, err := connectCDPExecutor(port)
		if err != nil {
			t.Fatalf("connectCDPExecutor: %v", err)
		}
		defer executor.Close()

		x, y, err := findButtonCenter(executor, "sign")
		if err != nil {
			t.Fatalf("findButtonCenter: %v", err)
		}
		if x != 200 || y != 150 {
			t.Errorf("got (%.0f, %.0f), want (200, 150)", x, y)
		}
	})

	t.Run("waitForPageReady success", func(t *testing.T) {
		handler := mockCDPHandler(func(method string, params json.RawMessage) map[string]interface{} {
			if method == "Runtime.evaluate" {
				return map[string]interface{}{
					"result": map[string]interface{}{"type": "string", "value": "true"},
				}
			}
			return map[string]interface{}{}
		})
		port, close := newMockCDPServer(t, handler)
		defer close()

		executor, err := connectCDPExecutor(port)
		if err != nil {
			t.Fatalf("connectCDPExecutor: %v", err)
		}
		defer executor.Close()

		err = waitForPageReady(executor, 5*time.Second)
		if err != nil {
			t.Fatalf("waitForPageReady: %v", err)
		}
	})

	t.Run("waitForPageReady timeout", func(t *testing.T) {
		handler := mockCDPHandler(func(method string, params json.RawMessage) map[string]interface{} {
			if method == "Runtime.evaluate" {
				return map[string]interface{}{
					"result": map[string]interface{}{"type": "string", "value": "false"},
				}
			}
			return map[string]interface{}{}
		})
		port, close := newMockCDPServer(t, handler)
		defer close()

		executor, err := connectCDPExecutor(port)
		if err != nil {
			t.Fatalf("connectCDPExecutor: %v", err)
		}
		defer executor.Close()

		err = waitForPageReady(executor, 50*time.Millisecond)
		if err == nil {
			t.Fatal("expected timeout error")
		}
	})
}
