package behavior

import (
	"encoding/json"
	"fmt"
	"math"
	"math/rand"
	"sort"
	"strings"
	"sync"
	"time"

	"github.com/gorilla/websocket"
)

// Recorder injects JS into a browser page to capture user interactions
// and retrieves the recorded events via CDP.
type Recorder struct {
	mu        sync.Mutex
	events    []RecordedEvent
	recording bool
	startTime time.Time
	wsConn    *websocket.Conn

	commandID      int
	injectScriptID string
	debugPort      int
	target         cdpTarget
	startTargets   []cdpTarget
	warnings       []string

	targetConns       map[string]*recordingTargetConn
	targetMonitorStop chan struct{}
	targetMonitorDone chan struct{}
}

type recordingTargetConn struct {
	key            string
	target         cdpTarget
	wsConn         *websocket.Conn
	commandID      int
	injectScriptID string
	startOffsetMs  int64
	primary        bool
}

// NewRecorder creates a new recorder.
func NewRecorder() *Recorder {
	return &Recorder{}
}

// StartRecording connects to the browser via CDP and injects the recording script.
func (r *Recorder) StartRecording(debugPort int) error {
	r.mu.Lock()
	defer r.mu.Unlock()

	if r.recording {
		return fmt.Errorf("already recording")
	}

	conn, target, targets, err := connectPageCDPWithTarget(debugPort)
	if err != nil {
		return fmt.Errorf("connect to CDP page endpoint: %w", err)
	}

	// Reset events
	r.events = make([]RecordedEvent, 0)
	r.startTime = time.Now()
	r.commandID = 0
	r.injectScriptID = ""
	r.debugPort = debugPort
	r.target = target
	r.startTargets = append([]cdpTarget(nil), targets...)
	r.warnings = recordingTargetWarnings(target, targets, nil)
	r.targetConns = make(map[string]*recordingTargetConn)

	primary := &recordingTargetConn{
		key:     cdpTargetKey(target),
		target:  target,
		wsConn:  conn,
		primary: true,
	}
	if primary.key == "" {
		primary.key = target.WebSocketDebuggerURL
	}
	if err := r.prepareRecordingTargetLocked(primary); err != nil {
		conn.Close()
		r.wsConn = nil
		return err
	}
	r.wsConn = conn
	r.commandID = primary.commandID
	r.injectScriptID = primary.injectScriptID
	r.targetConns[primary.key] = primary
	r.syncRecordingTargetsLocked("start")

	r.recording = true
	r.startTargetMonitorLocked()
	return nil
}

// StopRecording retrieves recorded events from the browser and returns a Recording.
func (r *Recorder) StopRecording(name string) (*Recording, error) {
	r.mu.Lock()
	if !r.recording {
		r.mu.Unlock()
		return nil, fmt.Errorf("not recording")
	}
	r.recording = false
	stopCh := r.targetMonitorStop
	doneCh := r.targetMonitorDone
	r.targetMonitorStop = nil
	r.targetMonitorDone = nil
	r.mu.Unlock()

	stopRecordingTargetMonitor(stopCh, doneCh)

	r.mu.Lock()
	defer r.mu.Unlock()

	defer func() {
		r.closeRecordingTargetsLocked()
	}()
	r.captureStopTargetWarningsLocked()
	r.syncRecordingTargetsLocked("stop")

	snapshot, err := r.retrieveMergedSnapshotLocked()
	if err != nil {
		return nil, err
	}
	r.events = snapshot.Events
	networkEvents, _ := r.retrieveNetworkEventsLocked()
	performanceMetrics, _ := r.retrievePerformanceMetricsLocked()
	domSnapshot, _ := r.retrieveDOMSnapshotLocked()

	r.cleanupRecordingTargetsLocked()

	rec := newRecordingFromSnapshot(name, recordingWarningDescription(r.warnings), snapshot)
	rec.NetworkEvents = networkEvents
	rec.Performance = performanceMetrics
	rec.DOMSnapshot = domSnapshot
	return rec, nil
}

func (r *Recorder) retrieveNetworkEventsLocked() ([]NetworkRecordedEvent, error) {
	target := r.primaryRecordingTargetLocked()
	if target == nil {
		return nil, fmt.Errorf("no recording target")
	}
	raw, err := r.sendTargetCommandLocked(target, "Runtime.evaluate", map[string]interface{}{
		"expression":    networkCaptureRetrieveJS,
		"returnByValue": true,
	}, 10*time.Second)
	if err != nil {
		return nil, err
	}
	value, ok := runtimeEvaluateValue(raw)
	if !ok {
		return nil, fmt.Errorf("network capture empty")
	}
	var events []NetworkRecordedEvent
	if err := json.Unmarshal([]byte(value), &events); err != nil {
		return nil, err
	}
	return events, nil
}

