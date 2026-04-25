package behavior

import (
	"encoding/json"
	"fmt"
	"math"
	"math/rand"
	"net/http"
	"time"

	"github.com/gorilla/websocket"
)

// cdpConn holds a connected CDP WebSocket.
type cdpConn struct {
	ws     *websocket.Conn
	msgID  int
	closed bool
}

var (
	behaviorHTTPClient = &http.Client{Timeout: 5 * time.Second}
	behaviorWSDialer   = &websocket.Dialer{HandshakeTimeout: 5 * time.Second}
)

// connectCDP dials the browser's CDP endpoint and returns a connection.
func connectCDP(debugPort int) (*cdpConn, error) {
	resp, err := behaviorHTTPClient.Get(fmt.Sprintf("http://127.0.0.1:%d/json/version", debugPort))
	if err != nil {
		return nil, fmt.Errorf("cdp connect: %w", err)
	}
	defer resp.Body.Close()

	var ver struct {
		WebSocketDebuggerURL string `json:"webSocketDebuggerUrl"`
	}
	if err := json.NewDecoder(resp.Body).Decode(&ver); err != nil {
		return nil, fmt.Errorf("decode /json/version: %w", err)
	}
	if ver.WebSocketDebuggerURL == "" {
		return nil, fmt.Errorf("no webSocketDebuggerUrl")
	}

	ws, _, err := behaviorWSDialer.Dial(ver.WebSocketDebuggerURL, nil)
	if err != nil {
		return nil, fmt.Errorf("ws dial: %w", err)
	}

	return &cdpConn{ws: ws}, nil
}

func (c *cdpConn) Close() {
	if !c.closed {
		c.closed = true
		c.ws.Close()
	}
}

// sendCommand sends a CDP command and returns the raw result.
func (c *cdpConn) sendCommand(method string, params interface{}) (json.RawMessage, error) {
	c.msgID++
	type req struct {
		ID     int         `json:"id"`
		Method string      `json:"method"`
		Params interface{} `json:"params,omitempty"`
	}
	type resp struct {
		ID     int             `json:"id"`
		Result json.RawMessage `json:"result"`
		Error  *struct {
			Message string `json:"message"`
		} `json:"error"`
	}

	if err := c.ws.WriteJSON(req{ID: c.msgID, Method: method, Params: params}); err != nil {
		return nil, fmt.Errorf("write %s: %w", method, err)
	}

	c.ws.SetReadDeadline(time.Now().Add(5 * time.Second))
	var r resp
	if err := c.ws.ReadJSON(&r); err != nil {
		return nil, fmt.Errorf("read %s resp: %w", method, err)
	}
	if r.Error != nil {
		return nil, fmt.Errorf("cdp error %s: %s", method, r.Error.Message)
	}
	return r.Result, nil
}

