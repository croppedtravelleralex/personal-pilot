package behavior

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"github.com/gorilla/websocket"

	"personal-pilot/backend/internal/behavior/humanize"
)

func TestCDPExecutorSendCommandDispatchesDialogDownloadAndUploadPrimitives(t *testing.T) {
	type receivedCommand struct {
		ID     int             `json:"id"`
		Method string          `json:"method"`
		Params json.RawMessage `json:"params"`
	}

	received := make(chan receivedCommand, 3)
	conn, closeServer := newCDPTestConn(t, func(t *testing.T, ws *websocket.Conn) {
		for i := 0; i < 3; i++ {
			var req receivedCommand
			if err := ws.ReadJSON(&req); err != nil {
				t.Errorf("read request %d: %v", i, err)
				return
			}
			received <- req
			if err := ws.WriteJSON(map[string]interface{}{
				"id":     req.ID,
				"result": map[string]interface{}{"ok": true},
			}); err != nil {
				t.Errorf("write response %d: %v", i, err)
				return
			}
		}
	})
	defer closeServer()

	executor := NewCDPExecutor(conn, humanize.ConfigForLevel(humanize.LevelNone))
	defer executor.Close()

	uploadPath := filepath.Join(t.TempDir(), "upload.txt")
	if err := os.WriteFile(uploadPath, []byte("upload fixture"), 0644); err != nil {
		t.Fatalf("write upload fixture: %v", err)
	}
	downloadPath := t.TempDir()

	commands := []struct {
		method string
		params map[string]interface{}
	}{
		{
			method: "Page.handleJavaScriptDialog",
			params: map[string]interface{}{"accept": true, "promptText": "ok"},
		},
		{
			method: "Page.setDownloadBehavior",
			params: map[string]interface{}{"behavior": "allow", "downloadPath": downloadPath},
		},
		{
			method: "DOM.setFileInputFiles",
			params: map[string]interface{}{"nodeId": 7, "files": []string{uploadPath}},
		},
	}

	for _, command := range commands {
		raw, err := executor.sendCommand(command.method, command.params)
		if err != nil {
			t.Fatalf("send %s: %v", command.method, err)
		}
		var result struct {
			OK bool `json:"ok"`
		}
		if err := json.Unmarshal(raw, &result); err != nil {
			t.Fatalf("decode %s result: %v", command.method, err)
		}
		if !result.OK {
			t.Fatalf("%s result OK = false, want true", command.method)
		}
	}

	for i, want := range commands {
		select {
		case got := <-received:
			if got.Method != want.method {
				t.Fatalf("command %d method = %q, want %q", i, got.Method, want.method)
			}
			params := map[string]interface{}{}
			if err := json.Unmarshal(got.Params, &params); err != nil {
				t.Fatalf("decode command %d params: %v", i, err)
			}
			assertPrimitiveParams(t, want.method, params, downloadPath, uploadPath)
		case <-time.After(time.Second):
			t.Fatalf("timed out waiting for command %d %s", i, want.method)
		}
	}
}

func TestCDPExecutorResolveSelector(t *testing.T) {
	executor := &CDPExecutor{}

	tests := []struct {
		name     string
		selector string
		byText   string
		byXPath  string
		want     string
		wantErr  bool
	}{
		{name: "css", selector: "#submit", want: "#submit"},
		{name: "text", byText: "Submit", want: `//*[text()="Submit"]`},
		{name: "xpath", byXPath: `//*[@data-testid="submit"]`, want: `//*[@data-testid="submit"]`},
		{name: "missing", wantErr: true},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got, err := executor.ResolveSelector(tt.selector, tt.byText, tt.byXPath)
			if tt.wantErr {
				if err == nil {
					t.Fatal("expected error")
				}
				return
			}
			if err != nil {
				t.Fatalf("ResolveSelector: %v", err)
			}
			if got != tt.want {
				t.Fatalf("ResolveSelector = %q, want %q", got, tt.want)
			}
		})
	}
}