func (r *Recorder) retrievePerformanceMetricsLocked() (*PerformanceCaptureMetrics, error) {
	target := r.primaryRecordingTargetLocked()
	if target == nil {
		return nil, fmt.Errorf("no recording target")
	}
	raw, err := r.sendTargetCommandLocked(target, "Runtime.evaluate", map[string]interface{}{
		"expression":    performanceCaptureRetrieveJS,
		"returnByValue": true,
	}, 10*time.Second)
	if err != nil {
		return nil, err
	}
	value, ok := runtimeEvaluateValue(raw)
	if !ok {
		return nil, fmt.Errorf("performance metrics empty")
	}
	var metrics PerformanceCaptureMetrics
	if err := json.Unmarshal([]byte(value), &metrics); err != nil {
		return nil, err
	}
	return &metrics, nil
}

func (r *Recorder) retrieveDOMSnapshotLocked() (string, error) {
	target := r.primaryRecordingTargetLocked()
	if target == nil {
		return "", fmt.Errorf("no recording target")
	}
	js := `(function(){
		function walk(node, depth) {
			if (!node || depth > 3) return '';
			var name = node.nodeName || '';
			if (name === '#text') return '';
			var line = '  '.repeat(depth) + name.toLowerCase() + '\n';
			var children = node.children || [];
			for (var i = 0; i < children.length && i < 30; i++) line += walk(children[i], depth + 1);
			return line;
		}
		return walk(document.documentElement, 0);
	})()`
	raw, err := r.sendTargetCommandLocked(target, "Runtime.evaluate", map[string]interface{}{
		"expression":    js,
		"returnByValue": true,
	}, 10*time.Second)
	if err != nil {
		return "", err
	}
	value, ok := runtimeEvaluateValue(raw)
	if !ok {
		return "", fmt.Errorf("dom snapshot empty")
	}
	return value, nil
}

// IsRecording returns whether the recorder is currently active.
func (r *Recorder) IsRecording() bool {
	r.mu.Lock()
	defer r.mu.Unlock()
	return r.recording
}

// Ping checks whether the underlying CDP connection is still alive.
// Returns nil if the connection is healthy, or an error if it is dead.
func (r *Recorder) Ping() error {
	r.mu.Lock()
	defer r.mu.Unlock()

	if r.wsConn == nil {
		return fmt.Errorf("no CDP connection")
	}

	r.wsConn.SetWriteDeadline(time.Now().Add(2 * time.Second))
	if err := r.wsConn.WriteMessage(websocket.PingMessage, nil); err != nil {
		r.wsConn.Close()
		r.wsConn = nil
		r.recording = false
		return fmt.Errorf("CDP ping failed: %w", err)
	}
	return nil
}

// readCDPResponse reads messages from the WebSocket until it finds a response
// matching the expected CDP command ID. Skips event notifications (messages
// without an id field) which Chrome may send between command and response.
func readCDPResponse(conn *websocket.Conn, expectedID int, timeout time.Duration) ([]byte, error) {
	deadline := time.Now().Add(timeout)
	for time.Now().Before(deadline) {
		conn.SetReadDeadline(deadline)
		_, msg, err := conn.ReadMessage()
		if err != nil {
			conn.SetReadDeadline(time.Time{})
			return nil, fmt.Errorf("read CDP response (id=%d): %w", expectedID, err)
		}
		var probe struct {
			ID int `json:"id"`
		}
		if json.Unmarshal(msg, &probe) == nil && probe.ID == expectedID {
			conn.SetReadDeadline(time.Time{})
			return msg, nil
		}
	}
	conn.SetReadDeadline(time.Time{})
	return nil, fmt.Errorf("timeout waiting for CDP response id=%d", expectedID)
}

func sendCDPCommand(conn *websocket.Conn, id int, method string, params map[string]interface{}, timeout time.Duration) ([]byte, error) {
	if conn == nil {
		return nil, fmt.Errorf("no CDP connection")
	}
	msg := map[string]interface{}{
		"id":     id,
		"method": method,
	}
	if params != nil {
		msg["params"] = params
	}
	if err := conn.WriteJSON(msg); err != nil {
		return nil, fmt.Errorf("%s: %w", method, err)
	}
	return readCDPResponse(conn, id, timeout)
}

