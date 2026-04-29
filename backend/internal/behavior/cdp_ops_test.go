package behavior

import (
	"encoding/json"
	"fmt"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"
	"time"

	"github.com/gorilla/websocket"
)

func TestSelectPageTargetPrefersNonChromeAvailablePage(t *testing.T) {
	target, ok := selectPageTarget([]cdpTarget{
		{
			Type:                 "page",
			URL:                  "chrome://new-tab-page/",
			WebSocketDebuggerURL: "ws://chrome",
		},
		{
			Type:                 "page",
			URL:                  "https://example.test/app",
			Title:                "App",
			WebSocketDebuggerURL: "ws://app",
		},
	})

	if !ok {
		t.Fatal("expected a target")
	}
	if target.WebSocketDebuggerURL != "ws://app" {
		t.Fatalf("selected %q, want non-chrome target ws://app", target.WebSocketDebuggerURL)
	}
}

func TestSelectPageTargetPrefersActiveNonChromePage(t *testing.T) {
	target, ok := selectPageTarget([]cdpTarget{
		{
			Type:                 "page",
			URL:                  "https://example.test/background",
			WebSocketDebuggerURL: "ws://background",
		},
		{
			Type:                 "page",
			URL:                  "https://example.test/current",
			WebSocketDebuggerURL: "ws://current",
			Active:               true,
		},
	})

	if !ok {
		t.Fatal("expected a target")
	}
	if target.WebSocketDebuggerURL != "ws://current" {
		t.Fatalf("selected %q, want active page ws://current", target.WebSocketDebuggerURL)
	}
}

func TestSelectPageTargetSkipsUnavailableAndFallsBackToInternal(t *testing.T) {
	target, ok := selectPageTarget([]cdpTarget{
		{
			Type: "page",
			URL:  "https://example.test/no-ws",
		},
		{
			Type:                 "page",
			URL:                  "chrome://version/",
			WebSocketDebuggerURL: "ws://chrome",
		},
	})

	if !ok {
		t.Fatal("expected fallback target")
	}
	if target.WebSocketDebuggerURL != "ws://chrome" {
		t.Fatalf("selected %q, want internal fallback ws://chrome", target.WebSocketDebuggerURL)
	}
}

func TestSendCDPCommandWSReadsResultAfterEvents(t *testing.T) {
	conn, closeServer := newCDPTestConn(t, func(t *testing.T, ws *websocket.Conn) {
		var req struct {
			ID int `json:"id"`
		}
		if err := ws.ReadJSON(&req); err != nil {
			t.Errorf("read request: %v", err)
			return
		}
		if err := ws.WriteJSON(map[string]interface{}{
			"method": "Runtime.consoleAPICalled",
		}); err != nil {
			t.Errorf("write event: %v", err)
			return
		}
		if err := ws.WriteJSON(map[string]interface{}{
			"id":     req.ID,
			"result": map[string]interface{}{"ok": true},
		}); err != nil {
			t.Errorf("write response: %v", err)
		}
	})
	defer closeServer()
	defer conn.Close()

	raw, err := sendCDPCommandWS(conn, 7, "Runtime.evaluate", map[string]interface{}{"expression": "1"}, time.Second)
	if err != nil {
		t.Fatalf("sendCDPCommandWS: %v", err)
	}

	var result struct {
		OK bool `json:"ok"`
	}
	if err := json.Unmarshal(raw, &result); err != nil {
		t.Fatalf("unmarshal result: %v", err)
	}
	if !result.OK {
		t.Fatalf("result.OK = false, want true")
	}
}

func TestSendCDPCommandWSReturnsCDPError(t *testing.T) {
	conn, closeServer := newCDPTestConn(t, func(t *testing.T, ws *websocket.Conn) {
		var req struct {
			ID int `json:"id"`
		}
		if err := ws.ReadJSON(&req); err != nil {
			t.Errorf("read request: %v", err)
			return
		}
		if err := ws.WriteJSON(map[string]interface{}{
			"id": req.ID,
			"error": map[string]interface{}{
				"code":    -32000,
				"message": "dispatch failed",
			},
		}); err != nil {
			t.Errorf("write response: %v", err)
		}
	})
	defer closeServer()
	defer conn.Close()

	_, err := sendCDPCommandWS(conn, 8, "Input.dispatchMouseEvent", map[string]interface{}{"type": "mouseMoved"}, time.Second)
	if err == nil {
		t.Fatal("expected CDP error")
	}
	if !strings.Contains(err.Error(), "dispatch failed") || !strings.Contains(err.Error(), "Input.dispatchMouseEvent") {
		t.Fatalf("error = %q, want method and CDP message", err.Error())
	}
}

func newCDPTestConn(t *testing.T, handler func(*testing.T, *websocket.Conn)) (*websocket.Conn, func()) {
	t.Helper()

	upgrader := websocket.Upgrader{}
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		ws, err := upgrader.Upgrade(w, r, nil)
		if err != nil {
			t.Errorf("upgrade websocket: %v", err)
			return
		}
		defer ws.Close()
		handler(t, ws)
	}))

	wsURL := "ws" + strings.TrimPrefix(server.URL, "http")
	conn, _, err := websocket.DefaultDialer.Dial(wsURL, nil)
	if err != nil {
		server.Close()
		t.Fatalf("dial websocket: %v", err)
	}

	return conn, server.Close
}

