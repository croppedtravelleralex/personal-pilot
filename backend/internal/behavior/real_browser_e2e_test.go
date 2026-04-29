package behavior

import (
	"encoding/json"
	"fmt"
	"net"
	"net/http"
	"net/http/httptest"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"github.com/gorilla/websocket"
)

func TestRealFingerprintBrowserRecordPlayback(t *testing.T) {
	if os.Getenv("ANT_RECORDING_E2E") != "1" {
		t.Skip("set ANT_RECORDING_E2E=1 to run the real fingerprint browser recording E2E test")
	}

	chromePath := fingerprintChromePath(t)
	debugPort := freeTCPPort(t)
	userDataDir := t.TempDir()
	server := httptest.NewServer(http.HandlerFunc(recordingE2EPage))
	t.Cleanup(server.Close)

	cmd := exec.Command(chromePath,
		fmt.Sprintf("--remote-debugging-port=%d", debugPort),
		"--user-data-dir="+userDataDir,
		"--disable-sync",
		"--no-first-run",
		"--no-default-browser-check",
		"--disable-default-apps",
		"--window-size=1100,900",
		server.URL,
	)
	if err := cmd.Start(); err != nil {
		t.Fatalf("start fingerprint browser: %v", err)
	}
	t.Cleanup(func() {
		closeBrowser(t, debugPort)
		_ = cmd.Process.Kill()
		_, _ = cmd.Process.Wait()
	})

	waitForCDP(t, debugPort)
	conn := connectE2ECDP(t, debugPort)
	defer conn.Close()
	sender := newE2ESender(conn)
	waitForPageReady(t, sender, server.URL)

	rec := NewRecorder()
	if err := rec.StartRecording(debugPort); err != nil {
		t.Fatalf("start recording: %v", err)
	}

	points := readE2EPoints(t, sender)
	dispatchClick(t, sender, points.ButtonX, points.ButtonY)
	dispatchClick(t, sender, points.InputX, points.InputY)
	dispatchText(t, sender, "abc")
	dispatchWheel(t, sender, points.ScrollX, points.ScrollY, 420)

	if _, err := sender.send("Page.navigate", map[string]interface{}{"url": server.URL + "?phase=after-refresh"}); err != nil {
		t.Fatalf("navigate during recording: %v", err)
	}
	waitForPageReady(t, sender, server.URL)
	points = readE2EPoints(t, sender)
	dispatchClick(t, sender, points.ButtonX, points.ButtonY)
	if clicks := evalString(t, sender, `String(window.tapCount || 0)`); clicks != "1" {
		t.Fatalf("post-navigation CDP click did not reach page, tapCount=%s", clicks)
	}

	recording, err := rec.StopRecording("real fingerprint browser e2e")
	if err != nil {
		t.Fatalf("stop recording: %v", err)
	}
	assertRecordedE2EEvents(t, recording)

	resetE2EPage(t, sender)
	engine := NewPlaybackEngine(recording, VariationConfig{})
	if err := engine.Play(t.Context(), debugPort); err != nil {
		t.Fatalf("playback: %v", err)
	}

	state := readE2EState(t, sender)
	if state.Clicks != 2 {
		t.Fatalf("playback click count = %d, want 2; duplicate click playback likely regressed", state.Clicks)
	}
	if state.InputValue != "abc" {
		t.Fatalf("playback input value = %q, want abc", state.InputValue)
	}
	if state.ScrollTop <= 0 {
		t.Fatalf("playback scrollTop = %d, want > 0", state.ScrollTop)
	}
}

func fingerprintChromePath(t *testing.T) string {
	t.Helper()
	if explicit := strings.TrimSpace(os.Getenv("ANT_RECORDING_E2E_CHROME")); explicit != "" {
		if _, err := os.Stat(explicit); err != nil {
			t.Fatalf("ANT_RECORDING_E2E_CHROME not usable: %v", err)
		}
		return explicit
	}

	candidates := []string{
		filepath.Join("..", "..", "..", "chrome", "fingerprint-chromium-139.0.7258.154", "chrome.exe"),
		filepath.Join("..", "..", "..", "chrome", "fingerprint-chromium-142.0.7444.175", "chrome.exe"),
		filepath.Join("..", "..", "..", "chrome", "fingerprint-chromium-144.0.7559.132", "chrome.exe"),
	}
	for _, candidate := range candidates {
		abs, err := filepath.Abs(candidate)
		if err == nil {
			if _, statErr := os.Stat(abs); statErr == nil {
				return abs
			}
		}
	}
	t.Fatal("no fingerprint chromium chrome.exe found; set ANT_RECORDING_E2E_CHROME")
	return ""
}

func freeTCPPort(t *testing.T) int {
	t.Helper()
	ln, err := net.Listen("tcp", "127.0.0.1:0")
	if err != nil {
		t.Fatalf("allocate TCP port: %v", err)
	}
	defer ln.Close()
	return ln.Addr().(*net.TCPAddr).Port
}

