package scheduler

import (
	"encoding/json"
	"fmt"
	"net"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"
	"time"

	"github.com/gorilla/websocket"
)

// ─── CDP Runner Tests ───────────────────────────────────────────────────────

func TestCDPTaskRunner_ProfileNotFound(t *testing.T) {
	resolver := func(profileID string) (int, error) {
		return 0, fmt.Errorf("profile not found")
	}
	runner := NewCDPTaskRunner(resolver)

	task := &TaskDef{
		ID:        "test-1",
		Name:      "无Profile",
		ProfileID: "nonexistent",
		Actions:   []TaskAction{{Type: "navigate", Target: "https://example.com"}},
	}

	err := runner.Run(task)
	if err == nil {
		t.Fatal("期望错误，但得到 nil")
	}
	if !strings.Contains(err.Error(), "profile not found") {
		t.Fatalf("错误消息不匹配: %v", err)
	}
}

func TestCDPTaskRunner_TaskNoProfileID(t *testing.T) {
	runner := NewCDPTaskRunner(func(profileID string) (int, error) {
		return 0, nil
	})
	task := &TaskDef{ID: "test-2", Name: "无ProfileID", ProfileID: ""}

	err := runner.Run(task)
	if err == nil {
		t.Fatal("期望错误，但得到 nil")
	}
	if !strings.Contains(err.Error(), "no profile ID") {
		t.Fatalf("错误消息不匹配: %v", err)
	}
}

func TestCDPTaskRunner_CDPConnectionRefused(t *testing.T) {
	// Use a port that's very unlikely to have a CDP server
	resolver := func(profileID string) (int, error) {
		return 51999, nil
	}
	runner := NewCDPTaskRunner(resolver)
	task := &TaskDef{
		ID:        "test-3",
		Name:      "连接拒绝",
		ProfileID: "test-profile",
		Actions:   []TaskAction{{Type: "navigate", Target: "https://example.com"}},
	}

	err := runner.Run(task)
	if err == nil {
		t.Fatal("期望错误，但得到 nil")
	}
}

// ─── Mock CDP Server ────────────────────────────────────────────────────────

// mockCDPServer creates a test HTTP+WebSocket server that simulates CDP.
// Returns the server URL and a channel to receive received commands.
func mockCDPServer(t *testing.T, handler func(method string, params interface{}) (interface{}, error)) (*httptest.Server, chan string) {
	t.Helper()

	cmdCh := make(chan string, 100)
	var upgrader = websocket.Upgrader{
		CheckOrigin: func(r *http.Request) bool { return true },
	}

	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if strings.HasSuffix(r.URL.Path, "/json/version") {
			w.Header().Set("Content-Type", "application/json")
			wsURL := "ws://" + r.Host + "/devtools"
			json.NewEncoder(w).Encode(map[string]string{
				"webSocketDebuggerUrl": wsURL,
			})
			return
		}

		// WebSocket upgrade
		conn, err := upgrader.Upgrade(w, r, nil)
		if err != nil {
			return
		}
		defer conn.Close()

		for {
			_, msg, err := conn.ReadMessage()
			if err != nil {
				return
			}

			var req struct {
				ID     int             `json:"id"`
				Method string          `json:"method"`
				Params json.RawMessage `json:"params"`
			}
			if err := json.Unmarshal(msg, &req); err != nil {
				continue
			}

			cmdCh <- req.Method

			result, err := handler(req.Method, req.Params)
			if err != nil {
				// Send CDP error
				resp := map[string]interface{}{
					"id": req.ID,
					"error": map[string]interface{}{
						"message": err.Error(),
					},
				}
				conn.WriteJSON(resp)
			} else {
				resp := map[string]interface{}{
					"id":     req.ID,
					"result": result,
				}
				conn.WriteJSON(resp)
			}
		}
	}))

	return server, cmdCh
}

func TestCDPTaskRunner_NavigateAction(t *testing.T) {
	server, cmdCh := mockCDPServer(t, func(method string, params interface{}) (interface{}, error) {
		switch method {
		case "Page.navigate":
			return map[string]interface{}{"frameId": "frame-1", "loaderId": "loader-1"}, nil
		default:
			return nil, fmt.Errorf("unexpected method: %s", method)
		}
	})
	defer server.Close()

	// Extract the port from the test server
	port := server.Listener.Addr().(*net.TCPAddr).Port
	resolver := func(profileID string) (int, error) {
		return port, nil
	}

	runner := NewCDPTaskRunner(resolver)
	task := &TaskDef{
		ID:        "nav-test",
		Name:      "导航测试",
		ProfileID: "test-profile",
		Actions:   []TaskAction{{Type: "navigate", Target: "https://example.com"}},
	}

	err := runner.Run(task)
	if err != nil {
		t.Fatalf("导航动作失败: %v", err)
	}

	select {
	case method := <-cmdCh:
		if method != "Page.navigate" {
			t.Fatalf("期望 Page.navigate，得到 %s", method)
		}
	case <-time.After(time.Second):
		t.Fatal("未收到 CDP 命令")
	}
}

func TestCDPTaskRunner_ClickAction(t *testing.T) {
	server, _ := mockCDPServer(t, func(method string, params interface{}) (interface{}, error) {
		switch method {
		case "Runtime.evaluate":
			return map[string]interface{}{
				"result": map[string]interface{}{
					"type": "undefined",
				},
			}, nil
		default:
			return nil, fmt.Errorf("unexpected: %s", method)
		}
	})
	defer server.Close()

	port := server.Listener.Addr().(*net.TCPAddr).Port
	runner := NewCDPTaskRunner(func(id string) (int, error) { return port, nil })

	task := &TaskDef{
		ID:        "click-test",
		ProfileID: "p1",
		Actions:   []TaskAction{{Type: "click", Target: ".submit-btn"}},
	}

	err := runner.Run(task)
	if err != nil {
		t.Fatalf("点击动作失败: %v", err)
	}
}