func TestCDPExecutorCaptureScreenshotParsesDirectCDPResult(t *testing.T) {
	received := make(chan string, 1)
	conn, closeServer := newCDPTestConn(t, func(t *testing.T, ws *websocket.Conn) {
		var req struct {
			ID     int    `json:"id"`
			Method string `json:"method"`
		}
		if err := ws.ReadJSON(&req); err != nil {
			t.Errorf("read screenshot request: %v", err)
			return
		}
		received <- req.Method
		if err := ws.WriteJSON(map[string]interface{}{
			"id":     req.ID,
			"result": map[string]interface{}{"data": "cG5n"},
		}); err != nil {
			t.Errorf("write screenshot response: %v", err)
		}
	})
	defer closeServer()

	executor := NewCDPExecutor(conn, humanize.ConfigForLevel(humanize.LevelNone))
	defer executor.Close()

	got, err := executor.CaptureScreenshot()
	if err != nil {
		t.Fatalf("CaptureScreenshot: %v", err)
	}
	if got != "data:image/png;base64,cG5n" {
		t.Fatalf("screenshot = %q, want data URL", got)
	}
	select {
	case method := <-received:
		if method != "Page.captureScreenshot" {
			t.Fatalf("method = %q, want Page.captureScreenshot", method)
		}
	case <-time.After(time.Second):
		t.Fatal("timed out waiting for screenshot command")
	}
}

func TestCDPExecutorExecuteMutatedActionWithResultReturnsTypedPayload(t *testing.T) {
	tests := []struct {
		name       string
		action     humanize.MutatedAction
		wantMethod string
		rawResult  string
		read       func(MutatedActionExecutionResult) *string
		want       string
	}{
		{
			name:       "screenshot",
			action:     humanize.MutatedAction{Type: humanize.MutatedScreenshot},
			wantMethod: "Page.captureScreenshot",
			rawResult:  `{"data":"cG5n"}`,
			read:       func(result MutatedActionExecutionResult) *string { return result.ScreenshotDataURL },
			want:       "data:image/png;base64,cG5n",
		},
		{
			name:       "html",
			action:     humanize.MutatedAction{Type: humanize.MutatedGetHtml, Selector: "#main"},
			wantMethod: "Runtime.evaluate",
			rawResult:  `{"result":{"type":"string","value":"<main>ok</main>"}}`,
			read:       func(result MutatedActionExecutionResult) *string { return result.HTML },
			want:       "<main>ok</main>",
		},
		{
			name:       "text",
			action:     humanize.MutatedAction{Type: humanize.MutatedGetText, Selector: "#status"},
			wantMethod: "Runtime.evaluate",
			rawResult:  `{"result":{"type":"string","value":"ready"}}`,
			read:       func(result MutatedActionExecutionResult) *string { return result.Text },
			want:       "ready",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			executor := &CDPExecutor{
				sleepFn: func(time.Duration) {},
				sendCommandHook: func(method string, _ interface{}) ([]byte, error) {
					if method != tt.wantMethod {
						t.Fatalf("method = %q, want %q", method, tt.wantMethod)
					}
					return []byte(tt.rawResult), nil
				},
			}

			result, err := executor.ExecuteMutatedActionWithResult(tt.action)
			if err != nil {
				t.Fatalf("ExecuteMutatedActionWithResult: %v", err)
			}
			if result.ActionType != tt.action.Type {
				t.Fatalf("ActionType = %d, want %d", result.ActionType, tt.action.Type)
			}
			payload := tt.read(result)
			if payload == nil || *payload != tt.want {
				t.Fatalf("payload = %v, want %q", payload, tt.want)
			}
		})
	}
}

func TestCDPExecutorExecuteMutatedActionRejectsUnsupportedType(t *testing.T) {
	executor := &CDPExecutor{sleepFn: func(time.Duration) {}}
	action := humanize.MutatedAction{Type: humanize.MutatedActionType(999)}

	if _, err := executor.ExecuteMutatedActionWithResult(action); err == nil || !strings.Contains(err.Error(), "unsupported mutated action type 999") {
		t.Fatalf("ExecuteMutatedActionWithResult error = %v", err)
	}
	if err := executor.ExecuteMutatedAction(action); err == nil || !strings.Contains(err.Error(), "unsupported mutated action type 999") {
		t.Fatalf("ExecuteMutatedAction error = %v", err)
	}
}

