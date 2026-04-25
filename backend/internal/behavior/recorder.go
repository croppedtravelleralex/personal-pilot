package behavior

import (
	"encoding/json"
	"fmt"
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

	conn, _, err := websocket.DefaultDialer.Dial(fmt.Sprintf("ws://127.0.0.1:%d/devtools/browser", debugPort), nil)
	if err != nil {
		return fmt.Errorf("connect to CDP browser endpoint: %w", err)
	}
	r.wsConn = conn

	// Reset events
	r.events = make([]RecordedEvent, 0)
	r.startTime = time.Now()

	// Inject recording script via Runtime.evaluate
	injectJS := `
(function() {
  if (window.__antRecorder) return;
  const start = performance.now();
  const events = [];
  window.__antRecordedEvents = events;

  function record(type, e) {
    const t = Math.round(performance.now() - start);
    const evt = { t, type };
    if (e) {
      if (typeof e.clientX === 'number') { evt.x = e.clientX; evt.y = e.clientY; }
      if (typeof e.button === 'number') evt.btn = e.button;
      if (e.key) evt.key = e.key;
      if (e.deltaX !== undefined) { evt.dx = e.deltaX; evt.dy = e.deltaY; }
    }
    events.push(evt);
  }

  function throttle(fn, ms) {
    let last = 0;
    return function(...args) {
      const now = performance.now();
      if (now - last >= ms) { last = now; return fn.apply(this, args); }
    };
  }

  document.addEventListener('mousemove', throttle(function(e) { record('move', e); }, 50), true);
  document.addEventListener('mousedown', function(e) { record('down', e); }, true);
  document.addEventListener('mouseup', function(e) { record('up', e); }, true);
  document.addEventListener('click', function(e) { record('click', e); }, true);
  document.addEventListener('keydown', function(e) {
    const evt = { t: Math.round(performance.now() - start), type: 'key' };
    evt.key = e.key;
    if (e.key.length === 1) evt.text = e.key;
    if (e.ctrlKey) evt.key = 'Control+' + evt.key;
    if (e.altKey) evt.key = 'Alt+' + evt.key;
    if (e.shiftKey) evt.key = 'Shift+' + evt.key;
    events.push(evt);
  }, true);
  document.addEventListener('wheel', function(e) { record('scroll', e); }, true);

  window.__antRecorder = true;
  console.log('[AntRecorder] Recording started');
})();
`
	msg := map[string]interface{}{
		"id":     1,
		"method": "Runtime.evaluate",
		"params": map[string]interface{}{
			"expression":    injectJS,
			"awaitPromise":  false,
			"returnByValue": false,
		},
	}
	if err := conn.WriteJSON(msg); err != nil {
		conn.Close()
		r.wsConn = nil
		return fmt.Errorf("inject recording script: %w", err)
	}

	r.recording = true
	return nil
}

// StopRecording retrieves recorded events from the browser and returns a Recording.
func (r *Recorder) StopRecording(name string) (*Recording, error) {
	r.mu.Lock()
	defer r.mu.Unlock()

	if !r.recording {
		return nil, fmt.Errorf("not recording")
	}
	r.recording = false

	defer func() {
		if r.wsConn != nil {
			r.wsConn.Close()
			r.wsConn = nil
		}
	}()

	// Retrieve events array via Runtime.evaluate
	retrieveJS := `JSON.stringify(window.__antRecordedEvents || [])`
	msg := map[string]interface{}{
		"id":     2,
		"method": "Runtime.evaluate",
		"params": map[string]interface{}{
			"expression":    retrieveJS,
			"returnByValue": true,
		},
	}
	if err := r.wsConn.WriteJSON(msg); err != nil {
		return nil, fmt.Errorf("retrieve events: %w", err)
	}

	// Read the response
	_, raw, err := r.wsConn.ReadMessage()
	if err != nil {
		return nil, fmt.Errorf("read events response: %w", err)
	}

	var response struct {
		Result struct {
			Result struct {
				Value string `json:"value"`
			} `json:"result"`
		} `json:"result"`
	}
	if err := json.Unmarshal(raw, &response); err != nil {
		return nil, fmt.Errorf("parse events response: %w", err)
	}

	eventsJSON := response.Result.Result.Value
	if eventsJSON == "" {
		return nil, fmt.Errorf("no events returned from browser")
	}

	var events []RecordedEvent
	if err := json.Unmarshal([]byte(eventsJSON), &events); err != nil {
		return nil, fmt.Errorf("parse events JSON: %w", err)
	}

	// Get viewport size
	viewportW, viewportH := 1920, 1080
	if r.wsConn != nil {
		vpMsg := map[string]interface{}{
			"id":     3,
			"method": "Runtime.evaluate",
			"params": map[string]interface{}{
				"expression":    `JSON.stringify({w: window.innerWidth, h: window.innerHeight})`,
				"returnByValue": true,
			},
		}
		if err := r.wsConn.WriteJSON(vpMsg); err == nil {
			_, vpRaw, vpErr := r.wsConn.ReadMessage()
			if vpErr == nil {
				var vpResp struct {
					Result struct {
						Result struct {
							Value string `json:"value"`
						} `json:"result"`
					} `json:"result"`
				}
				if json.Unmarshal(vpRaw, &vpResp) == nil && vpResp.Result.Result.Value != "" {
					var vp struct {
						W int `json:"w"`
						H int `json:"h"`
					}
					if json.Unmarshal([]byte(vpResp.Result.Result.Value), &vp) == nil {
						if vp.W > 0 {
							viewportW = vp.W
						}
						if vp.H > 0 {
							viewportH = vp.H
						}
					}
				}
			}
		}
	}

	// Tear down recording script
	teardownJS := `delete window.__antRecordedEvents; delete window.__antRecorder;`
	_ = r.wsConn.WriteJSON(map[string]interface{}{
		"id":     4,
		"method": "Runtime.evaluate",
		"params": map[string]interface{}{
			"expression":    teardownJS,
			"returnByValue": false,
		},
	})

	duration := int64(0)
	if len(events) > 0 {
		duration = events[len(events)-1].T
	}

	recording := &Recording{
		ID:          generateID(),
		Name:        name,
		Description: "",
		Events:      events,
		DurationMs:  duration,
		ViewportW:   viewportW,
		ViewportH:   viewportH,
		CreatedAt:   time.Now().Format(time.RFC3339),
	}

	return recording, nil
}

// IsRecording returns whether the recorder is currently active.
func (r *Recorder) IsRecording() bool {
	r.mu.Lock()
	defer r.mu.Unlock()
	return r.recording
}

// generateID generates a short unique ID for recordings.
func generateID() string {
	return fmt.Sprintf("rec-%d", time.Now().UnixNano())
}
