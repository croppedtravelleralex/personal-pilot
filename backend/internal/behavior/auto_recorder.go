package behavior

import (
	"encoding/json"
	"fmt"
	"math"
	"math/rand"
	"time"
)

// SimulatedAction represents a single action to dispatch during auto-recording.
type SimulatedAction struct {
	Type     string  // "wait", "scroll", "move", "click", "key"
	DeltaY   float64 // for scroll — pixels to scroll
	X        float64 // for move/click — target X
	Y        float64 // for move/click — target Y
	Key      string  // for key — key to type
	Text     string  // for key — text char
	WaitMs   int64   // for wait — duration in ms
	JitterMs int64   // random ±jitter added to wait
}

type autoRecordJSONWriter interface {
	WriteJSON(v interface{}) error
}

func writeAutoRecordJSON(conn autoRecordJSONWriter, action string, msg map[string]interface{}) error {
	if err := conn.WriteJSON(msg); err != nil {
		return fmt.Errorf("%s: %w", action, err)
	}
	return nil
}

// NurturingActions returns a conservative 养号 (account nurturing) behavior sequence.
// Designed to look like a real human browsing: slow scrolls, reading pauses, no risky clicks.
func NurturingActions() []SimulatedAction {
	return []SimulatedAction{
		// 1. Initial page load settling
		{Type: "wait", WaitMs: 2500, JitterMs: 800},

		// 2. First slow scroll — scanning the feed
		{Type: "scroll", DeltaY: 180, WaitMs: 400, JitterMs: 150},
		{Type: "wait", WaitMs: 3000, JitterMs: 1200},

		// 3. Continue scrolling — reading
		{Type: "scroll", DeltaY: 250, WaitMs: 350, JitterMs: 150},
		{Type: "wait", WaitMs: 4500, JitterMs: 1500},

		// 4. Scroll a bit more
		{Type: "scroll", DeltaY: 200, WaitMs: 380, JitterMs: 150},
		{Type: "wait", WaitMs: 5200, JitterMs: 1800},

		// 5. Scroll down deeper
		{Type: "scroll", DeltaY: 300, WaitMs: 350, JitterMs: 150},
		{Type: "wait", WaitMs: 6000, JitterMs: 2000},

		// 6. Small scroll — micro-adjustment
		{Type: "scroll", DeltaY: 80, WaitMs: 400, JitterMs: 150},
		{Type: "wait", WaitMs: 3500, JitterMs: 1000},

		// 7. Scroll back up a bit — re-reading
		{Type: "scroll", DeltaY: -150, WaitMs: 400, JitterMs: 150},
		{Type: "wait", WaitMs: 4000, JitterMs: 1500},

		// 8. Continue down
		{Type: "scroll", DeltaY: 220, WaitMs: 350, JitterMs: 150},
		{Type: "wait", WaitMs: 5500, JitterMs: 2000},

		// 9. More scrolling
		{Type: "scroll", DeltaY: 180, WaitMs: 380, JitterMs: 150},
		{Type: "wait", WaitMs: 4800, JitterMs: 1500},

		// 10. Long reading pause at interesting content
		{Type: "scroll", DeltaY: 120, WaitMs: 400, JitterMs: 150},
		{Type: "wait", WaitMs: 8000, JitterMs: 3000},

		// 11. Final scrolls
		{Type: "scroll", DeltaY: 250, WaitMs: 350, JitterMs: 150},
		{Type: "wait", WaitMs: 3500, JitterMs: 1000},

		// Total: ~60 seconds of natural browsing
	}
}