func (r *Recorder) nextCommandIDLocked() int {
	r.commandID++
	return r.commandID
}

func (r *Recorder) sendCommandLocked(method string, params map[string]interface{}, timeout time.Duration) ([]byte, error) {
	return sendCDPCommand(r.wsConn, r.nextCommandIDLocked(), method, params, timeout)
}

func (r *Recorder) sendTargetCommandLocked(target *recordingTargetConn, method string, params map[string]interface{}, timeout time.Duration) ([]byte, error) {
	if target == nil {
		return nil, fmt.Errorf("no recording target")
	}
	target.commandID++
	return sendCDPCommand(target.wsConn, target.commandID, method, params, timeout)
}

const recorderIsolatedWorldName = "__antRecorderWorld"

var recordingTargetMonitorInterval = 100 * time.Millisecond

func (r *Recorder) injectExistingChildFramesLocked() {
	target := r.primaryRecordingTargetLocked()
	if target == nil {
		r.addWarningLocked("iframe frame tree unavailable: no recording target")
		return
	}
	r.injectExistingChildFramesForTargetLocked(target)
}

func (r *Recorder) injectExistingChildFramesForTargetLocked(target *recordingTargetConn) {
	raw, err := r.sendTargetCommandLocked(target, "Page.getFrameTree", nil, 10*time.Second)
	if err != nil {
		r.addWarningLocked("iframe frame tree unavailable for " + describeCDPTarget(target.target) + ": " + err.Error())
		return
	}

	frameIDs, err := childFrameIDsFromFrameTreeResponse(raw)
	if err != nil {
		r.addWarningLocked("iframe frame tree parse failed for " + describeCDPTarget(target.target) + ": " + err.Error())
		return
	}
	for _, frameID := range frameIDs {
		contextID, err := r.createIsolatedWorldForTargetLocked(target, frameID)
		if err != nil {
			r.addWarningLocked("iframe recorder injection skipped for frame " + frameID + " in " + describeCDPTarget(target.target) + ": " + err.Error())
			continue
		}
		if _, err := r.sendTargetCommandLocked(target, "Runtime.evaluate", map[string]interface{}{
			"expression":    RecordingInjectJS,
			"awaitPromise":  false,
			"returnByValue": false,
			"contextId":     contextID,
		}, 10*time.Second); err != nil {
			r.addWarningLocked("iframe recorder injection failed for frame " + frameID + " in " + describeCDPTarget(target.target) + ": " + err.Error())
		}
	}
}

func (r *Recorder) createIsolatedWorldLocked(frameID string) (int, error) {
	target := r.primaryRecordingTargetLocked()
	if target == nil {
		return 0, fmt.Errorf("no recording target")
	}
	return r.createIsolatedWorldForTargetLocked(target, frameID)
}

func (r *Recorder) createIsolatedWorldForTargetLocked(target *recordingTargetConn, frameID string) (int, error) {
	raw, err := r.sendTargetCommandLocked(target, "Page.createIsolatedWorld", map[string]interface{}{
		"frameId":   frameID,
		"worldName": recorderIsolatedWorldName,
	}, 5*time.Second)
	if err != nil {
		return 0, err
	}
	var response struct {
		Result struct {
			ExecutionContextID int `json:"executionContextId"`
		} `json:"result"`
	}
	if err := json.Unmarshal(raw, &response); err != nil {
		return 0, fmt.Errorf("parse isolated world response: %w", err)
	}
	if response.Result.ExecutionContextID == 0 {
		return 0, fmt.Errorf("missing executionContextId")
	}
	return response.Result.ExecutionContextID, nil
}

type cdpFrameNode struct {
	Frame struct {
		ID string `json:"id"`
	} `json:"frame"`
	ChildFrames []cdpFrameNode `json:"childFrames,omitempty"`
}

func childFrameIDsFromFrameTreeResponse(raw []byte) ([]string, error) {
	var response struct {
		Result struct {
			FrameTree cdpFrameNode `json:"frameTree"`
		} `json:"result"`
	}
	if err := json.Unmarshal(raw, &response); err != nil {
		return nil, fmt.Errorf("parse Page.getFrameTree response: %w", err)
	}
	var ids []string
	collectChildFrameIDs(response.Result.FrameTree, &ids)
	return ids, nil
}

