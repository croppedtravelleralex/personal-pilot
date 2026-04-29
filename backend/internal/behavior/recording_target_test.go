package behavior

import (
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"strings"
	"sync"
	"testing"
	"time"

	"github.com/gorilla/websocket"
)

func TestTargetWarningsOnlyReportUnconnectableTargets(t *testing.T) {
	start := cdpTarget{
		ID:                   "target-main",
		Type:                 "page",
		URL:                  "https://example.test/main",
		WebSocketDebuggerURL: "ws://main",
	}
	iframe := cdpTarget{
		ID:                   "target-frame",
		Type:                 "iframe",
		URL:                  "https://example.test/frame",
		WebSocketDebuggerURL: "ws://frame",
	}
	missingWS := cdpTarget{
		ID:   "target-missing-ws",
		Type: "iframe",
		URL:  "https://example.test/missing-ws",
	}
	internal := cdpTarget{
		ID:   "target-internal",
		Type: "iframe",
		URL:  "chrome://new-tab-page/",
	}
	stop := []cdpTarget{
		start,
		iframe,
		missingWS,
		internal,
		{
			ID:                   "target-new-tab",
			Type:                 "page",
			URL:                  "https://example.test/new-tab",
			WebSocketDebuggerURL: "ws://new-tab",
			Active:               true,
		},
	}

	warnings := recordingTargetWarnings(start, []cdpTarget{start, iframe}, stop)
	joined := strings.Join(warnings, " ")
	for _, want := range []string{
		"target-missing-ws",
		"missing webSocketDebuggerUrl",
	} {
		if !strings.Contains(joined, want) {
			t.Fatalf("target warnings = %q, want %q", joined, want)
		}
	}
	for _, blocked := range []string{
		"captures only selected target",
		"not fully synchronized yet",
		"multiple CDP page targets",
		"selected page target changed",
		"target-internal",
	} {
		if strings.Contains(joined, blocked) {
			t.Fatalf("target warnings = %q, must not report %q", joined, blocked)
		}
	}
}

func TestRecordablePageTargetsIncludeIframeTargets(t *testing.T) {
	targets := recordablePageTargets([]cdpTarget{
		{
			ID:                   "page",
			Type:                 "page",
			URL:                  "https://example.test/page",
			WebSocketDebuggerURL: "ws://page",
		},
		{
			ID:                   "frame",
			Type:                 "iframe",
			URL:                  "https://example.test/frame",
			WebSocketDebuggerURL: "ws://frame",
		},
		{
			ID:                   "internal-frame",
			Type:                 "iframe",
			URL:                  "chrome://new-tab-page/",
			WebSocketDebuggerURL: "ws://internal-frame",
		},
		{
			ID:   "missing-ws",
			Type: "iframe",
			URL:  "https://example.test/missing-ws",
		},
		{
			ID:                   "worker",
			Type:                 "worker",
			URL:                  "https://example.test/worker",
			WebSocketDebuggerURL: "ws://worker",
		},
	})

	if len(targets) != 2 {
		t.Fatalf("recordable targets = %#v, want page + iframe only", targets)
	}
	if targets[0].ID != "page" || targets[1].ID != "frame" {
		t.Fatalf("recordable target order = %#v, want page then frame", targets)
	}
}

func TestTargetWarningDescriptionIsRecordingMetadata(t *testing.T) {
	description := recordingWarningDescription([]string{"multiple CDP page targets detected at stop"})
	recording := newRecordingFromSnapshot("target warning", description, recordingSnapshot{
		Events: []RecordedEvent{{T: 10, Type: "down"}},
		Meta:   recordingMetaState{ViewportW: 800, ViewportH: 600},
	})

	if recording.Description == "" {
		t.Fatal("target warnings must be persisted in recording metadata")
	}
	if !strings.Contains(recording.Description, "recording metadata warning") {
		t.Fatalf("description = %q, want metadata warning marker", recording.Description)
	}
}

