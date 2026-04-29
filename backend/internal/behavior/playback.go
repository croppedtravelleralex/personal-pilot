package behavior

import (
	"context"
	"encoding/json"
	"fmt"
	"math"
	"math/rand"
	"reflect"
	"time"

	"github.com/gorilla/websocket"
)

// PlaybackEngine replays a recording through CDP with configurable variation.
type PlaybackEngine struct {
	recording        *Recording
	variation        VariationConfig
	wsConn           *websocket.Conn
	cancel           context.CancelFunc
	rng              *rand.Rand
	viewport         playbackViewport
	progressCallback func(PlaybackProgress)
}

type playbackViewport struct {
	Width  int
	Height int
	DPR    float64
	Scale  float64
}

// PlaybackProgress describes replay progress for UI event emission.
type PlaybackProgress struct {
	RecordingID string  `json:"recordingId"`
	EventIndex  int     `json:"eventIndex"`
	EventTotal  int     `json:"eventTotal"`
	Percent     float64 `json:"percent"`
	ElapsedMs   int64   `json:"elapsedMs"`
	Status      string  `json:"status"`
}

// NewPlaybackEngine creates a new playback engine.
func NewPlaybackEngine(recording *Recording, variation VariationConfig) *PlaybackEngine {
	return &PlaybackEngine{
		recording: recording,
		variation: variation,
		rng:       rand.New(rand.NewSource(time.Now().UnixNano())),
	}
}

// SetProgressCallback registers a callback invoked during playback progress.
func (e *PlaybackEngine) SetProgressCallback(callback func(PlaybackProgress)) {
	e.progressCallback = callback
}

// Play starts playback on the given browser instance via CDP.
// Returns a cancel function to stop playback mid-way.
func (e *PlaybackEngine) Play(ctx context.Context, debugPort int) error {
	if ctx == nil {
		ctx = context.Background()
	}
	startedAt := time.Now()
	conn, err := ConnectPageCDP(debugPort)
	if err != nil {
		e.emitProgress(0, e.recordingEventTotal(), startedAt, "failed")
		return fmt.Errorf("connect to CDP page endpoint: %w", err)
	}
	e.wsConn = conn
	viewport, err := e.readCurrentViewport()
	if err != nil {
		conn.Close()
		e.emitProgress(0, e.recordingEventTotal(), startedAt, "failed")
		return fmt.Errorf("read current viewport: %w", err)
	}
	e.viewport = viewport

	ctx, cancel := context.WithCancel(ctx)
	e.cancel = cancel

	go func() {
		<-ctx.Done()
		if e.wsConn != nil {
			e.wsConn.Close()
		}
	}()

	err = e.run(ctx)
	// Cancel context to release the goroutine above (prevents leak on normal completion)
	cancel()
	return err
}

// Stop cancels an in-progress playback.
func (e *PlaybackEngine) Stop() {
	if e.cancel != nil {
		e.cancel()
	}
}