func collectChildFrameIDs(node cdpFrameNode, ids *[]string) {
	for _, child := range node.ChildFrames {
		if strings.TrimSpace(child.Frame.ID) != "" {
			*ids = append(*ids, child.Frame.ID)
		}
		collectChildFrameIDs(child, ids)
	}
}

func (r *Recorder) prepareRecordingTargetLocked(target *recordingTargetConn) error {
	if _, err := r.sendTargetCommandLocked(target, "Page.enable", nil, 10*time.Second); err != nil {
		return fmt.Errorf("enable page domain for %s: %w", describeCDPTarget(target.target), err)
	}

	addRaw, err := r.sendTargetCommandLocked(target, "Page.addScriptToEvaluateOnNewDocument", map[string]interface{}{
		"source": RecordingInjectJS,
	}, 10*time.Second)
	if err != nil {
		return fmt.Errorf("register recording script for new documents on %s: %w", describeCDPTarget(target.target), err)
	}

	var addResp struct {
		Result struct {
			Identifier string `json:"identifier"`
		} `json:"result"`
	}
	if json.Unmarshal(addRaw, &addResp) == nil {
		target.injectScriptID = addResp.Result.Identifier
	}

	if _, err := r.sendTargetCommandLocked(target, "Runtime.evaluate", map[string]interface{}{
		"expression":    RecordingInjectJS,
		"awaitPromise":  false,
		"returnByValue": false,
	}, 10*time.Second); err != nil {
		return fmt.Errorf("inject recording script on %s: %w", describeCDPTarget(target.target), err)
	}
	if _, err := r.sendTargetCommandLocked(target, "Page.addScriptToEvaluateOnNewDocument", map[string]interface{}{
		"source": NetworkCaptureInjectJS,
	}, 10*time.Second); err == nil {
		_, _ = r.sendTargetCommandLocked(target, "Runtime.evaluate", map[string]interface{}{
			"expression":    NetworkCaptureInjectJS,
			"returnByValue": false,
		}, 10*time.Second)
	}
	r.injectExistingChildFramesForTargetLocked(target)
	return nil
}

func (r *Recorder) syncRecordingTargetsLocked(phase string) {
	if r.debugPort <= 0 {
		return
	}
	targets, err := listCDPTargets(r.debugPort)
	if err != nil {
		r.addWarningLocked("target sync metadata unavailable at " + phase + ": " + err.Error())
		return
	}
	for _, target := range recordablePageTargets(targets) {
		key := cdpTargetKey(target)
		if key == "" {
			continue
		}
		if existing, ok := r.targetConns[key]; ok {
			existing.target = target
			continue
		}
		if err := r.connectRecordingTargetLocked(target); err != nil {
			r.addWarningLocked("target sync failed for " + describeCDPTarget(target) + " at " + phase + ": " + err.Error())
			continue
		}
		if phase != "start" && phase != "target-event" {
			r.addWarningLocked("target " + describeCDPTarget(target) + " was first synchronized at " + phase + "; events before synchronization may be missing.")
		}
	}
}

func (r *Recorder) connectRecordingTargetLocked(target cdpTarget) error {
	ws, _, err := behaviorWSDialer.Dial(target.WebSocketDebuggerURL, nil)
	if err != nil {
		return fmt.Errorf("ws dial target: %w", err)
	}

	rt := &recordingTargetConn{
		key:           cdpTargetKey(target),
		target:        target,
		wsConn:        ws,
		startOffsetMs: int64(time.Since(r.startTime) / time.Millisecond),
	}
	if rt.key == "" {
		rt.key = target.WebSocketDebuggerURL
	}
	if err := r.prepareRecordingTargetLocked(rt); err != nil {
		ws.Close()
		return err
	}
	r.targetConns[rt.key] = rt
	return nil
}

func (r *Recorder) startTargetMonitorLocked() {
	stopCh := make(chan struct{})
	doneCh := make(chan struct{})
	debugPort := r.debugPort
	r.targetMonitorStop = stopCh
	r.targetMonitorDone = doneCh
	go r.monitorRecordingTargets(debugPort, stopCh, doneCh)
}