func TestRecordingSynchronizesNewPageTargetSnapshots(t *testing.T) {
	server, state := newTargetSyncCDPServer(t)
	defer server.Close()

	rec := NewRecorder()
	if err := rec.StartRecording(serverPort(t, server.URL)); err != nil {
		t.Fatalf("StartRecording: %v", err)
	}
	defer func() {
		if rec.IsRecording() {
			_, _ = rec.StopRecording("cleanup")
		}
	}()

	state.setExposeNew(true)
	if !state.waitForCommand("new", "Runtime.evaluate", func(params map[string]interface{}) bool {
		return params["expression"] == RecordingInjectJS
	}, 3*time.Second) {
		t.Fatalf("new page target was not injected; commands=%v", state.commandsFor("new"))
	}

	recording, err := rec.StopRecording("multi target")
	if err != nil {
		t.Fatalf("StopRecording: %v", err)
	}

	if !state.hasCommand("new", "Page.addScriptToEvaluateOnNewDocument", nil) {
		t.Fatalf("new page target did not register new-document injection; commands=%v", state.commandsFor("new"))
	}
	if len(recording.Events) != 2 {
		t.Fatalf("merged events = %#v, want 2 events from both targets", recording.Events)
	}
	if recording.Events[0].Type != "down" || recording.Events[1].Type != "input" {
		t.Fatalf("merged events order = %#v, want primary down before new target input", recording.Events)
	}
	if recording.Events[1].T < recording.Events[0].T {
		t.Fatalf("merged events not sorted by time: %#v", recording.Events)
	}
	if strings.Contains(recording.Description, "not fully synchronized") ||
		strings.Contains(recording.Description, "captures only selected target") {
		t.Fatalf("recording still reports old selected-target-only limitation: %q", recording.Description)
	}
}

func TestRecordingSynchronizesTargetCreatedEventBeforePolling(t *testing.T) {
	oldInterval := recordingTargetMonitorInterval
	recordingTargetMonitorInterval = 10 * time.Second
	defer func() {
		recordingTargetMonitorInterval = oldInterval
	}()

	server, state := newTargetSyncCDPServer(t)
	defer server.Close()

	rec := NewRecorder()
	if err := rec.StartRecording(serverPort(t, server.URL)); err != nil {
		t.Fatalf("StartRecording: %v", err)
	}
	defer func() {
		if rec.IsRecording() {
			_, _ = rec.StopRecording("cleanup")
		}
	}()

	if !state.waitForBrowserCommand("Target.setDiscoverTargets", 3*time.Second) {
		t.Fatalf("browser target discovery was not enabled")
	}

	state.setExposeNew(true)
	if !state.waitForCommand("new", "Runtime.evaluate", func(params map[string]interface{}) bool {
		return params["expression"] == RecordingInjectJS
	}, 1*time.Second) {
		t.Fatalf("new target was not synchronized from Target.targetCreated event; commands=%v", state.commandsFor("new"))
	}

	recording, err := rec.StopRecording("event target")
	if err != nil {
		t.Fatalf("StopRecording: %v", err)
	}
	if strings.Contains(recording.Description, "events before synchronization") {
		t.Fatalf("event-synchronized target must not use polling-gap warning: %q", recording.Description)
	}
}

func TestRecordingSynchronizesIframeTargetSnapshots(t *testing.T) {
	server, state := newTargetSyncCDPServer(t)
	defer server.Close()

	state.setExposeFrame(true)

	rec := NewRecorder()
	if err := rec.StartRecording(serverPort(t, server.URL)); err != nil {
		t.Fatalf("StartRecording: %v", err)
	}
	defer func() {
		if rec.IsRecording() {
			_, _ = rec.StopRecording("cleanup")
		}
	}()

	if !state.hasCommand("frame", "Page.addScriptToEvaluateOnNewDocument", nil) {
		t.Fatalf("iframe target did not register new-document injection; commands=%v", state.commandsFor("frame"))
	}
	if !state.hasCommand("frame", "Runtime.evaluate", func(params map[string]interface{}) bool {
		return params["expression"] == RecordingInjectJS
	}) {
		t.Fatalf("iframe target was not injected; commands=%v", state.commandsFor("frame"))
	}

	recording, err := rec.StopRecording("iframe target")
	if err != nil {
		t.Fatalf("StopRecording: %v", err)
	}

	if len(recording.Events) != 2 {
		t.Fatalf("merged events = %#v, want 2 events from page + iframe targets", recording.Events)
	}
	foundFrameEvent := false
	for _, event := range recording.Events {
		if event.Type == "scroll" {
			foundFrameEvent = true
			break
		}
	}
	if !foundFrameEvent {
		t.Fatalf("merged events = %#v, want iframe scroll event", recording.Events)
	}
	if strings.Contains(recording.Description, "not fully synchronized") ||
		strings.Contains(recording.Description, "captures only selected target") ||
		strings.Contains(recording.Description, "multiple CDP page targets") {
		t.Fatalf("recording reports stale target limitation: %q", recording.Description)
	}
}

type targetSyncCommand struct {
	Method string
	Params map[string]interface{}
}

type targetSyncState struct {
	mu          sync.Mutex
	exposeNew   bool
	exposeFrame bool
	commands    map[string][]targetSyncCommand

	browserConns    []*targetSyncBrowserConn
	browserCommands []string
}

type targetSyncBrowserConn struct {
	ws      *websocket.Conn
	writeMu sync.Mutex
}

func (c *targetSyncBrowserConn) writeJSON(value interface{}) error {
	c.writeMu.Lock()
	defer c.writeMu.Unlock()
	return c.ws.WriteJSON(value)
}

