package behavior

import (
	"encoding/json"
	"os"
	"path/filepath"
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