func TestCDPExecutorMousePointerOverlayDispatchesInstallAndCleanup(t *testing.T) {
	type receivedCommand struct {
		ID     int    `json:"id"`
		Method string `json:"method"`
		Params struct {
			Expression string `json:"expression"`
		} `json:"params"`
	}
	received := make(chan receivedCommand, 2)
	conn, closeServer := newCDPTestConn(t, func(t *testing.T, ws *websocket.Conn) {
		for i := 0; i < 2; i++ {
			var req receivedCommand
			if err := ws.ReadJSON(&req); err != nil {
				t.Errorf("read overlay request %d: %v", i, err)
				return
			}
			received <- req
			if err := ws.WriteJSON(map[string]interface{}{
				"id":     req.ID,
				"result": map[string]interface{}{"result": map[string]interface{}{"value": true}},
			}); err != nil {
				t.Errorf("write overlay response %d: %v", i, err)
				return
			}
		}
	})
	defer closeServer()

	executor := NewCDPExecutor(conn, humanize.ConfigForLevel(humanize.LevelNone))
	defer executor.Close()

	if err := executor.ShowMousePointerOverlay(); err != nil {
		t.Fatalf("ShowMousePointerOverlay: %v", err)
	}
	if err := executor.HideMousePointerOverlay(); err != nil {
		t.Fatalf("HideMousePointerOverlay: %v", err)
	}

	var commands []receivedCommand
	for i := 0; i < 2; i++ {
		select {
		case command := <-received:
			commands = append(commands, command)
		case <-time.After(time.Second):
			t.Fatalf("timed out waiting for overlay command %d", i)
		}
	}
	if commands[0].Method != "Runtime.evaluate" || !strings.Contains(commands[0].Params.Expression, "__personalPilotPointerOverlay") {
		t.Fatalf("install command = %+v", commands[0])
	}
	if commands[1].Method != "Runtime.evaluate" || !strings.Contains(commands[1].Params.Expression, "destroy") {
		t.Fatalf("cleanup command = %+v", commands[1])
	}
}

func TestExtractResultValueParsesRuntimeEvaluatePrimitives(t *testing.T) {
	tests := []struct {
		name string
		raw  string
		want string
	}{
		{name: "nested string", raw: `{"result":{"type":"string","value":"ok"}}`, want: "ok"},
		{name: "nested bool true", raw: `{"result":{"type":"boolean","value":true}}`, want: "true"},
		{name: "nested bool false", raw: `{"result":{"type":"boolean","value":false}}`, want: "false"},
		{name: "legacy string", raw: `{"value":"legacy"}`, want: "legacy"},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if got := extractResultValue([]byte(tt.raw)); got != tt.want {
				t.Fatalf("extractResultValue = %q, want %q", got, tt.want)
			}
		})
	}
}

func TestParseBoundsResultParsesRuntimeEvaluateObjectValue(t *testing.T) {
	raw := []byte(`{"result":{"type":"object","value":{"x1":10,"y1":20,"x2":110,"y2":70,"width":100,"height":50}}}`)

	got, err := parseBoundsResult(raw)
	if err != nil {
		t.Fatalf("parseBoundsResult: %v", err)
	}
	if got.X1 != 10 || got.Y1 != 20 || got.X2 != 110 || got.Y2 != 70 || got.Width != 100 || got.Height != 50 {
		t.Fatalf("bounds = %+v, want x1=10 y1=20 x2=110 y2=70 width=100 height=50", got)
	}
}

func TestParseBoundsResultRoundsRuntimeEvaluateFloatCoordinates(t *testing.T) {
	raw := []byte(`{"result":{"type":"object","value":{"x1":10.4,"y1":20.5,"x2":110.49,"y2":70.51,"width":100.2,"height":50.8}}}`)

	got, err := parseBoundsResult(raw)
	if err != nil {
		t.Fatalf("parseBoundsResult: %v", err)
	}
	if got.X1 != 10 || got.Y1 != 21 || got.X2 != 110 || got.Y2 != 71 || got.Width != 100 || got.Height != 51 {
		t.Fatalf("bounds = %+v, want rounded integer bounds", got)
	}
}

func TestParseBoundsResultReturnsElementNotFoundForRuntimeNull(t *testing.T) {
	raw := []byte(`{"result":{"type":"object","value":null}}`)

	if _, err := parseBoundsResult(raw); err == nil {
		t.Fatal("expected element not found error")
	}
}