func (e *PlaybackEngine) run(ctx context.Context) (err error) {
	if ctx == nil {
		ctx = context.Background()
	}
	events := []RecordedEvent{}
	if e.recording != nil {
		events = normalizeRecordedEvents(e.recording.Events)
	}
	startedAt := time.Now()
	lastProgressIndex := 0
	defer func() {
		if err != nil {
			status := "failed"
			if ctx.Err() != nil {
				status = "stopped"
			}
			e.emitProgress(lastProgressIndex, len(events), startedAt, status)
		}
	}()
	if len(events) == 0 {
		e.emitProgress(0, 0, startedAt, "completed")
		return nil
	}

	recordedViewport := recordingPlaybackViewport(e.recording)
	currentViewport := e.viewport
	if !currentViewport.valid() {
		currentViewport = recordedViewport
	}

	lastX := 0.0
	lastY := 0.0
	msgID := 100

	for i, evt := range events {
		select {
		case <-ctx.Done():
			return ctx.Err()
		default:
		}

		if i > 0 {
			prevT := events[i-1].T
			rawDelay := evt.T - prevT
			sleepMs := computePlaybackDelayMs(rawDelay, e.variation, e.rng)
			if sleepMs > 0 {
				select {
				case <-ctx.Done():
					return ctx.Err()
				case <-time.After(time.Duration(sleepMs) * time.Millisecond):
				}
			}
		}

		x, y := mapRecordedPoint(evt.X, evt.Y, recordedViewport, currentViewport)
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
			lastX, lastY = x, y

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
			scrollX, scrollY := resolveScrollPoint(evt, lastX, lastY, recordedViewport, currentViewport)
			handled := false
			if evt.TargetPath != "" {
				var err error
				handled, err = e.scrollRecordedTarget(msgID, evt.TargetPath, evt.DeltaX, evt.DeltaY)
				if err != nil {
					return fmt.Errorf("scroll target event failed: %w", err)
				}
				msgID++
			}
			if !handled {
				if err := e.dispatchScroll(msgID, scrollX, scrollY, evt.DeltaX, evt.DeltaY); err != nil {
					return fmt.Errorf("scroll event failed: %w", err)
				}
				msgID++
			}
			lastX, lastY = scrollX, scrollY
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

		lastProgressIndex = i + 1
		e.emitProgress(lastProgressIndex, len(events), startedAt, "running")

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

	e.emitProgress(len(events), len(events), startedAt, "completed")
	return nil
}

func (e *PlaybackEngine) recordingEventTotal() int {
	if e == nil || e.recording == nil {
		return 0
	}
	return len(normalizeRecordedEvents(e.recording.Events))
}

func (e *PlaybackEngine) emitProgress(eventIndex int, eventTotal int, startedAt time.Time, status string) {
	if e == nil || e.progressCallback == nil {
		return
	}
	if eventIndex < 0 {
		eventIndex = 0
	}
	if eventTotal < 0 {
		eventTotal = 0
	}
	if eventIndex > eventTotal {
		eventIndex = eventTotal
	}

	percent := 0.0
	if eventTotal > 0 {
		percent = (float64(eventIndex) / float64(eventTotal)) * 100
	} else if status == "completed" {
		percent = 100
	}
	if percent < 0 {
		percent = 0
	}
	if percent > 100 {
		percent = 100
	}

	elapsedMs := int64(0)
	if !startedAt.IsZero() {
		elapsedMs = time.Since(startedAt).Milliseconds()
		if elapsedMs < 0 {
			elapsedMs = 0
		}
	}

	recordingID := ""
	if e.recording != nil {
		recordingID = e.recording.ID
	}
	e.progressCallback(PlaybackProgress{
		RecordingID: recordingID,
		EventIndex:  eventIndex,
		EventTotal:  eventTotal,
		Percent:     percent,
		ElapsedMs:   elapsedMs,
		Status:      status,
	})
}

func (e *PlaybackEngine) readCurrentViewport() (playbackViewport, error) {
	raw, err := e.sendCommand(1, "Runtime.evaluate", map[string]interface{}{
		"expression": `JSON.stringify({
			w: window.innerWidth || document.documentElement.clientWidth || 0,
			h: window.innerHeight || document.documentElement.clientHeight || 0,
			dpr: window.devicePixelRatio || 1,
			scale: (window.visualViewport && window.visualViewport.scale) || 1
		})`,
		"returnByValue": true,
	})
	if err != nil {
		return playbackViewport{}, err
	}

	var response struct {
		Result struct {
			Value string `json:"value"`
		} `json:"result"`
	}
	if err := json.Unmarshal(raw, &response); err != nil {
		return playbackViewport{}, fmt.Errorf("parse viewport response: %w", err)
	}

	var viewport struct {
		W     int     `json:"w"`
		H     int     `json:"h"`
		DPR   float64 `json:"dpr"`
		Scale float64 `json:"scale"`
	}
	if response.Result.Value == "" {
		return playbackViewport{}, fmt.Errorf("empty viewport response")
	}
	if err := json.Unmarshal([]byte(response.Result.Value), &viewport); err != nil {
		return playbackViewport{}, fmt.Errorf("parse viewport value: %w", err)
	}

	return sanitizePlaybackViewport(playbackViewport{
		Width:  viewport.W,
		Height: viewport.H,
		DPR:    viewport.DPR,
		Scale:  viewport.Scale,
	}), nil
}

func recordingPlaybackViewport(recording interface{}) playbackViewport {
	viewport := playbackViewport{DPR: 1, Scale: 1}
	if recording == nil {
		return viewport
	}

	value := reflect.ValueOf(recording)
	if value.Kind() == reflect.Pointer {
		if value.IsNil() {
			return viewport
		}
		value = value.Elem()
	}
	if value.Kind() != reflect.Struct {
		return viewport
	}

	viewport.Width = int(readNumericField(value, "ViewportW", "ViewportWidth", "Width"))
	viewport.Height = int(readNumericField(value, "ViewportH", "ViewportHeight", "Height"))
	viewport.DPR = readNumericField(value, "ViewportDPR", "DevicePixelRatio", "DPR", "DeviceScaleFactor")
	viewport.Scale = readNumericField(value, "ViewportScale", "VisualViewportScale", "Scale", "PageScaleFactor")
	return sanitizePlaybackViewport(viewport)
}

func readNumericField(value reflect.Value, names ...string) float64 {
	for _, name := range names {
		field := value.FieldByName(name)
		if !field.IsValid() {
			continue
		}
		switch field.Kind() {
		case reflect.Int, reflect.Int8, reflect.Int16, reflect.Int32, reflect.Int64:
			return float64(field.Int())
		case reflect.Uint, reflect.Uint8, reflect.Uint16, reflect.Uint32, reflect.Uint64:
			return float64(field.Uint())
		case reflect.Float32, reflect.Float64:
			return field.Float()
		}
	}
	return 0
}

func sanitizePlaybackViewport(viewport playbackViewport) playbackViewport {
	if viewport.DPR <= 0 || math.IsNaN(viewport.DPR) || math.IsInf(viewport.DPR, 0) {
		viewport.DPR = 1
	}
	if viewport.Scale <= 0 || math.IsNaN(viewport.Scale) || math.IsInf(viewport.Scale, 0) {
		viewport.Scale = 1
	}
	return viewport
}

func (v playbackViewport) valid() bool {
	return v.Width > 0 && v.Height > 0
}

func mapRecordedPoint(x, y float64, recorded, current playbackViewport) (float64, float64) {
	scaleX, scaleY := viewportScale(recorded, current)
	return x * scaleX, y * scaleY
}

func viewportScale(recorded, current playbackViewport) (float64, float64) {
	recorded = sanitizePlaybackViewport(recorded)
	current = sanitizePlaybackViewport(current)
	if !recorded.valid() || !current.valid() {
		return 1, 1
	}

	// MouseEvent.clientX/clientY and CDP Input coordinates are CSS pixels.
	// DPR/scale metadata is retained for future physical-pixel/container mapping
	// without changing legacy recordings that only stored CSS viewport size.
	return float64(current.Width) / float64(recorded.Width),
		float64(current.Height) / float64(recorded.Height)
}

func resolveScrollPoint(evt RecordedEvent, lastX, lastY float64, recorded, current playbackViewport) (float64, float64) {
	if evt.X != 0 || evt.Y != 0 {
		return mapRecordedPoint(evt.X, evt.Y, recorded, current)
	}
	if lastX != 0 || lastY != 0 {
		return lastX, lastY
	}
	if current.valid() {
		return float64(current.Width) / 2, float64(current.Height) / 2
	}
	return 0, 0
}

func computePlaybackDelayMs(rawDelay int64, variation VariationConfig, rng *rand.Rand) int64 {
	if rawDelay <= 0 {
		return 0
	}
	delay := applySpeedVariation(float64(rawDelay), variation, rng)
	delay += boundedTimingJitterMs(variation, rng)
	if delay <= 0 {
		return 0
	}
	return int64(math.Round(delay))
}

func applySpeedVariation(delay float64, variation VariationConfig, rng *rand.Rand) float64 {
	amount := clamp01(math.Abs(variation.Intensity) * math.Abs(variation.SpeedVariation))
	if amount <= 0 || rng == nil {
		return delay
	}
	factor := 1 + ((rng.Float64()*2)-1)*amount
	if factor < 0.05 {
		factor = 0.05
	}
	return delay * factor
}

func boundedTimingJitterMs(variation VariationConfig, rng *rand.Rand) float64 {
	maxJitter := math.Abs(variation.Intensity) * math.Abs(variation.TimingJitter)
	if maxJitter <= 0 || rng == nil {
		return 0
	}
	return ((rng.Float64() * 2) - 1) * maxJitter
}

func clamp01(v float64) float64 {
	if v < 0 {
		return 0
	}
	if v > 1 {
		return 1
	}
	return v
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
		time.Sleep(time.Duration(computePlaybackDelayMs(int64(8+e.rng.Intn(8)), VariationConfig{
			Intensity:      e.variation.Intensity,
			SpeedVariation: e.variation.SpeedVariation,
		}, e.rng)) * time.Millisecond)
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
			"type":       eventType,
			"x":          math.Round(x),
			"y":          math.Round(y),
			"button":     btn,
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

func (e *PlaybackEngine) scrollRecordedTarget(msgID int, targetPath string, deltaX, deltaY float64) (bool, error) {
	pathJSON, err := json.Marshal(targetPath)
	if err != nil {
		return false, fmt.Errorf("marshal target path: %w", err)
	}
	expr := fmt.Sprintf(`(function() {
  const el = document.querySelector(%s);
  if (!el || typeof el.scrollBy !== 'function') return false;
  el.scrollBy({ left: %f, top: %f, behavior: 'auto' });
  return true;
})()`, pathJSON, deltaX, deltaY)

	raw, err := e.sendCommand(msgID, "Runtime.evaluate", map[string]interface{}{
		"expression":    expr,
		"returnByValue": true,
	})
	if err != nil {
		return false, err
	}

	var response struct {
		Result struct {
			Value bool `json:"value"`
		} `json:"result"`
		ExceptionDetails json.RawMessage `json:"exceptionDetails,omitempty"`
	}
	if err := json.Unmarshal(raw, &response); err != nil {
		return false, fmt.Errorf("parse scroll target response: %w", err)
	}
	if len(response.ExceptionDetails) > 0 && string(response.ExceptionDetails) != "null" {
		return false, nil
	}
	return response.Result.Value, nil
}

func (e *PlaybackEngine) dispatchScroll(msgID int, x, y, deltaX, deltaY float64) error {
	// Dispatch at the recorded viewport coordinate as a fallback for legacy
	// recordings that do not contain a scroll container path.
	return e.sendMessage(map[string]interface{}{
		"id":     msgID,
		"method": "Input.dispatchMouseEvent",
		"params": map[string]interface{}{
			"type":   "mouseWheel",
			"x":      math.Round(x),
			"y":      math.Round(y),
			"deltaX": math.Round(deltaX),
			"deltaY": math.Round(deltaY),
		},
	})
}

func (e *PlaybackEngine) sendMessage(msg map[string]interface{}) error {
	id, ok := intFromInterface(msg["id"])
	if !ok {
		return fmt.Errorf("cdp message missing numeric id")
	}
	method, ok := msg["method"].(string)
	if !ok || method == "" {
		return fmt.Errorf("cdp message missing method")
	}
	_, err := e.sendCommand(id, method, msg["params"])
	return err
}

func (e *PlaybackEngine) sendCommand(id int, method string, params interface{}) (json.RawMessage, error) {
	if e.wsConn == nil {
		return nil, fmt.Errorf("websocket not connected")
	}
	return sendCDPCommandWS(e.wsConn, id, method, params, 5*time.Second)
}

func intFromInterface(value interface{}) (int, bool) {
	switch v := value.(type) {
	case int:
		return v, true
	case int64:
		return int(v), true
	case float64:
		return int(v), true
	case json.Number:
		i, err := v.Int64()
		if err != nil {
			return 0, false
		}
		return int(i), true
	default:
		return 0, false
	}
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