func (r *Recorder) monitorRecordingTargets(debugPort int, stopCh <-chan struct{}, doneCh chan<- struct{}) {
	defer close(doneCh)
	ticker := time.NewTicker(recordingTargetMonitorInterval)
	defer ticker.Stop()

	discoveryConn, discoveryCh, discoveryDone := startRecordingTargetDiscovery(debugPort)
	if discoveryConn != nil {
		defer func() {
			discoveryConn.Close()
			<-discoveryDone
		}()
	}

	for {
		select {
		case <-stopCh:
			return
		case _, ok := <-discoveryCh:
			if !ok {
				discoveryCh = nil
				continue
			}
			r.mu.Lock()
			if !r.recording {
				r.mu.Unlock()
				return
			}
			r.syncRecordingTargetsLocked("target-event")
			r.mu.Unlock()
		case <-ticker.C:
			r.mu.Lock()
			if !r.recording {
				r.mu.Unlock()
				return
			}
			r.syncRecordingTargetsLocked("monitor")
			r.mu.Unlock()
		}
	}
}

func startRecordingTargetDiscovery(debugPort int) (*cdpConn, <-chan struct{}, <-chan struct{}) {
	if debugPort <= 0 {
		return nil, nil, nil
	}
	conn, err := connectCDP(debugPort)
	if err != nil {
		return nil, nil, nil
	}
	if _, err := conn.sendCommand("Target.setDiscoverTargets", map[string]interface{}{"discover": true}); err != nil {
		conn.Close()
		return nil, nil, nil
	}

	events := make(chan struct{}, 16)
	done := make(chan struct{})
	go func() {
		defer close(done)
		defer close(events)
		readRecordingTargetDiscoveryEvents(conn.ws, events)
	}()
	return conn, events, done
}

func readRecordingTargetDiscoveryEvents(conn *websocket.Conn, events chan<- struct{}) {
	for {
		_, msg, err := conn.ReadMessage()
		if err != nil {
			return
		}
		var event struct {
			Method string `json:"method"`
		}
		if err := json.Unmarshal(msg, &event); err != nil {
			continue
		}
		switch event.Method {
		case "Target.targetCreated", "Target.targetInfoChanged":
			select {
			case events <- struct{}{}:
			default:
			}
		}
	}
}

func stopRecordingTargetMonitor(stopCh chan struct{}, doneCh chan struct{}) {
	if stopCh != nil {
		close(stopCh)
	}
	if doneCh != nil {
		<-doneCh
	}
}

func (r *Recorder) captureStopTargetWarningsLocked() {
	if r.debugPort <= 0 {
		return
	}
	targets, err := listCDPTargets(r.debugPort)
	if err != nil {
		r.addWarningLocked("target metadata unavailable at stop: " + err.Error())
		return
	}
	for _, warning := range recordingTargetWarnings(r.target, r.startTargets, targets) {
		r.addWarningLocked(warning)
	}
}

func (r *Recorder) addWarningLocked(warning string) {
	warning = strings.TrimSpace(warning)
	if warning == "" {
		return
	}
	for _, existing := range r.warnings {
		if existing == warning {
			return
		}
	}
	r.warnings = append(r.warnings, warning)
}

type recordingSnapshot struct {
	Events []RecordedEvent    `json:"events"`
	Meta   recordingMetaState `json:"meta"`
}

type recordingMetaState struct {
	StartURL         string  `json:"startUrl,omitempty"`
	CurrentURL       string  `json:"currentUrl,omitempty"`
	Title            string  `json:"title,omitempty"`
	DevicePixelRatio float64 `json:"devicePixelRatio,omitempty"`
	DPR              float64 `json:"dpr,omitempty"`
	Scale            float64 `json:"scale,omitempty"`
	ViewportW        int     `json:"viewportW,omitempty"`
	ViewportH        int     `json:"viewportH,omitempty"`
}

const recordingSnapshotExpression = `(function() {
  if (window.__antRecorderSnapshot) return JSON.stringify(window.__antRecorderSnapshot());
  return JSON.stringify({
    events: window.__antRecordedEvents || [],
    meta: {
      startUrl: window.location && window.location.href || '',
      currentUrl: window.location && window.location.href || '',
      title: document.title || '',
      devicePixelRatio: window.devicePixelRatio || 1,
      scale: window.visualViewport && window.visualViewport.scale || 1,
      viewportW: window.innerWidth || 0,
      viewportH: window.innerHeight || 0
    }
  });
})()`

func (r *Recorder) retrieveSnapshotLocked() (recordingSnapshot, error) {
	target := r.primaryRecordingTargetLocked()
	if target == nil {
		return recordingSnapshot{}, fmt.Errorf("no recording target")
	}
	return r.retrieveSnapshotFromTargetLocked(target)
}