// TestConnectPageCDP verifies CDP discovery + WebSocket connection
// against a running Chrome instance (debug port 62258).
func TestConnectPageCDP(t *testing.T) {
	debugPort := 62258

	// 1. Verify HTTP /json endpoint is reachable
	resp, err := behaviorHTTPClient.Get(fmt.Sprintf("http://127.0.0.1:%d/json", debugPort))
	if err != nil {
		t.Skipf("Chrome not running on port %d: %v", debugPort, err)
	}
	defer resp.Body.Close()

	var targets []struct {
		Type                 string `json:"type"`
		URL                  string `json:"url"`
		Title                string `json:"title"`
		WebSocketDebuggerURL string `json:"webSocketDebuggerUrl"`
	}
	if err := json.NewDecoder(resp.Body).Decode(&targets); err != nil {
		t.Fatalf("decode /json: %v", err)
	}
	t.Logf("Found %d CDP targets", len(targets))
	for _, tg := range targets {
		t.Logf("  [%s] %s - %s", tg.Type, tg.Title, tg.URL)
	}

	// 2. Connect to first page target via ConnectPageCDP
	conn, err := ConnectPageCDP(debugPort)
	if err != nil {
		t.Fatalf("ConnectPageCDP: %v", err)
	}
	defer conn.Close()
	t.Log("CDP WebSocket connected successfully!")

	// 3. Send Runtime.evaluate to verify the connection works
	cdpConn := &cdpConn{ws: conn}
	result, err := cdpConn.sendCommand("Runtime.evaluate", map[string]interface{}{
		"expression":    "document.title",
		"returnByValue": true,
	})
	if err != nil {
		t.Fatalf("Runtime.evaluate: %v", err)
	}

	var evalResp struct {
		Result struct {
			Value string `json:"value"`
		} `json:"result"`
	}
	if err := json.Unmarshal(result, &evalResp); err != nil {
		t.Fatalf("unmarshal eval result: %v", err)
	}
	t.Logf("Page title: %s", evalResp.Result.Value)

	// 4. Test recording JS injection
	injectJS := `
(function() {
  if (window.__antTestRecorder) return 'already_injected';
  window.__antTestRecorder = true;
  const events = [];
  window.__antTestEvents = events;
  const start = performance.now();

  function record(type, e) {
    const t = Math.round(performance.now() - start);
    const evt = { t, type };
    if (e) {
      if (typeof e.clientX === 'number') { evt.x = e.clientX; evt.y = e.clientY; }
      if (typeof e.button === 'number') evt.btn = e.button;
      if (e.key) evt.key = e.key;
    }
    events.push(evt);
  }

  document.addEventListener('mousemove', function(e) {
    if (performance.now() - (window.__antLastMove||0) < 50) return;
    window.__antLastMove = performance.now();
    record('move', e);
  }, true);
  document.addEventListener('mousedown', function(e) { record('down', e); }, true);
  document.addEventListener('mouseup', function(e) { record('up', e); }, true);
  document.addEventListener('click', function(e) { record('click', e); }, true);
  document.addEventListener('keydown', function(e) {
    record('key', e);
  }, true);
  document.addEventListener('wheel', function(e) { record('scroll', e); }, true);

  return 'injected';
})();
`
	result2, err := cdpConn.sendCommand("Runtime.evaluate", map[string]interface{}{
		"expression":    injectJS,
		"returnByValue": true,
	})
	if err != nil {
		t.Fatalf("inject recording script: %v", err)
	}

	var injectResp struct {
		Result struct {
			Value string `json:"value"`
		} `json:"result"`
	}
	if err := json.Unmarshal(result2, &injectResp); err != nil {
		t.Fatalf("unmarshal inject result: %v", err)
	}
	t.Logf("Injection result: %s", injectResp.Result.Value)

	if injectResp.Result.Value != "injected" && injectResp.Result.Value != "already_injected" {
		t.Errorf("unexpected injection result: %s", injectResp.Result.Value)
	}

	// 5. Retrieve recorded events (should be empty or have minimal events)
	time.Sleep(500 * time.Millisecond)

	retrieveJS := "JSON.stringify(window.__antTestEvents || [])"
	result3, err := cdpConn.sendCommand("Runtime.evaluate", map[string]interface{}{
		"expression":    retrieveJS,
		"returnByValue": true,
	})
	if err != nil {
		t.Fatalf("retrieve events: %v", err)
	}

	var retrieveResp struct {
		Result struct {
			Value string `json:"value"`
		} `json:"result"`
	}
	if err := json.Unmarshal(result3, &retrieveResp); err != nil {
		t.Fatalf("unmarshal retrieve result: %v", err)
	}
	t.Logf("Recorded events: %s", retrieveResp.Result.Value)

	// Cleanup
	cleanupJS := "delete window.__antTestRecorder; delete window.__antTestEvents; delete window.__antLastMove;"
	cdpConn.sendCommand("Runtime.evaluate", map[string]interface{}{
		"expression":    cleanupJS,
		"returnByValue": false,
	})

	t.Log("CDP recording test PASSED!")
}