// mouseMove generates a sequence of mouse move events along a bezier path.
func (c *cdpConn) mouseMove(fromX, fromY, toX, toY float64, profile MouseProfile) error {
	steps := int(math.Max(10, math.Sqrt(math.Pow(toX-fromX, 2)+math.Pow(toY-fromY, 2))/15))
	// Clamp steps to a reasonable range
	if steps > 80 {
		steps = 80
	}
	if steps < 5 {
		steps = 5
	}

	for i := 0; i <= steps; i++ {
		t := float64(i) / float64(steps)

		// Bezier curve blending — use one or two control points.
		var bx, by float64
		switch profile.CurveStyle {
		case "bezier3":
			// Cubic bezier with 2 control points
			cp1x := fromX + (toX-fromX)*0.3 + randOff(profile.JitterPx)
			cp1y := fromY - 50 + randOff(profile.JitterPx)
			cp2x := fromX + (toX-fromX)*0.7 + randOff(profile.JitterPx)
			cp2y := toY + 50 + randOff(profile.JitterPx)
			u := 1 - t
			bx = u*u*u*fromX + 3*u*u*t*cp1x + 3*u*t*t*cp2x + t*t*t*toX
			by = u*u*u*fromY + 3*u*u*t*cp1y + 3*u*t*t*cp2y + t*t*t*toY
		case "natural":
			// Simpler bezier2 but with speed variation implemented below
			cp1x := fromX + (toX-fromX)*0.5 + randOff(profile.JitterPx*2)
			cp1y := fromY - 30 + randOff(profile.JitterPx*2)
			u := 1 - t
			bx = u*u*fromX + 2*u*t*cp1x + t*t*toX
			by = u*u*fromY + 2*u*t*cp1y + t*t*toY
		default: // bezier2
			cp1x := fromX + (toX-fromX)*0.4 + randOff(profile.JitterPx)
			cp1y := fromY - 40 + randOff(profile.JitterPx)
			u := 1 - t
			bx = u*u*fromX + 2*u*t*cp1x + t*t*toX
			by = u*u*fromY + 2*u*t*cp1y + t*t*toY
		}

		x := bx + randOff(profile.JitterPx)
		y := by + randOff(profile.JitterPx)

		if err := c.dispatchMouseEvent("mouseMoved", x, y, "none", 0); err != nil {
			return err
		}

		// Variable speed — longer delay at start/end, faster in middle
		baseDelay := 1000.0 / profile.SpeedMean
		speedFactor := 1.0 + math.Sin(math.Pi*t)*0.5 // slower at ends
		delay := time.Duration(baseDelay * speedFactor * float64(time.Millisecond))
		time.Sleep(delay)

		// Random micro-pause
		if rand.Float64() < profile.PauseProb {
			pauseMs := rand.Intn(profile.PauseMaxMs + 1)
			if pauseMs > 0 {
				time.Sleep(time.Duration(pauseMs) * time.Millisecond)
			}
		}
	}
	return nil
}

// dispatchMouseEvent sends a single CDP Input.dispatchMouseEvent.
func (c *cdpConn) dispatchMouseEvent(typ string, x, y float64, button string, clickCount int) error {
	params := map[string]interface{}{
		"type":       typ,
		"x":          x,
		"y":          y,
		"button":     button,
		"clickCount": clickCount,
	}
	_, err := c.sendCommand("Input.dispatchMouseEvent", params)
	return err
}

// scroll injects a human-like scroll sequence via JS.
func (c *cdpConn) scrollHuman(scroll ScrollProfile) error {
	steps := 2 + rand.Intn(4)
	totalPx := float64(scroll.ScrollStepPx) + randOff(scroll.ScrollStepStdDev)
	totalPx = math.Max(20, totalPx)

	stepPx := totalPx / float64(steps)

	for i := 0; i < steps; i++ {
		// Scroll down in small increments
		js := fmt.Sprintf("window.scrollBy({top: %d, behavior: 'auto'})", int(stepPx)+rand.Intn(10))
		if _, err := c.sendCommand("Runtime.evaluate", map[string]interface{}{
			"expression": js,
		}); err != nil {
			return err
		}

		delay := time.Duration(scroll.PauseBetweenMs/steps) +
			time.Duration(rand.Intn(scroll.PauseStdDevMs))*time.Millisecond
		time.Sleep(delay)
	}

	// Occasional overscroll correction
	if rand.Float64() < scroll.OverscrollProb {
		js := fmt.Sprintf("window.scrollBy({top: %d, behavior: 'auto'})", -10-rand.Intn(20))
		c.sendCommand("Runtime.evaluate", map[string]interface{}{"expression": js})
		time.Sleep(100 * time.Millisecond)
	}

	// Occasional reverse scroll (re-read something)
	if rand.Float64() < scroll.ReverseProb {
		js := fmt.Sprintf("window.scrollBy({top: %d, behavior: 'auto'})", -30-rand.Intn(50))
		c.sendCommand("Runtime.evaluate", map[string]interface{}{"expression": js})
	}

	return nil
}

// randOff returns a random offset in [-max, max].
func randOff(maxPx int) float64 {
	if maxPx <= 0 {
		return 0
	}
	return float64(rand.Intn(2*maxPx+1) - maxPx)
}