func (r *Recorder) retrieveSnapshotFromTargetLocked(target *recordingTargetConn) (recordingSnapshot, error) {
	raw, err := r.sendTargetCommandLocked(target, "Runtime.evaluate", map[string]interface{}{
		"expression":    recordingSnapshotExpression,
		"returnByValue": true,
	}, 10*time.Second)
	if err != nil {
		return recordingSnapshot{}, fmt.Errorf("retrieve events from %s: %w", describeCDPTarget(target.target), err)
	}

	value, err := runtimeStringValue(raw)
	if err != nil {
		return recordingSnapshot{}, err
	}
	if strings.TrimSpace(value) == "" {
		return recordingSnapshot{}, fmt.Errorf("no events returned from browser")
	}

	snapshot, err := parseRecordingSnapshotJSON(value)
	if err != nil {
		return recordingSnapshot{}, err
	}
	return snapshot, nil
}

func (r *Recorder) retrieveMergedSnapshotLocked() (recordingSnapshot, error) {
	targets := r.recordingTargetsLocked()
	if len(targets) == 0 {
		return recordingSnapshot{}, fmt.Errorf("no recording target")
	}

	var merged recordingSnapshot
	var firstErr error
	successes := 0
	primaryMetaSet := false

	for _, target := range targets {
		// Re-inject on stop in case navigation replaced the runtime after the
		// new-document hook was installed.
		if _, err := r.sendTargetCommandLocked(target, "Runtime.evaluate", map[string]interface{}{
			"expression":    RecordingInjectJS,
			"awaitPromise":  false,
			"returnByValue": false,
		}, 10*time.Second); err != nil {
			if firstErr == nil {
				firstErr = err
			}
			r.addWarningLocked("ensure recording script before stop failed for " + describeCDPTarget(target.target) + ": " + err.Error())
			continue
		}

		snapshot, err := r.retrieveSnapshotFromTargetLocked(target)
		if err != nil {
			if firstErr == nil {
				firstErr = err
			}
			r.addWarningLocked(err.Error())
			continue
		}
		offsetRecordingEvents(snapshot.Events, target.startOffsetMs)
		merged.Events = append(merged.Events, snapshot.Events...)
		if target.primary || !primaryMetaSet {
			merged.Meta = snapshot.Meta
			primaryMetaSet = true
		}
		successes++
	}

	if successes == 0 {
		if firstErr != nil {
			return recordingSnapshot{}, firstErr
		}
		return recordingSnapshot{}, fmt.Errorf("no recording snapshots returned")
	}

	sort.SliceStable(merged.Events, func(i, j int) bool {
		return merged.Events[i].T < merged.Events[j].T
	})
	merged.Events = normalizeRecordedEvents(merged.Events)
	return merged, nil
}

func (r *Recorder) recordingTargetsLocked() []*recordingTargetConn {
	targets := make([]*recordingTargetConn, 0, len(r.targetConns))
	for _, target := range r.targetConns {
		if target != nil && target.wsConn != nil {
			targets = append(targets, target)
		}
	}
	sort.SliceStable(targets, func(i, j int) bool {
		if targets[i].primary != targets[j].primary {
			return targets[i].primary
		}
		return targets[i].key < targets[j].key
	})
	return targets
}

func (r *Recorder) primaryRecordingTargetLocked() *recordingTargetConn {
	for _, target := range r.targetConns {
		if target != nil && target.primary {
			return target
		}
	}
	if r.wsConn == nil {
		return nil
	}
	return &recordingTargetConn{
		key:            cdpTargetKey(r.target),
		target:         r.target,
		wsConn:         r.wsConn,
		commandID:      r.commandID,
		injectScriptID: r.injectScriptID,
		primary:        true,
	}
}

func (r *Recorder) cleanupRecordingTargetsLocked() {
	teardownJS := `if (window.__antRecorderTeardown) { window.__antRecorderTeardown(); } else { delete window.__antRecordedEvents; delete window.__antRecorder; }`
	for _, target := range r.recordingTargetsLocked() {
		if target.injectScriptID != "" {
			_, _ = r.sendTargetCommandLocked(target, "Page.removeScriptToEvaluateOnNewDocument", map[string]interface{}{
				"identifier": target.injectScriptID,
			}, 5*time.Second)
		}
		_, _ = r.sendTargetCommandLocked(target, "Runtime.evaluate", map[string]interface{}{
			"expression":    teardownJS,
			"returnByValue": false,
		}, 5*time.Second)
	}
}