func waitForCDP(t *testing.T, debugPort int) {
	t.Helper()
	url := fmt.Sprintf("http://127.0.0.1:%d/json/version", debugPort)
	deadline := time.Now().Add(15 * time.Second)
	for time.Now().Before(deadline) {
		resp, err := behaviorHTTPClient.Get(url)
		if err == nil {
			_ = resp.Body.Close()
			return
		}
		time.Sleep(150 * time.Millisecond)
	}
	t.Fatalf("CDP not ready on port %d", debugPort)
}

func closeBrowser(t *testing.T, debugPort int) {
	t.Helper()
	conn, err := ConnectPageCDP(debugPort)
	if err != nil {
		return
	}
	defer conn.Close()
	_, _ = sendCDPCommandWS(conn, 9999, "Browser.close", nil, 2*time.Second)
}

func connectE2ECDP(t *testing.T, debugPort int) *websocket.Conn {
	t.Helper()
	conn, err := ConnectPageCDP(debugPort)
	if err != nil {
		t.Fatalf("connect page CDP: %v", err)
	}
	return conn
}

type e2eSender struct {
	conn *websocket.Conn
	id   int
}

func newE2ESender(conn *websocket.Conn) *e2eSender {
	return &e2eSender{conn: conn}
}

func (s *e2eSender) send(method string, params map[string]interface{}) (json.RawMessage, error) {
	s.id++
	return sendCDPCommandWS(s.conn, s.id, method, params, 5*time.Second)
}

func waitForPageReady(t *testing.T, sender *e2eSender, wantPrefix string) {
	t.Helper()
	deadline := time.Now().Add(10 * time.Second)
	for time.Now().Before(deadline) {
		state := evalString(t, sender, `JSON.stringify({ ready: document.readyState, url: location.href })`)
		var parsed struct {
			Ready string `json:"ready"`
			URL   string `json:"url"`
		}
		if json.Unmarshal([]byte(state), &parsed) == nil &&
			parsed.Ready == "complete" &&
			strings.HasPrefix(parsed.URL, wantPrefix) {
			return
		}
		time.Sleep(100 * time.Millisecond)
	}
	t.Fatalf("page did not become ready for %s", wantPrefix)
}

type e2ePoints struct {
	ButtonX float64 `json:"buttonX"`
	ButtonY float64 `json:"buttonY"`
	InputX  float64 `json:"inputX"`
	InputY  float64 `json:"inputY"`
	ScrollX float64 `json:"scrollX"`
	ScrollY float64 `json:"scrollY"`
}

func readE2EPoints(t *testing.T, sender *e2eSender) e2ePoints {
	t.Helper()
	raw := evalString(t, sender, `JSON.stringify((function() {
  function center(id) {
    const r = document.getElementById(id).getBoundingClientRect();
    return { x: r.left + r.width / 2, y: r.top + r.height / 2 };
  }
  const button = center('tap');
  const input = center('name');
  const scroll = center('scrollbox');
  return { buttonX: button.x, buttonY: button.y, inputX: input.x, inputY: input.y, scrollX: scroll.x, scrollY: scroll.y };
})())`)
	var points e2ePoints
	if err := json.Unmarshal([]byte(raw), &points); err != nil {
		t.Fatalf("parse points %q: %v", raw, err)
	}
	return points
}

func dispatchClick(t *testing.T, sender *e2eSender, x, y float64) {
	t.Helper()
	events := []string{"mouseMoved", "mousePressed", "mouseReleased"}
	for _, typ := range events {
		params := map[string]interface{}{
			"type":       typ,
			"x":          x,
			"y":          y,
			"button":     "left",
			"clickCount": 1,
		}
		if typ == "mouseMoved" {
			params["button"] = "none"
			params["clickCount"] = 0
		}
		if _, err := sender.send("Input.dispatchMouseEvent", params); err != nil {
			t.Fatalf("dispatch %s: %v", typ, err)
		}
	}
	time.Sleep(100 * time.Millisecond)
}

func dispatchText(t *testing.T, sender *e2eSender, text string) {
	t.Helper()
	for _, ch := range text {
		key := string(ch)
		if _, err := sender.send("Input.dispatchKeyEvent", map[string]interface{}{"type": "rawKeyDown", "key": key, "text": key}); err != nil {
			t.Fatalf("key down %q: %v", key, err)
		}
		if _, err := sender.send("Input.dispatchKeyEvent", map[string]interface{}{"type": "char", "key": key, "text": key}); err != nil {
			t.Fatalf("key char %q: %v", key, err)
		}
		if _, err := sender.send("Input.dispatchKeyEvent", map[string]interface{}{"type": "keyUp", "key": key}); err != nil {
			t.Fatalf("key up %q: %v", key, err)
		}
	}
	time.Sleep(100 * time.Millisecond)
}