func newTargetSyncCDPServer(t *testing.T) (*httptest.Server, *targetSyncState) {
	t.Helper()

	state := &targetSyncState{commands: map[string][]targetSyncCommand{}}
	upgrader := websocket.Upgrader{}

	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		switch r.URL.Path {
		case "/json":
			_ = json.NewEncoder(w).Encode(state.targets(r.Host))
		case "/json/version":
			_ = json.NewEncoder(w).Encode(map[string]string{
				"webSocketDebuggerUrl": "ws://" + r.Host + "/browser",
			})
		case "/browser":
			ws, err := upgrader.Upgrade(w, r, nil)
			if err != nil {
				t.Errorf("upgrade browser websocket: %v", err)
				return
			}
			defer ws.Close()
			handleTargetSyncBrowserWS(t, ws, state)
		case "/main", "/new", "/frame", "/internal":
			targetName := strings.TrimPrefix(r.URL.Path, "/")
			ws, err := upgrader.Upgrade(w, r, nil)
			if err != nil {
				t.Errorf("upgrade websocket: %v", err)
				return
			}
			defer ws.Close()
			handleTargetSyncWS(t, ws, state, targetName)
		default:
			http.NotFound(w, r)
		}
	}))

	return server, state
}

func (s *targetSyncState) setExposeNew(value bool) {
	s.mu.Lock()
	changed := value && !s.exposeNew
	s.exposeNew = value
	s.mu.Unlock()
	if changed {
		s.broadcastTargetEvent("new")
	}
}

func (s *targetSyncState) setExposeFrame(value bool) {
	s.mu.Lock()
	changed := value && !s.exposeFrame
	s.exposeFrame = value
	s.mu.Unlock()
	if changed {
		s.broadcastTargetEvent("frame")
	}
}

func (s *targetSyncState) targets(host string) []cdpTarget {
	s.mu.Lock()
	defer s.mu.Unlock()

	targets := []cdpTarget{
		{
			ID:                   "main",
			Type:                 "page",
			URL:                  "https://example.test/main",
			Title:                "Main",
			WebSocketDebuggerURL: "ws://" + host + "/main",
			Active:               true,
		},
		{
			ID:                   "internal",
			Type:                 "page",
			URL:                  "chrome://new-tab-page/",
			Title:                "New Tab",
			WebSocketDebuggerURL: "ws://" + host + "/internal",
		},
	}
	if s.exposeNew {
		targets = append(targets, cdpTarget{
			ID:                   "new",
			Type:                 "page",
			URL:                  "https://example.test/new",
			Title:                "New",
			WebSocketDebuggerURL: "ws://" + host + "/new",
		})
	}
	if s.exposeFrame {
		targets = append(targets, cdpTarget{
			ID:                   "frame",
			Type:                 "iframe",
			URL:                  "https://example.test/frame",
			Title:                "Frame",
			WebSocketDebuggerURL: "ws://" + host + "/frame",
		})
	}
	return targets
}

func handleTargetSyncWS(t *testing.T, ws *websocket.Conn, state *targetSyncState, targetName string) {
	t.Helper()

	for {
		var req struct {
			ID     int                    `json:"id"`
			Method string                 `json:"method"`
			Params map[string]interface{} `json:"params"`
		}
		if err := ws.ReadJSON(&req); err != nil {
			return
		}
		state.addCommand(targetName, targetSyncCommand{Method: req.Method, Params: req.Params})

		result := map[string]interface{}{}
		switch req.Method {
		case "Page.addScriptToEvaluateOnNewDocument":
			result["identifier"] = targetName + "-script"
		case "Runtime.evaluate":
			expression, _ := req.Params["expression"].(string)
			if strings.Contains(expression, "return JSON.stringify(window.__antRecorderSnapshot())") {
				raw, _ := json.Marshal(targetSyncSnapshot(targetName))
				result["result"] = map[string]interface{}{
					"type":  "string",
					"value": string(raw),
				}
			} else {
				result["result"] = map[string]interface{}{"type": "undefined"}
			}
		case "Page.getFrameTree":
			result["frameTree"] = map[string]interface{}{
				"frame":       map[string]interface{}{"id": targetName + "-frame"},
				"childFrames": []map[string]interface{}{},
			}
		}

		if err := ws.WriteJSON(map[string]interface{}{"id": req.ID, "result": result}); err != nil {
			return
		}
	}
}