func (r *Recorder) closeRecordingTargetsLocked() {
	for _, target := range r.targetConns {
		if target != nil && target.wsConn != nil {
			_ = target.wsConn.Close()
			target.wsConn = nil
		}
	}
	r.targetConns = nil
	r.wsConn = nil
	r.injectScriptID = ""
}

func offsetRecordingEvents(events []RecordedEvent, offsetMs int64) {
	if offsetMs <= 0 {
		return
	}
	for i := range events {
		events[i].T += offsetMs
	}
}

func runtimeStringValue(raw []byte) (string, error) {
	var response struct {
		Result struct {
			Result struct {
				Value string `json:"value"`
			} `json:"result"`
		} `json:"result"`
	}
	if err := json.Unmarshal(raw, &response); err != nil {
		return "", fmt.Errorf("parse Runtime.evaluate response: %w", err)
	}
	return response.Result.Result.Value, nil
}

func parseRecordingSnapshotJSON(value string) (recordingSnapshot, error) {
	trimmed := strings.TrimSpace(value)
	if strings.HasPrefix(trimmed, "[") {
		var events []RecordedEvent
		if err := json.Unmarshal([]byte(trimmed), &events); err != nil {
			return recordingSnapshot{}, fmt.Errorf("parse events JSON: %w", err)
		}
		return recordingSnapshot{Events: normalizeRecordedEvents(events)}, nil
	}

	var snapshot recordingSnapshot
	if err := json.Unmarshal([]byte(trimmed), &snapshot); err != nil {
		return recordingSnapshot{}, fmt.Errorf("parse recording snapshot JSON: %w", err)
	}
	snapshot.Events = normalizeRecordedEvents(snapshot.Events)
	return snapshot, nil
}

func newRecordingFromSnapshot(name, description string, snapshot recordingSnapshot) *Recording {
	events := normalizeRecordedEvents(snapshot.Events)
	duration := int64(0)
	if len(events) > 0 {
		duration = events[len(events)-1].T
	}

	viewportW, viewportH := snapshot.Meta.ViewportW, snapshot.Meta.ViewportH
	if viewportW <= 0 {
		viewportW = 1920
	}
	if viewportH <= 0 {
		viewportH = 1080
	}

	dpr := snapshot.Meta.DevicePixelRatio
	if dpr == 0 {
		dpr = snapshot.Meta.DPR
	}
	if dpr == 0 {
		dpr = 1
	}
	scale := snapshot.Meta.Scale
	if scale == 0 {
		scale = 1
	}

	return &Recording{
		ID:               generateID(),
		Name:             name,
		Description:      description,
		Events:           events,
		DurationMs:       duration,
		ViewportW:        viewportW,
		ViewportH:        viewportH,
		StartURL:         snapshot.Meta.StartURL,
		CurrentURL:       snapshot.Meta.CurrentURL,
		Title:            snapshot.Meta.Title,
		DevicePixelRatio: dpr,
		Scale:            scale,
		CreatedAt:        time.Now().Format(time.RFC3339),
	}
}

func recordingWarningDescription(warnings []string) string {
	warnings = dedupeWarnings(warnings)
	if len(warnings) == 0 {
		return ""
	}
	return "[recording metadata warning] " + strings.Join(warnings, "; ")
}

func recordingTargetWarnings(_ cdpTarget, startTargets []cdpTarget, stopTargets []cdpTarget) []string {
	warnings := make([]string, 0, 2)
	warnings = append(warnings, unconnectableTargetWarnings("start", startTargets)...)
	warnings = append(warnings, unconnectableTargetWarnings("stop", stopTargets)...)
	return dedupeWarnings(warnings)
}

func recordablePageTargets(targets []cdpTarget) []cdpTarget {
	recordable := make([]cdpTarget, 0, len(targets))
	for _, target := range targets {
		if isRecordableTargetType(target.Type) &&
			strings.TrimSpace(target.WebSocketDebuggerURL) != "" &&
			!isInternalBrowserURL(target.URL) {
			recordable = append(recordable, target)
		}
	}
	return recordable
}

func unconnectableTargetWarnings(phase string, targets []cdpTarget) []string {
	if len(targets) == 0 {
		return nil
	}
	warnings := make([]string, 0, 1)
	for _, target := range targets {
		if !isRecordableTargetType(target.Type) || isInternalBrowserURL(target.URL) {
			continue
		}
		if strings.TrimSpace(target.WebSocketDebuggerURL) == "" {
			warnings = append(warnings, fmt.Sprintf(
				"target %s could not be synchronized at %s: missing webSocketDebuggerUrl.",
				describeCDPTarget(target),
				phase,
			))
		}
	}
	return warnings
}

