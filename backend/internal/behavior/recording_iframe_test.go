package behavior

import (
	"encoding/json"
	"net"
	"net/http"
	"net/http/httptest"
	"net/url"
	"strconv"
	"strings"
	"sync"
	"testing"

	"github.com/gorilla/websocket"
)

func TestIframeStartRecordingInjectsExistingChildFrames(t *testing.T) {
	server, records := newIframeCDPServer(t)
	defer server.Close()

	port := serverPort(t, server.URL)
	rec := NewRecorder()
	if err := rec.StartRecording(port); err != nil {
		t.Fatalf("StartRecording: %v", err)
	}
	defer func() {
		rec.mu.Lock()
		if rec.wsConn != nil {
			_ = rec.wsConn.Close()
		}
		rec.recording = false
		rec.mu.Unlock()
	}()

	seenGetFrameTree := false
	seenCreateWorld := false
	seenChildEvaluate := false
	for _, record := range records.snapshot() {
		switch record.Method {
		case "Page.getFrameTree":
			seenGetFrameTree = true
		case "Page.createIsolatedWorld":
			if record.Params["frameId"] == "child-frame" {
				seenCreateWorld = true
			}
		case "Runtime.evaluate":
			if id, ok := numericParam(record.Params, "contextId"); ok && id == 42 {
				seenChildEvaluate = true
			}
		}
	}

	if !seenGetFrameTree {
		t.Fatal("StartRecording must inspect the current frame tree")
	}
	if !seenCreateWorld {
		t.Fatal("StartRecording must create an isolated world for existing child frames")
	}
	if !seenChildEvaluate {
		t.Fatal("StartRecording must inject the recorder into the child frame context")
	}
}

func TestIframeRecordingScriptPublishesFrameEventsToTop(t *testing.T) {
	required := []string{
		"__antRecorderFrameEventV1",
		"window.top.postMessage",
		"findFrameElement",
		"framePath + ' >> '",
	}
	for _, snippet := range required {
		if !strings.Contains(RecordingInjectJS, snippet) {
			t.Fatalf("RecordingInjectJS missing iframe aggregation snippet %q", snippet)
		}
	}
}

type iframeCDPRecord struct {
	Method string
	Params map[string]interface{}
}

type iframeCDPRecords struct {
	mu      sync.Mutex
	records []iframeCDPRecord
}

func (r *iframeCDPRecords) add(record iframeCDPRecord) {
	r.mu.Lock()
	defer r.mu.Unlock()
	r.records = append(r.records, record)
}

func (r *iframeCDPRecords) snapshot() []iframeCDPRecord {
	r.mu.Lock()
	defer r.mu.Unlock()
	out := make([]iframeCDPRecord, len(r.records))
	copy(out, r.records)
	return out
}

func newIframeCDPServer(t *testing.T) (*httptest.Server, *iframeCDPRecords) {
	t.Helper()
	records := &iframeCDPRecords{}
	upgrader := websocket.Upgrader{}

	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		switch r.URL.Path {
		case "/json":
			_ = json.NewEncoder(w).Encode([]cdpTarget{{
				ID:                   "main-target",
				Type:                 "page",
				URL:                  "https://example.test/app",
				WebSocketDebuggerURL: "ws://" + r.Host + "/page",
			}})
		case "/page":
			ws, err := upgrader.Upgrade(w, r, nil)
			if err != nil {
				t.Errorf("upgrade websocket: %v", err)
				return
			}
			defer ws.Close()
			for {
				var req struct {
					ID     int                    `json:"id"`
					Method string                 `json:"method"`
					Params map[string]interface{} `json:"params"`
				}
				if err := ws.ReadJSON(&req); err != nil {
					return
				}
				records.add(iframeCDPRecord{Method: req.Method, Params: req.Params})

				result := map[string]interface{}{}
				switch req.Method {
				case "Page.addScriptToEvaluateOnNewDocument":
					result["identifier"] = "inject-script"
				case "Runtime.evaluate":
					result["result"] = map[string]interface{}{"type": "undefined"}
				case "Page.getFrameTree":
					result["frameTree"] = map[string]interface{}{
						"frame": map[string]interface{}{"id": "main-frame"},
						"childFrames": []map[string]interface{}{{
							"frame": map[string]interface{}{"id": "child-frame"},
						}},
					}
				case "Page.createIsolatedWorld":
					result["executionContextId"] = 42
				}

				if err := ws.WriteJSON(map[string]interface{}{"id": req.ID, "result": result}); err != nil {
					return
				}
			}
		default:
			http.NotFound(w, r)
		}
	}))

	return server, records
}

func serverPort(t *testing.T, rawURL string) int {
	t.Helper()
	parsed, err := url.Parse(rawURL)
	if err != nil {
		t.Fatalf("parse server url: %v", err)
	}
	_, portText, err := net.SplitHostPort(parsed.Host)
	if err != nil {
		t.Fatalf("split server host: %v", err)
	}
	port, err := strconv.Atoi(portText)
	if err != nil {
		t.Fatalf("parse server port: %v", err)
	}
	return port
}

func numericParam(params map[string]interface{}, name string) (int, bool) {
	if params == nil {
		return 0, false
	}
	switch value := params[name].(type) {
	case int:
		return value, true
	case float64:
		return int(value), true
	default:
		return 0, false
	}
}
