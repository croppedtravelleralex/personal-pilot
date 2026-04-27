package behavior

import (
	"context"
	"encoding/json"
	"fmt"
	"math"
	"math/rand"
	"time"

	"github.com/gorilla/websocket"
)

// PlaybackEngine replays a recording through CDP with configurable variation.
type PlaybackEngine struct {
	recording *Recording
	variation VariationConfig
	wsConn    *websocket.Conn
	cancel    context.CancelFunc
	rng       *rand.Rand
}

// NewPlaybackEngine creates a new playback engine.
func NewPlaybackEngine(recording *Recording, variation VariationConfig) *PlaybackEngine {
	return &PlaybackEngine{
		recording: recording,
		variation: variation,
		rng:       rand.New(rand.NewSource(time.Now().UnixNano())),
	}
}

// Play starts playback on the given browser instance via CDP.
// Returns a cancel function to stop playback mid-way.
func (e *PlaybackEngine) Play(ctx context.Context, debugPort int) error {
	conn, _, err := websocket.DefaultDialer.Dial(
		fmt.Sprintf("ws://127.0.0.1:%d/devtools/page", debugPort), nil,
	)
	if err != nil {
		return fmt.Errorf("connect to CDP page endpoint: %w", err)
	}
	e.wsConn = conn

	ctx, cancel := context.WithCancel(ctx)
	e.cancel = cancel

	go func() {
		<-ctx.Done()
		if e.wsConn != nil {
			e.wsConn.Close()
		}
	}()

	return e.run(ctx)
}

// Stop cancels an in-progress playback.
func (e *PlaybackEngine) Stop() {
	if e.cancel != nil {
		e.cancel()
	}
}

func (e *PlaybackEngine) run(ctx context.Context) error {
	events := e.recording.Events
	if len(events) == 0 {
		return nil
	}

	lastX := 0.0
	lastY := 0.0
	msgID := 1

	for i, evt := range events {
		select {
		case <-ctx.Done():
			return ctx.Err()
		default:
		}

		// Calculate delay until this event
		var targetT int64
		if i == 0 {
			targetT = 0
		} else {
			prevT := events[i-1].T
			rawDelay := evt.T - prevT
			// Apply timing jitter
			jitter := int64(math.Abs(e.gaussian(0, e.variation.Intensity*e.variation.TimingJitter)))
			targetT = prevT + rawDelay + jitter
		}

		// Sleep until target time
		if i > 0 {
			sleepMs := targetT - events[i-1].T
			if sleepMs > 0 {
				select {
				case <-ctx.Done():
					return ctx.Err()
				case <-time.After(time.Duration(sleepMs) * time.Millisecond):
				}
			}
		}

		// Apply position offset
		x := evt.X
		y := evt.Y
		if e.variation.Intensity > 0 && (evt.Type == "move" || evt.Type == "down" || evt.Type == "up" || evt.Type == "click") {
			x += e.gaussian(0, e.variation.Intensity*e.variation.PositionJitter)
			y += e.gaussian(0, e.variation.Intensity*e.variation.PositionJitter)
		}

		switch evt.Type {
		case "move":
			if err := e.dispatchMouseMove(msgID, lastX, lastY, x, y); err != nil {
				return fmt.Errorf("move event failed: %w", err)
			}
			msgID++
			lastX, lastY = x, y

		case "down":
			if err := e.dispatchMouseEvent(msgID, "mousePressed", x, y, evt.Button); err != nil {
				return fmt.Errorf("mouseDown event failed: %w", err)
			}
			msgID++
			lastX, lastY = x, y

		case "up":
			if err := e.dispatchMouseEvent(msgID, "mouseReleased", x, y, evt.Button); err != nil {
				return fmt.Errorf("mouseUp event failed: %w", err)
			}
			msgID++

		case "click":
			if err := e.dispatchMouseEvent(msgID, "mousePressed", x, y, evt.Button); err != nil {
				return fmt.Errorf("click press failed: %w", err)
			}
			msgID++
			// Small delay between down and up
			time.Sleep(time.Duration(30+e.rng.Intn(70)) * time.Millisecond)
			if err := e.dispatchMouseEvent(msgID, "mouseReleased", x, y, evt.Button); err != nil {
				return fmt.Errorf("click release failed: %w", err)
			}
			msgID++

		case "key":
			if err := e.dispatchKeyEvent(msgID, "rawKeyDown", evt.Key, evt.Text); err != nil {
				return fmt.Errorf("keyDown event failed: %w", err)
			}
			msgID++
			if evt.Text != "" {
				if err := e.dispatchKeyEvent(msgID, "char", evt.Key, evt.Text); err != nil {
					return fmt.Errorf("keyChar event failed: %w", err)
				}
				msgID++
			}
			if err := e.dispatchKeyEvent(msgID, "keyUp", evt.Key, ""); err != nil {
				return fmt.Errorf("keyUp event failed: %w", err)
			}
			msgID++

		case "scroll":
			if err := e.dispatchScroll(msgID, evt.DeltaX, evt.DeltaY); err != nil {
				return fmt.Errorf("scroll event failed: %w", err)
			}
			msgID++
		}

		// Micro-corrections: small overshoot/undershoot after click
		if e.variation.MicroCorrections && evt.Type == "click" && e.rng.Float64() < 0.3 {
			correctionMs := 50 + e.rng.Intn(150)
			select {
			case <-ctx.Done():
				return ctx.Err()
			case <-time.After(time.Duration(correctionMs) * time.Millisecond):
			}
			overshootX := x + float64(e.rng.Intn(3)-1)
			overshootY := y + float64(e.rng.Intn(3)-1)
			if err := e.dispatchMouseMove(msgID, x, y, overshootX, overshootY); err != nil {
				return fmt.Errorf("micro-correction failed: %w", err)
			}
			msgID++
			lastX, lastY = overshootX, overshootY
		}

		// Extra pauses
		if e.variation.ExtraPauses && i > 0 && i < len(events)-1 {
			evtT := events[i].T
			nextT := events[i+1].T
			gap := nextT - evtT
			// Only add pauses between long gaps
			if gap > 1000 && e.rng.Float64() < 0.2 {
				pauseMs := 200 + e.rng.Intn(600)
				select {
				case <-ctx.Done():
					return ctx.Err()
				case <-time.After(time.Duration(pauseMs) * time.Millisecond):
				}
			}
		}
	}

	return nil
}