func isRecordableTargetType(typ string) bool {
	return strings.EqualFold(typ, "page") || strings.EqualFold(typ, "iframe")
}

func describeCDPTarget(target cdpTarget) string {
	id := strings.TrimSpace(target.ID)
	if id == "" {
		id = "unknown"
	}
	url := strings.TrimSpace(target.URL)
	if url == "" {
		return id
	}
	return id + " (" + url + ")"
}

func cdpTargetKey(target cdpTarget) string {
	if id := strings.TrimSpace(target.ID); id != "" {
		return id
	}
	if wsURL := strings.TrimSpace(target.WebSocketDebuggerURL); wsURL != "" {
		return wsURL
	}
	return strings.TrimSpace(target.URL)
}

func dedupeWarnings(warnings []string) []string {
	if len(warnings) == 0 {
		return nil
	}
	seen := make(map[string]struct{}, len(warnings))
	out := make([]string, 0, len(warnings))
	for _, warning := range warnings {
		warning = strings.TrimSpace(warning)
		if warning == "" {
			continue
		}
		if _, ok := seen[warning]; ok {
			continue
		}
		seen[warning] = struct{}{}
		out = append(out, warning)
	}
	return out
}

func normalizeRecordedEvents(events []RecordedEvent) []RecordedEvent {
	if len(events) == 0 {
		return events
	}
	normalized := make([]RecordedEvent, 0, len(events))
	for i, evt := range events {
		if evt.Sensitive {
			evt.Text = ""
		}
		if evt.Type == "click" && hasAtomicClickPair(events, i) {
			continue
		}
		normalized = append(normalized, evt)
	}
	return normalized
}

func hasAtomicClickPair(events []RecordedEvent, clickIndex int) bool {
	click := events[clickIndex]
	seenUp := false
	for i := clickIndex - 1; i >= 0; i-- {
		prev := events[i]
		if click.T-prev.T > 1000 {
			break
		}
		if !sameClickPoint(click, prev) {
			continue
		}
		if prev.Type == "up" {
			seenUp = true
			continue
		}
		if prev.Type == "down" && seenUp {
			return true
		}
	}
	return false
}

func sameClickPoint(a, b RecordedEvent) bool {
	if a.Button != b.Button {
		return false
	}
	return math.Abs(a.X-b.X) <= 2 && math.Abs(a.Y-b.Y) <= 2
}

// RecoverRecording reconnects to a running recording session and retrieves events.
// This is used when the in-memory recorder state was lost (e.g. after process restart).
func RecoverRecording(debugPort int, name string) (*Recording, error) {
	conn, err := ConnectPageCDP(debugPort)
	if err != nil {
		return nil, fmt.Errorf("recover CDP connection: %w", err)
	}
	defer conn.Close()

	commandID := 1
	if _, err := sendCDPCommand(conn, commandID, "Runtime.evaluate", map[string]interface{}{
		"expression":    RecordingInjectJS,
		"awaitPromise":  false,
		"returnByValue": false,
	}, 10*time.Second); err != nil {
		return nil, fmt.Errorf("ensure recording script during recovery: %w", err)
	}

	commandID++
	raw, err := sendCDPCommand(conn, commandID, "Runtime.evaluate", map[string]interface{}{
		"expression":    recordingSnapshotExpression,
		"returnByValue": true,
	}, 10*time.Second)
	if err != nil {
		return nil, fmt.Errorf("retrieve events: %w", err)
	}

	value, err := runtimeStringValue(raw)
	if err != nil {
		return nil, err
	}
	if strings.TrimSpace(value) == "" {
		return nil, fmt.Errorf("no events returned from browser")
	}

	snapshot, err := parseRecordingSnapshotJSON(value)
	if err != nil {
		return nil, err
	}

	// Tear down recording script
	commandID++
	teardownJS := `if (window.__antRecorderTeardown) { window.__antRecorderTeardown(); } else { delete window.__antRecordedEvents; delete window.__antRecorder; }`
	_, _ = sendCDPCommand(conn, commandID, "Runtime.evaluate", map[string]interface{}{
		"expression":    teardownJS,
		"returnByValue": false,
	}, 5*time.Second)

	return newRecordingFromSnapshot(name, "", snapshot), nil
}

// generateID generates a short unique ID for recordings.
func generateID() string {
	return fmt.Sprintf("rec-%d-%d", time.Now().UnixMilli(), rand.Int63n(10000))
}