func handleTargetSyncBrowserWS(t *testing.T, ws *websocket.Conn, state *targetSyncState) {
	t.Helper()

	conn := &targetSyncBrowserConn{ws: ws}
	defer state.unregisterBrowserConn(conn)

	for {
		var req struct {
			ID     int                    `json:"id"`
			Method string                 `json:"method"`
			Params map[string]interface{} `json:"params"`
		}
		if err := ws.ReadJSON(&req); err != nil {
			return
		}
		state.addBrowserCommand(req.Method)
		if req.Method == "Target.setDiscoverTargets" {
			state.registerBrowserConn(conn)
		}
		if err := conn.writeJSON(map[string]interface{}{"id": req.ID, "result": map[string]interface{}{}}); err != nil {
			return
		}
	}
}

func targetSyncSnapshot(targetName string) recordingSnapshot {
	if targetName == "frame" {
		return recordingSnapshot{
			Events: []RecordedEvent{{T: 6, Type: "scroll", DeltaY: 120}},
			Meta: recordingMetaState{
				StartURL:   "https://example.test/frame",
				CurrentURL: "https://example.test/frame",
				Title:      "Frame",
				ViewportW:  900,
				ViewportH:  700,
			},
		}
	}
	if targetName == "new" {
		return recordingSnapshot{
			Events: []RecordedEvent{{T: 5, Type: "input", Text: "new target"}},
			Meta: recordingMetaState{
				StartURL:   "https://example.test/new",
				CurrentURL: "https://example.test/new",
				Title:      "New",
				ViewportW:  900,
				ViewportH:  700,
			},
		}
	}
	return recordingSnapshot{
		Events: []RecordedEvent{{T: 5, Type: "down", X: 10, Y: 20}},
		Meta: recordingMetaState{
			StartURL:   "https://example.test/main",
			CurrentURL: "https://example.test/main",
			Title:      "Main",
			ViewportW:  900,
			ViewportH:  700,
		},
	}
}

func (s *targetSyncState) addCommand(targetName string, command targetSyncCommand) {
	s.mu.Lock()
	defer s.mu.Unlock()
	s.commands[targetName] = append(s.commands[targetName], command)
}

func (s *targetSyncState) addBrowserCommand(method string) {
	s.mu.Lock()
	defer s.mu.Unlock()
	s.browserCommands = append(s.browserCommands, method)
}

func (s *targetSyncState) registerBrowserConn(conn *targetSyncBrowserConn) {
	s.mu.Lock()
	defer s.mu.Unlock()
	for _, existing := range s.browserConns {
		if existing == conn {
			return
		}
	}
	s.browserConns = append(s.browserConns, conn)
}

func (s *targetSyncState) unregisterBrowserConn(conn *targetSyncBrowserConn) {
	s.mu.Lock()
	defer s.mu.Unlock()
	for i, existing := range s.browserConns {
		if existing == conn {
			s.browserConns = append(s.browserConns[:i], s.browserConns[i+1:]...)
			return
		}
	}
}

func (s *targetSyncState) broadcastTargetEvent(targetName string) {
	s.mu.Lock()
	conns := append([]*targetSyncBrowserConn(nil), s.browserConns...)
	s.mu.Unlock()

	for _, conn := range conns {
		_ = conn.writeJSON(map[string]interface{}{
			"method": "Target.targetCreated",
			"params": map[string]interface{}{
				"targetInfo": map[string]interface{}{
					"targetId": targetName,
					"type":     "page",
					"url":      "https://example.test/" + targetName,
				},
			},
		})
	}
}

func (s *targetSyncState) commandsFor(targetName string) []targetSyncCommand {
	s.mu.Lock()
	defer s.mu.Unlock()
	out := make([]targetSyncCommand, len(s.commands[targetName]))
	copy(out, s.commands[targetName])
	return out
}

func (s *targetSyncState) hasCommand(targetName string, method string, match func(map[string]interface{}) bool) bool {
	s.mu.Lock()
	defer s.mu.Unlock()
	for _, command := range s.commands[targetName] {
		if command.Method != method {
			continue
		}
		if match == nil || match(command.Params) {
			return true
		}
	}
	return false
}

func (s *targetSyncState) waitForCommand(targetName string, method string, match func(map[string]interface{}) bool, timeout time.Duration) bool {
	deadline := time.Now().Add(timeout)
	for time.Now().Before(deadline) {
		if s.hasCommand(targetName, method, match) {
			return true
		}
		time.Sleep(25 * time.Millisecond)
	}
	return s.hasCommand(targetName, method, match)
}

func (s *targetSyncState) waitForBrowserCommand(method string, timeout time.Duration) bool {
	deadline := time.Now().Add(timeout)
	for time.Now().Before(deadline) {
		s.mu.Lock()
		for _, command := range s.browserCommands {
			if command == method {
				s.mu.Unlock()
				return true
			}
		}
		s.mu.Unlock()
		time.Sleep(25 * time.Millisecond)
	}
	s.mu.Lock()
	defer s.mu.Unlock()
	for _, command := range s.browserCommands {
		if command == method {
			return true
		}
	}
	return false
}