func assertPrimitiveParams(t *testing.T, method string, params map[string]interface{}, downloadPath, uploadPath string) {
	t.Helper()

	switch method {
	case "Page.handleJavaScriptDialog":
		if params["accept"] != true || params["promptText"] != "ok" {
			t.Fatalf("dialog params = %#v", params)
		}
	case "Page.setDownloadBehavior":
		if params["behavior"] != "allow" || params["downloadPath"] != downloadPath {
			t.Fatalf("download params = %#v", params)
		}
	case "DOM.setFileInputFiles":
		files, ok := params["files"].([]interface{})
		if !ok || len(files) != 1 || files[0] != uploadPath || params["nodeId"] != float64(7) {
			t.Fatalf("upload params = %#v", params)
		}
	default:
		t.Fatalf("unexpected method %q", method)
	}
}

func TestDispatchMouseEventRetriesWithoutOverlayRoundTrip(t *testing.T) {
	calls := 0
	methods := make([]string, 0, 2)
	sleeps := make([]time.Duration, 0, 1)
	executor := &CDPExecutor{
		sendCommandHook: func(method string, _ interface{}) ([]byte, error) {
			calls++
			methods = append(methods, method)
			if calls == 1 {
				return nil, fmt.Errorf("temporary timeout")
			}
			return []byte(`{}`), nil
		},
		sleepFn: func(delay time.Duration) { sleeps = append(sleeps, delay) },
	}
	if err := executor.dispatchMouseEvent("mousePressed", 10, 20, "left", 1); err != nil {
		t.Fatalf("dispatchMouseEvent: %v", err)
	}
	if calls != 2 {
		t.Fatalf("calls=%d, want 2", calls)
	}
	if len(sleeps) != 1 || sleeps[0] != 200*time.Millisecond {
		t.Fatalf("retry sleeps=%v", sleeps)
	}
	for _, method := range methods {
		if method != "Input.dispatchMouseEvent" {
			t.Fatalf("unexpected overlay/extra command: %v", methods)
		}
	}
}

func TestMouseMoveStepCountAdaptsToRTT(t *testing.T) {
	fast := mouseMoveStepCount(1200, humanize.LevelHigh, 20*time.Millisecond)
	slow := mouseMoveStepCount(1200, humanize.LevelHigh, 600*time.Millisecond)
	if fast > 32 {
		t.Fatalf("fast steps=%d, want <=32", fast)
	}
	if slow >= fast || slow > 12 {
		t.Fatalf("slow steps=%d fast=%d", slow, fast)
	}
}

func TestClickFallsBackToOSAfterCDPRetries(t *testing.T) {
	osCalls := 0
	executor := &CDPExecutor{
		currentX: 42,
		currentY: 84,
		sendCommandHook: func(method string, _ interface{}) ([]byte, error) {
			if method == "Input.dispatchMouseEvent" {
				return nil, fmt.Errorf("persistent timeout")
			}
			return []byte(`{}`), nil
		},
		sleepFn: func(time.Duration) {},
		OSClickAtFallback: func(x, y float64) error {
			osCalls++
			if x != 42 || y != 84 {
				t.Fatalf("os fallback coords=(%v,%v)", x, y)
			}
			return nil
		},
	}
	if err := executor.Click(); err != nil {
		t.Fatalf("Click: %v", err)
	}
	if osCalls != 1 {
		t.Fatalf("osCalls=%d, want 1", osCalls)
	}
	if executor.ClickFallback != "os" {
		t.Fatalf("ClickFallback=%q, want os", executor.ClickFallback)
	}
}

func TestMouseMoveFittsUsesHumanizeWhenLevelActive(t *testing.T) {
	moves := 0
	cfg := humanize.HumanizationConfig{}
	cfg.FromLevel(humanize.LevelHigh)
	cfg.Click.Seed = 99
	executor := &CDPExecutor{
		middleware: humanize.NewBehavioralMutationMiddleware(cfg),
		level:      cfg.Level,
		sendCommandHook: func(method string, params interface{}) ([]byte, error) {
			if method == "Input.dispatchMouseEvent" {
				moves++
				return []byte(`{}`), nil
			}
			return []byte(`{}`), nil
		},
		sleepFn: func(time.Duration) {},
	}
	if err := executor.mouseMove(0, 0, 400, 300); err != nil {
		t.Fatalf("mouseMove: %v", err)
	}
	if moves < 4 {
		t.Fatalf("fitts moves=%d, want >=4", moves)
	}
}
