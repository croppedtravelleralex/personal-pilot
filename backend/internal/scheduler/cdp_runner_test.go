package scheduler

import (
	"encoding/json"
	"fmt"
	"net"
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
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

func TestCDPTaskRunner_WaitActionParsesRuntimeEvaluateResult(t *testing.T) {
	server, _ := mockCDPServer(t, func(method string, params interface{}) (interface{}, error) {
		switch method {
		case "Runtime.evaluate":
			return map[string]interface{}{
				"result": map[string]interface{}{
					"type":  "boolean",
					"value": true,
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
		ID:        "wait-found",
		ProfileID: "p1",
		Actions:   []TaskAction{{Type: "wait", Target: ".ready", Timeout: 100}},
	}

	if err := runner.Run(task); err != nil {
		t.Fatalf("等待 Runtime.evaluate 嵌套布尔结果失败: %v", err)
	}
}

func TestRuntimeEvaluateValueHelpers(t *testing.T) {
	boolValue, err := runtimeEvaluateBool(json.RawMessage(`{"result":{"type":"boolean","value":true}}`))
	if err != nil {
		t.Fatalf("runtimeEvaluateBool nested: %v", err)
	}
	if !boolValue {
		t.Fatal("runtimeEvaluateBool nested = false, want true")
	}

	stringValue, err := runtimeEvaluateString(json.RawMessage(`{"result":{"type":"string","value":"ok"}}`))
	if err != nil {
		t.Fatalf("runtimeEvaluateString nested: %v", err)
	}
	if stringValue != "ok" {
		t.Fatalf("runtimeEvaluateString nested = %q, want ok", stringValue)
	}

	legacyValue, err := runtimeEvaluateString(json.RawMessage(`{"value":"legacy"}`))
	if err != nil {
		t.Fatalf("runtimeEvaluateString legacy: %v", err)
	}
	if legacyValue != "legacy" {
		t.Fatalf("runtimeEvaluateString legacy = %q, want legacy", legacyValue)
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

func TestCDPTaskRunner_CDPActionDispatchesDialogDownloadAndUploadPrimitives(t *testing.T) {
	type receivedCommand struct {
		method string
		params map[string]interface{}
	}

	received := make(chan receivedCommand, 3)
	server, _ := mockCDPServer(t, func(method string, params interface{}) (interface{}, error) {
		raw, ok := params.(json.RawMessage)
		if !ok {
			return nil, fmt.Errorf("params type = %T, want json.RawMessage", params)
		}
		decoded := map[string]interface{}{}
		if len(raw) > 0 {
			if err := json.Unmarshal(raw, &decoded); err != nil {
				return nil, fmt.Errorf("decode params: %w", err)
			}
		}
		received <- receivedCommand{method: method, params: decoded}
		return map[string]interface{}{"ok": true}, nil
	})
	defer server.Close()

	uploadPath := filepath.Join(t.TempDir(), "upload.txt")
	if err := os.WriteFile(uploadPath, []byte("upload fixture"), 0644); err != nil {
		t.Fatalf("write upload fixture: %v", err)
	}

	port := server.Listener.Addr().(*net.TCPAddr).Port
	runner := NewCDPTaskRunner(func(id string) (int, error) { return port, nil })
	downloadPath := t.TempDir()

	task := &TaskDef{
		ID:        "raw-primitives",
		ProfileID: "p1",
		Actions: []TaskAction{
			{
				Type:   "cdp",
				Target: "Page.handleJavaScriptDialog",
				Value:  `{"accept":true,"promptText":"ok"}`,
			},
			{
				Type:   "cdp",
				Target: "Page.setDownloadBehavior",
				Value:  fmt.Sprintf(`{"behavior":"allow","downloadPath":%q}`, downloadPath),
			},
			{
				Type:   "cdp",
				Target: "DOM.setFileInputFiles",
				Value:  fmt.Sprintf(`{"nodeId":7,"files":[%q]}`, uploadPath),
			},
		},
	}

	if err := runner.Run(task); err != nil {
		t.Fatalf("raw CDP primitives failed: %v", err)
	}

	wantMethods := []string{
		"Page.handleJavaScriptDialog",
		"Page.setDownloadBehavior",
		"DOM.setFileInputFiles",
	}
	for i, want := range wantMethods {
		select {
		case got := <-received:
			if got.method != want {
				t.Fatalf("command %d method = %q, want %q", i, got.method, want)
			}
			switch want {
			case "Page.handleJavaScriptDialog":
				if got.params["accept"] != true || got.params["promptText"] != "ok" {
					t.Fatalf("dialog params = %#v", got.params)
				}
			case "Page.setDownloadBehavior":
				if got.params["behavior"] != "allow" || got.params["downloadPath"] != downloadPath {
					t.Fatalf("download params = %#v", got.params)
				}
			case "DOM.setFileInputFiles":
				files, ok := got.params["files"].([]interface{})
				if !ok || len(files) != 1 || files[0] != uploadPath || got.params["nodeId"] != float64(7) {
					t.Fatalf("upload params = %#v", got.params)
				}
			}
		case <-time.After(time.Second):
			t.Fatalf("timed out waiting for command %d %s", i, want)
		}
	}
}

func TestCDPTaskRunner_TypedM4PrimitiveActions(t *testing.T) {
	type receivedCommand struct {
		method string
		params map[string]interface{}
	}

	received := make(chan receivedCommand, 16)
	server, _ := mockCDPServer(t, func(method string, params interface{}) (interface{}, error) {
		decoded := map[string]interface{}{}
		if raw, ok := params.(json.RawMessage); ok && len(raw) > 0 {
			if err := json.Unmarshal(raw, &decoded); err != nil {
				return nil, fmt.Errorf("decode params: %w", err)
			}
		}
		received <- receivedCommand{method: method, params: decoded}

		switch method {
		case "Runtime.evaluate":
			return map[string]interface{}{
				"result": map[string]interface{}{
					"type":  "object",
					"value": map[string]interface{}{"ok": true},
				},
			}, nil
		case "DOM.getDocument":
			return map[string]interface{}{"root": map[string]interface{}{"nodeId": 1}}, nil
		case "DOM.querySelector":
			return map[string]interface{}{"nodeId": 7}, nil
		case "DOM.setFileInputFiles", "Page.handleJavaScriptDialog", "Page.setDownloadBehavior",
			"Target.createTarget", "Target.activateTarget", "Target.closeTarget", "Target.getTargets":
			return map[string]interface{}{"ok": true}, nil
		default:
			return nil, fmt.Errorf("unexpected method: %s", method)
		}
	})
	defer server.Close()

	uploadPath := filepath.Join(t.TempDir(), "upload.txt")
	if err := os.WriteFile(uploadPath, []byte("upload fixture"), 0644); err != nil {
		t.Fatalf("write upload fixture: %v", err)
	}
	downloadPath := t.TempDir()

	port := server.Listener.Addr().(*net.TCPAddr).Port
	runner := NewCDPTaskRunner(func(id string) (int, error) { return port, nil })
	task := &TaskDef{
		ID:        "typed-m4-primitives",
		ProfileID: "p1",
		Actions: []TaskAction{
			{Type: "select", Target: "#country", Value: `{"value":"US"}`},
			{Type: "dialog", Target: "dismiss", Value: `{"promptText":"stop"}`},
			{Type: "download", Target: downloadPath, Value: `{"eventsEnabled":true}`},
			{Type: "upload", Target: "input[type=file]", Value: fmt.Sprintf(`{"files":[%q]}`, uploadPath)},
			{Type: "iframe", Target: "#frame", Value: `{"action":"click","selector":"#submit"}`},
			{Type: "tab", Target: "new", Value: `{"url":"https://example.com/new"}`},
			{Type: "tab", Target: "activate", Value: `{"targetId":"tab-1"}`},
			{Type: "tab", Target: "close", Value: `{"targetId":"tab-1"}`},
			{Type: "tab", Target: "list"},
		},
	}

	if err := runner.Run(task); err != nil {
		t.Fatalf("typed M4 primitives failed: %v", err)
	}

	wantMethods := []string{
		"Runtime.evaluate",
		"Page.handleJavaScriptDialog",
		"Page.setDownloadBehavior",
		"DOM.getDocument",
		"DOM.querySelector",
		"DOM.setFileInputFiles",
		"Runtime.evaluate",
		"Target.createTarget",
		"Target.activateTarget",
		"Target.closeTarget",
		"Target.getTargets",
	}
	for i, want := range wantMethods {
		select {
		case got := <-received:
			if got.method != want {
				t.Fatalf("command %d method = %q, want %q", i, got.method, want)
			}
			assertTypedPrimitiveParams(t, want, got.params, downloadPath, uploadPath)
		case <-time.After(time.Second):
			t.Fatalf("timed out waiting for command %d %s", i, want)
		}
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

func assertTypedPrimitiveParams(t *testing.T, method string, params map[string]interface{}, downloadPath, uploadPath string) {
	t.Helper()

	switch method {
	case "Runtime.evaluate":
		expression, ok := params["expression"].(string)
		if !ok || expression == "" {
			t.Fatalf("Runtime.evaluate params = %#v", params)
		}
	case "Page.handleJavaScriptDialog":
		if params["accept"] != false || params["promptText"] != "stop" {
			t.Fatalf("dialog params = %#v", params)
		}
	case "Page.setDownloadBehavior":
		if params["behavior"] != "allow" || params["downloadPath"] != downloadPath || params["eventsEnabled"] != true {
			t.Fatalf("download params = %#v", params)
		}
	case "DOM.getDocument":
		if params["depth"] != float64(1) {
			t.Fatalf("get document params = %#v", params)
		}
	case "DOM.querySelector":
		if params["nodeId"] != float64(1) || params["selector"] != "input[type=file]" {
			t.Fatalf("query selector params = %#v", params)
		}
	case "DOM.setFileInputFiles":
		files, ok := params["files"].([]interface{})
		if !ok || len(files) != 1 || files[0] != uploadPath || params["nodeId"] != float64(7) {
			t.Fatalf("upload params = %#v", params)
		}
	case "Target.createTarget":
		if params["url"] != "https://example.com/new" {
			t.Fatalf("create target params = %#v", params)
		}
	case "Target.activateTarget", "Target.closeTarget":
		if params["targetId"] != "tab-1" {
			t.Fatalf("%s params = %#v", method, params)
		}
	case "Target.getTargets":
		if len(params) != 0 {
			t.Fatalf("get targets params = %#v", params)
		}
	default:
		t.Fatalf("unexpected method %q", method)
	}
}