func (e *PlaybackEngine) dispatchMouseMove(msgID int, fromX, fromY, toX, toY float64) error {
	// Use simple linear interpolation for replay (not bezier - replay already has the path)
	steps := 5 + e.rng.Intn(5)
	for step := 0; step <= steps; step++ {
		t := float64(step) / float64(steps)
		x := fromX + (toX-fromX)*t
		y := fromY + (toY-fromY)*t
		if e.variation.Intensity > 0 {
			x += e.gaussian(0, 1)
			y += e.gaussian(0, 1)
		}
		if err := e.sendMessage(map[string]interface{}{
			"id":     msgID*1000 + step,
			"method": "Input.dispatchMouseEvent",
			"params": map[string]interface{}{
				"type": "mouseMoved",
				"x":    math.Round(x),
				"y":    math.Round(y),
			},
		}); err != nil {
			return err
		}
		time.Sleep(time.Duration(8+e.rng.Intn(8)) * time.Millisecond)
	}
	return nil
}

func (e *PlaybackEngine) dispatchMouseEvent(msgID int, eventType string, x, y float64, button int) error {
	btn := "left"
	if button == 1 {
		btn = "middle"
	} else if button == 2 {
		btn = "right"
	}

	return e.sendMessage(map[string]interface{}{
		"id":     msgID,
		"method": "Input.dispatchMouseEvent",
		"params": map[string]interface{}{
			"type":   eventType,
			"x":      math.Round(x),
			"y":      math.Round(y),
			"button": btn,
			"clickCount": 1,
		},
	})
}

func (e *PlaybackEngine) dispatchKeyEvent(msgID int, eventType string, key, text string) error {
	params := map[string]interface{}{
		"type": eventType,
		"key":  key,
	}
	if eventType == "char" && text != "" {
		params["type"] = "char"
		params["text"] = text
	} else if text != "" {
		params["text"] = text
	}

	return e.sendMessage(map[string]interface{}{
		"id":     msgID,
		"method": "Input.dispatchKeyEvent",
		"params": params,
	})
}

func (e *PlaybackEngine) dispatchScroll(msgID int, deltaX, deltaY float64) error {
	// Scroll via injected JS for smoother scrolling
	js := fmt.Sprintf("window.scrollBy(%d, %d)", int(math.Round(deltaX)), int(math.Round(deltaY)))
	if err := e.sendMessage(map[string]interface{}{
		"id":     msgID,
		"method": "Runtime.evaluate",
		"params": map[string]interface{}{
			"expression":    js,
			"returnByValue": false,
		},
	}); err != nil {
		return err
	}

	// Also dispatch a wheel event for completeness
	return e.sendMessage(map[string]interface{}{
		"id":     msgID * 1000,
		"method": "Input.dispatchMouseEvent",
		"params": map[string]interface{}{
			"type":   "mouseWheel",
			"x":      0,
			"y":      0,
			"deltaX": math.Round(deltaX),
			"deltaY": math.Round(deltaY),
		},
	})
}

func (e *PlaybackEngine) sendMessage(msg map[string]interface{}) error {
	if e.wsConn == nil {
		return fmt.Errorf("websocket not connected")
	}
	data, err := json.Marshal(msg)
	if err != nil {
		return fmt.Errorf("marshal message: %w", err)
	}
	if err := e.wsConn.WriteMessage(websocket.TextMessage, data); err != nil {
		return fmt.Errorf("write websocket message: %w", err)
	}
	return nil
}

// gaussian generates a random number from a normal distribution (Box-Muller).
func (e *PlaybackEngine) gaussian(mean, stddev float64) float64 {
	if stddev <= 0 {
		return mean
	}
	u1 := e.rng.Float64()
	u2 := e.rng.Float64()
	z := math.Sqrt(-2*math.Log(u1+1e-10)) * math.Cos(2*math.Pi*u2)
	return mean + z*stddev
}