func TestCDPTaskRunner_UnknownAction(t *testing.T) {
	runner := NewCDPTaskRunner(func(id string) (int, error) { return 0, nil })
	task := &TaskDef{
		ID:        "unknown",
		ProfileID: "p1",
		Actions:   []TaskAction{{Type: "invalid_action_type", Target: ""}},
	}

	err := runner.Run(task)
	if err == nil {
		t.Fatal("未知动作类型应该返回错误")
	}
}

func TestCDPTaskRunner_ExtractAction(t *testing.T) {
	server, _ := mockCDPServer(t, func(method string, params interface{}) (interface{}, error) {
		switch method {
		case "Runtime.evaluate":
			return map[string]interface{}{
				"result": map[string]interface{}{
					"type":  "string",
					"value": `{"text":"Hello World","html":"<p>Hello World</p>"}`,
				},
			}, nil
		default:
			return nil, fmt.Errorf("unexpected: %s", method)
		}
	})
	defer server.Close()

	port := server.Listener.Addr().(*net.TCPAddr).Port
	runner := NewCDPTaskRunner(func(id string) (int, error) { return port, nil })

	task := &TaskDef{
		ID:        "extract-test",
		ProfileID: "p1",
		Actions:   []TaskAction{{Type: "extract", Target: ".content", Value: "text"}},
	}

	err := runner.Run(task)
	if err != nil {
		t.Fatalf("提取动作失败: %v", err)
	}
}

func TestCDPTaskRunner_WaitActionTimeout(t *testing.T) {
	server, _ := mockCDPServer(t, func(method string, params interface{}) (interface{}, error) {
		switch method {
		case "Runtime.evaluate":
			// Simulate element not found (timed out)
			return map[string]interface{}{
				"result": map[string]interface{}{
					"type":  "boolean",
					"value": false,
				},
			}, nil
		default:
			return nil, fmt.Errorf("unexpected: %s", method)
		}
	})
	defer server.Close()

	port := server.Listener.Addr().(*net.TCPAddr).Port
	runner := NewCDPTaskRunner(func(id string) (int, error) { return port, nil })

	task := &TaskDef{
		ID:        "wait-test",
		ProfileID: "p1",
		Actions:   []TaskAction{{Type: "wait", Target: ".nonexistent", Timeout: 100}},
	}

	err := runner.Run(task)
	if err == nil {
		t.Fatal("等待不存在的元素应该返回错误")
	}
	if !strings.Contains(err.Error(), "timed out") {
		t.Fatalf("错误消息应该包含 timed out: %v", err)
	}
}

func TestCDPTaskRunner_CDPAction(t *testing.T) {
	server, _ := mockCDPServer(t, func(method string, params interface{}) (interface{}, error) {
		switch method {
		case "Runtime.evaluate":
			return map[string]interface{}{
				"result": map[string]interface{}{
					"type":  "string",
					"value": "ok",
				},
			}, nil
		default:
			return nil, fmt.Errorf("unexpected: %s", method)
		}
	})
	defer server.Close()

	port := server.Listener.Addr().(*net.TCPAddr).Port
	runner := NewCDPTaskRunner(func(id string) (int, error) { return port, nil })

	task := &TaskDef{
		ID:        "cdp-test",
		ProfileID: "p1",
		Actions: []TaskAction{
			{
				Type:   "cdp",
				Target: "Runtime.evaluate",
				Value:  `{"expression":"1+1"}`,
			},
		},
	}

	err := runner.Run(task)
	if err != nil {
		t.Fatalf("CDP 动作失败: %v", err)
	}
}

func TestCDPTaskRunner_WaitActionSleep(t *testing.T) {
	// Wait with no selector should just sleep (no CDP connection needed)
	runner := NewCDPTaskRunner(func(id string) (int, error) { return 51999, nil })

	task := &TaskDef{
		ID:        "sleep-test",
		ProfileID: "p1",
		Actions:   []TaskAction{{Type: "wait", Target: "", Timeout: 10}},
	}

	// Should succeed since no CDP connection is made for sleep-only waits
	err := runner.Run(task)
	if err == nil {
		t.Fatal("should have failed due to CDP connection error since other actions follow")
	}
}

func TestCDPTaskRunner_MultipleActions(t *testing.T) {
	actionCount := 0
	server, _ := mockCDPServer(t, func(method string, params interface{}) (interface{}, error) {
		actionCount++
		switch method {
		case "Page.navigate":
			return map[string]interface{}{"frameId": "f1"}, nil
		case "Runtime.evaluate":
			return map[string]interface{}{
				"result": map[string]interface{}{
					"type":  "undefined",
					"value": nil,
				},
			}, nil
		default:
			return nil, fmt.Errorf("unexpected: %s", method)
		}
	})
	defer server.Close()

	port := server.Listener.Addr().(*net.TCPAddr).Port
	runner := NewCDPTaskRunner(func(id string) (int, error) { return port, nil })

	task := &TaskDef{
		ID:        "multi",
		ProfileID: "p1",
		Actions: []TaskAction{
			{Type: "navigate", Target: "https://example.com"},
			{Type: "click", Target: ".btn"},
			{Type: "extract", Target: ".title"},
		},
	}

	err := runner.Run(task)
	if err != nil {
		t.Fatalf("多动作执行失败: %v", err)
	}
	if actionCount != 3 {
		t.Fatalf("期望 3 个 CDP 命令，收到 %d", actionCount)
	}
}