func dispatchWheel(t *testing.T, sender *e2eSender, x, y, deltaY float64) {
	t.Helper()
	if _, err := sender.send("Input.dispatchMouseEvent", map[string]interface{}{
		"type":   "mouseWheel",
		"x":      x,
		"y":      y,
		"deltaX": 0,
		"deltaY": deltaY,
	}); err != nil {
		t.Fatalf("dispatch wheel: %v", err)
	}
	time.Sleep(150 * time.Millisecond)
}

func assertRecordedE2EEvents(t *testing.T, recording *Recording) {
	t.Helper()
	counts := map[string]int{}
	scrollTargetPath := false
	for _, evt := range recording.Events {
		counts[evt.Type]++
		if evt.Type == "scroll" && evt.TargetPath != "" {
			scrollTargetPath = true
		}
	}
	if counts["click"] != 0 {
		t.Fatalf("recording contains derived click events: %d", counts["click"])
	}
	t.Logf("recording event counts: %v", counts)
	if counts["down"] < 2 || counts["up"] < 2 {
		t.Fatalf("recording down/up counts = %d/%d, want at least 2/2", counts["down"], counts["up"])
	}
	if counts["key"] == 0 && counts["input"] == 0 {
		t.Fatalf("recording missing keyboard/input events: %v", counts)
	}
	if counts["scroll"] == 0 {
		t.Fatalf("recording missing scroll events: %v", counts)
	}
	if !scrollTargetPath {
		t.Fatalf("recording scroll events missing targetPath")
	}
	if recording.StartURL == "" || recording.CurrentURL == "" || recording.Title == "" {
		t.Fatalf("recording metadata incomplete: start=%q current=%q title=%q", recording.StartURL, recording.CurrentURL, recording.Title)
	}
	if !strings.Contains(recording.CurrentURL, "phase=after-refresh") {
		t.Fatalf("recording did not survive navigation, currentUrl=%q", recording.CurrentURL)
	}
}

type e2eState struct {
	Clicks     int    `json:"clicks"`
	InputValue string `json:"inputValue"`
	ScrollTop  int    `json:"scrollTop"`
}

func resetE2EPage(t *testing.T, sender *e2eSender) {
	t.Helper()
	_ = evalString(t, sender, `(function() {
  window.tapCount = 0;
  document.getElementById('tap-count').textContent = '0';
  document.getElementById('name').value = '';
  document.getElementById('scrollbox').scrollTop = 0;
  return 'ok';
})()`)
}

func readE2EState(t *testing.T, sender *e2eSender) e2eState {
	t.Helper()
	raw := evalString(t, sender, `JSON.stringify({
  clicks: window.tapCount || 0,
  inputValue: document.getElementById('name').value,
  scrollTop: document.getElementById('scrollbox').scrollTop
})`)
	var state e2eState
	if err := json.Unmarshal([]byte(raw), &state); err != nil {
		t.Fatalf("parse state %q: %v", raw, err)
	}
	return state
}

func evalString(t *testing.T, sender *e2eSender, expression string) string {
	t.Helper()
	raw, err := sender.send("Runtime.evaluate", map[string]interface{}{
		"expression":    expression,
		"returnByValue": true,
	})
	if err != nil {
		t.Fatalf("evaluate %q: %v", expression, err)
	}
	var response struct {
		Result struct {
			Value string `json:"value"`
		} `json:"result"`
	}
	if err := json.Unmarshal(raw, &response); err != nil {
		t.Fatalf("parse eval response: %v", err)
	}
	return response.Result.Value
}

func recordingE2EPage(w http.ResponseWriter, r *http.Request) {
	w.Header().Set("Content-Type", "text/html; charset=utf-8")
	_, _ = w.Write([]byte(`<!doctype html>
<html>
<head>
  <meta charset="utf-8">
  <title>Ant Recording E2E</title>
  <style>
    body { font-family: system-ui, sans-serif; margin: 32px; }
    button, input { font-size: 16px; padding: 8px 12px; margin: 8px 0; display: block; }
    #scrollbox { width: 360px; height: 160px; overflow: auto; border: 1px solid #888; margin-top: 16px; }
    #scrollcontent { height: 900px; padding: 12px; background: linear-gradient(#fff, #d8eef7); }
  </style>
</head>
<body>
  <button id="tap" onclick="window.tapCount=(window.tapCount||0)+1;document.getElementById('tap-count').textContent=String(window.tapCount)">Tap</button>
  <div>Clicks: <strong id="tap-count">0</strong></div>
  <input id="name" autocomplete="off" placeholder="type here">
  <div id="scrollbox"><div id="scrollcontent">Scroll target<br><br><br><br><br><br><br><br><br><br>bottom</div></div>
  <script>
    window.tapCount = 0;
  </script>
</body>
</html>`))
}