// AutoRecord connects to a browser via CDP, injects the recording script,
// dispatches the given simulated actions, then retrieves and returns the recording.
// This allows programmatic recording without any mouse/keyboard control (pure CDP).
func AutoRecord(debugPort int, name string, actions []SimulatedAction) (*Recording, error) {
	conn, err := ConnectPageCDP(debugPort)
	if err != nil {
		return nil, fmt.Errorf("auto-record CDP connect: %w", err)
	}
	defer conn.Close()

	conn.SetReadDeadline(time.Time{}) // disable read deadline for streaming

	if err := writeAutoRecordJSON(conn, "inject recording script", map[string]interface{}{
		"id":     1,
		"method": "Runtime.evaluate",
		"params": map[string]interface{}{
			"expression":    RecordingInjectJS,
			"returnByValue": false,
		},
	}); err != nil {
		return nil, err
	}

	// Read injection response
	conn.SetReadDeadline(time.Now().Add(5 * time.Second))
	if _, _, err := conn.ReadMessage(); err != nil {
		return nil, fmt.Errorf("read injection response: %w", err)
	}
	conn.SetReadDeadline(time.Time{})

	// 2. Dispatch simulated actions. We check write failures immediately, but
	//    don't read action responses to avoid Chrome events polluting ordering.
	msgID := 100
	rng := rand.New(rand.NewSource(time.Now().UnixNano()))

	curX := float64(500 + rng.Intn(400))
	curY := float64(300 + rng.Intn(300))

	for _, action := range actions {
		switch action.Type {
		case "wait":
			waitMs := action.WaitMs
			if action.JitterMs > 0 {
				waitMs += rng.Int63n(action.JitterMs*2) - action.JitterMs
			}
			if waitMs < 0 {
				waitMs = 0
			}
			time.Sleep(time.Duration(waitMs) * time.Millisecond)

		case "scroll":
			curX += float64(rng.Intn(80) - 40)
			curY += float64(rng.Intn(40) - 20)
			if curX < 100 {
				curX = 100
			}
			if curY < 100 {
				curY = 100
			}
			msgID++
			if err := writeAutoRecordJSON(conn, "dispatch scroll mouse move", map[string]interface{}{
				"id":     msgID,
				"method": "Input.dispatchMouseEvent",
				"params": map[string]interface{}{
					"type": "mouseMoved",
					"x":    math.Round(curX),
					"y":    math.Round(curY),
				},
			}); err != nil {
				return nil, err
			}
			time.Sleep(time.Duration(50+rng.Intn(100)) * time.Millisecond)

			scrollY := action.DeltaY
			steps := 4 + rng.Intn(4)
			stepPx := scrollY / float64(steps)
			for step := 0; step < steps; step++ {
				msgID++
				dy := int(math.Round(stepPx)) + rng.Intn(5) - 2
				if err := writeAutoRecordJSON(conn, "dispatch scroll wheel", map[string]interface{}{
					"id":     msgID,
					"method": "Input.dispatchMouseEvent",
					"params": map[string]interface{}{
						"type":   "mouseWheel",
						"x":      math.Round(curX),
						"y":      math.Round(curY),
						"deltaX": 0,
						"deltaY": float64(dy),
					},
				}); err != nil {
					return nil, err
				}
				time.Sleep(time.Duration(8+rng.Intn(12)) * time.Millisecond)

				msgID++
				js := fmt.Sprintf("window.scrollBy({top: %d, behavior: 'auto'})", dy)
				if err := writeAutoRecordJSON(conn, "dispatch scroll fallback", map[string]interface{}{
					"id":     msgID,
					"method": "Runtime.evaluate",
					"params": map[string]interface{}{
						"expression":    js,
						"returnByValue": false,
					},
				}); err != nil {
					return nil, err
				}
				time.Sleep(time.Duration(20+rng.Intn(40)) * time.Millisecond)
			}

			waitMs := action.WaitMs
			if action.JitterMs > 0 {
				waitMs += rng.Int63n(action.JitterMs*2) - action.JitterMs
			}
			if waitMs > 0 {
				time.Sleep(time.Duration(waitMs) * time.Millisecond)
			}

		case "move":
			fromX, fromY := curX, curY
			toX, toY := action.X, action.Y
			if toX == 0 {
				toX = fromX
			}
			if toY == 0 {
				toY = fromY
			}
			moveSteps := 3 + rng.Intn(5)
			for i := 1; i <= moveSteps; i++ {
				t := float64(i) / float64(moveSteps)
				ix := fromX + (toX-fromX)*t + float64(rng.Intn(5)-2)
				iy := fromY + (toY-fromY)*t + float64(rng.Intn(5)-2)
				msgID++
				if err := writeAutoRecordJSON(conn, "dispatch move", map[string]interface{}{
					"id":     msgID,
					"method": "Input.dispatchMouseEvent",
					"params": map[string]interface{}{
						"type": "mouseMoved",
						"x":    math.Round(ix),
						"y":    math.Round(iy),
					},
				}); err != nil {
					return nil, err
				}
				time.Sleep(time.Duration(8+rng.Intn(12)) * time.Millisecond)
			}
			curX, curY = toX, toY

		case "click":
			toX, toY := action.X, action.Y
			if toX == 0 {
				toX = curX
			}
			if toY == 0 {
				toY = curY
			}
			moveSteps := 3 + rng.Intn(3)
			for i := 1; i <= moveSteps; i++ {
				t := float64(i) / float64(moveSteps)
				ix := curX + (toX-curX)*t + float64(rng.Intn(3)-1)
				iy := curY + (toY-curY)*t + float64(rng.Intn(3)-1)
				msgID++
				if err := writeAutoRecordJSON(conn, "dispatch click move", map[string]interface{}{
					"id":     msgID,
					"method": "Input.dispatchMouseEvent",
					"params": map[string]interface{}{
						"type": "mouseMoved",
						"x":    math.Round(ix),
						"y":    math.Round(iy),
					},
				}); err != nil {
					return nil, err
				}
				time.Sleep(time.Duration(8+rng.Intn(10)) * time.Millisecond)
			}
			curX, curY = toX, toY
			msgID++
			if err := writeAutoRecordJSON(conn, "dispatch click press", map[string]interface{}{
				"id":     msgID,
				"method": "Input.dispatchMouseEvent",
				"params": map[string]interface{}{
					"type":       "mousePressed",
					"x":          math.Round(curX),
					"y":          math.Round(curY),
					"button":     "left",
					"clickCount": 1,
				},
			}); err != nil {
				return nil, err
			}
			time.Sleep(time.Duration(30+rng.Intn(70)) * time.Millisecond)
			msgID++
			if err := writeAutoRecordJSON(conn, "dispatch click release", map[string]interface{}{
				"id":     msgID,
				"method": "Input.dispatchMouseEvent",
				"params": map[string]interface{}{
					"type":       "mouseReleased",
					"x":          math.Round(curX),
					"y":          math.Round(curY),
					"button":     "left",
					"clickCount": 1,
				},
			}); err != nil {
				return nil, err
			}

		case "key":
			if action.Key != "" {
				msgID++
				if err := writeAutoRecordJSON(conn, "dispatch key down", map[string]interface{}{
					"id":     msgID,
					"method": "Input.dispatchKeyEvent",
					"params": map[string]interface{}{
						"type": "rawKeyDown",
						"key":  action.Key,
					},
				}); err != nil {
					return nil, err
				}
				if action.Text != "" {
					msgID++
					if err := writeAutoRecordJSON(conn, "dispatch key char", map[string]interface{}{
						"id":     msgID,
						"method": "Input.dispatchKeyEvent",
						"params": map[string]interface{}{
							"type": "char",
							"text": action.Text,
							"key":  action.Key,
						},
					}); err != nil {
						return nil, err
					}
				}
				msgID++
				if err := writeAutoRecordJSON(conn, "dispatch key up", map[string]interface{}{
					"id":     msgID,
					"method": "Input.dispatchKeyEvent",
					"params": map[string]interface{}{
						"type": "keyUp",
						"key":  action.Key,
					},
				}); err != nil {
					return nil, err
				}
				time.Sleep(time.Duration(20+rng.Intn(40)) * time.Millisecond)
			}

		default:
		}
	}

	// 3. Retrieve events — send command, then loop-read messages,
	// skipping CDP events (no id field) until we find the response
	// to our Runtime.evaluate command (id=2).
	// We use a single long read deadline to avoid breaking the
	// websocket connection (gorilla treats deadline expiry as fatal).
	retrieveJS := `JSON.stringify(window.__antRecordedEvents || [])`
	if err := writeAutoRecordJSON(conn, "retrieve events", map[string]interface{}{
		"id":     2,
		"method": "Runtime.evaluate",
		"params": map[string]interface{}{
			"expression":    retrieveJS,
			"returnByValue": true,
		},
	}); err != nil {
		return nil, err
	}

	var raw []byte
	deadline := time.Now().Add(30 * time.Second)
	for time.Now().Before(deadline) {
		conn.SetReadDeadline(deadline)
		_, msg, err := conn.ReadMessage()
		if err != nil {
			// Deadline or connection drop — stop reading
			break
		}
		var probe struct {
			ID     int `json:"id"`
			Result struct {
				Result struct {
					Value string `json:"value"`
				} `json:"result"`
			} `json:"result"`
		}
		if json.Unmarshal(msg, &probe) == nil && probe.ID == 2 {
			raw = msg
			break
		}
	}
	conn.SetReadDeadline(time.Time{})
	if raw == nil {
		return nil, fmt.Errorf("read events response: no valid CDP response within timeout")
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
		return nil, fmt.Errorf("no events recorded")
	}

	var events []RecordedEvent
	if err := json.Unmarshal([]byte(eventsJSON), &events); err != nil {
		return nil, fmt.Errorf("parse events JSON: %w", err)
	}

	// 4. Get viewport — same safe loop-read pattern
	viewportW, viewportH := 1920, 1080
	vpMsg := map[string]interface{}{
		"id":     3,
		"method": "Runtime.evaluate",
		"params": map[string]interface{}{
			"expression":    `JSON.stringify({w: window.innerWidth, h: window.innerHeight})`,
			"returnByValue": true,
		},
	}
	if err := writeAutoRecordJSON(conn, "retrieve viewport", vpMsg); err != nil {
		return nil, err
	}
	vpDeadline := time.Now().Add(5 * time.Second)
	for time.Now().Before(vpDeadline) {
		conn.SetReadDeadline(vpDeadline)
		_, vpRaw, vpErr := conn.ReadMessage()
		if vpErr != nil {
			break
		}
		var vpProbe struct {
			ID     int `json:"id"`
			Result struct {
				Result struct {
					Value string `json:"value"`
				} `json:"result"`
			} `json:"result"`
		}
		if json.Unmarshal(vpRaw, &vpProbe) == nil && vpProbe.ID == 3 {
			if vpProbe.Result.Result.Value != "" {
				var vp struct {
					W int `json:"w"`
					H int `json:"h"`
				}
				if json.Unmarshal([]byte(vpProbe.Result.Result.Value), &vp) == nil {
					if vp.W > 0 {
						viewportW = vp.W
					}
					if vp.H > 0 {
						viewportH = vp.H
					}
				}
			}
			break
		}
	}
	conn.SetReadDeadline(time.Time{})

	// 5. Teardown
	teardownJS := `delete window.__antRecordedEvents; delete window.__antRecorder;`
	if err := writeAutoRecordJSON(conn, "teardown recording script", map[string]interface{}{
		"id":     4,
		"method": "Runtime.evaluate",
		"params": map[string]interface{}{
			"expression":    teardownJS,
			"returnByValue": false,
		},
	}); err != nil {
		return nil, err
	}

	duration := int64(0)
	if len(events) > 0 {
		duration = events[len(events)-1].T
	}

	recording := &Recording{
		ID:          generateID(),
		Name:        name,
		Description: "自动生成的养号行为录制",
		Events:      events,
		DurationMs:  duration,
		ViewportW:   viewportW,
		ViewportH:   viewportH,
		CreatedAt:   time.Now().Format(time.RFC3339),
	}

	return recording, nil
}
