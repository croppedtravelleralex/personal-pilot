package behavior

import (
	"encoding/json"
	"fmt"
	"math"
	"math/rand"
	"strings"
	"time"

	"github.com/gorilla/websocket"

	"personal-pilot/backend/internal/behavior/humanize"
)

// CDPExecutor dispatches humanized actions to a browser via CDP WebSocket.
type CDPExecutor struct {
	ws         *websocket.Conn
	middleware *humanize.BehavioralMutationMiddleware
	msgID      int
	currentX   float64
	currentY   float64
	level      humanize.HumanizationLevel
}

// NewCDPExecutor creates a CDP executor connected to the given WebSocket.
func NewCDPExecutor(ws *websocket.Conn, config humanize.HumanizationConfig) *CDPExecutor {
	if ws == nil {
		return nil
	}
	return &CDPExecutor{
		ws:         ws,
		middleware: humanize.NewBehavioralMutationMiddleware(config),
		msgID:      0,
		level:      config.Level,
	}
}

// HumanizationLevel returns the current humanization level.
func (e *CDPExecutor) HumanizationLevel() humanize.HumanizationLevel {
	return e.level
}

// UpdateHumanizationConfig replaces the middleware config and level at runtime.
func (e *CDPExecutor) UpdateHumanizationConfig(config humanize.HumanizationConfig) {
	e.middleware.UpdateConfig(config)
	e.level = config.Level
}

// Close closes the underlying CDP WebSocket connection.
func (e *CDPExecutor) Close() error {
	if e.ws != nil {
		return e.ws.Close()
	}
	return nil
}

// IsConnected returns true if the underlying WebSocket is not nil.
func (e *CDPExecutor) IsConnected() bool {
	return e.ws != nil
}

// Navigate navigates the connected page to the given URL using CDP Page.navigate.
func (e *CDPExecutor) Navigate(url string) error {
	_, err := e.sendCommand("Page.navigate", map[string]interface{}{
		"url": url,
	})
	return err
}

// MoveMouseTo moves the mouse cursor to the given viewport coordinates using a
// humanized Bezier trajectory.
func (e *CDPExecutor) MoveMouseTo(x, y float64) error {
	return e.mouseMove(e.currentX, e.currentY, x, y)
}

// Click clicks the left mouse button at the current mouse position.
func (e *CDPExecutor) Click() error {
	if err := e.dispatchMouseEvent("mousePressed", e.currentX, e.currentY, "left", 1); err != nil {
		return err
	}
	if err := e.dispatchMouseEvent("mouseReleased", e.currentX, e.currentY, "left", 1); err != nil {
		return err
	}
	return nil
}

// MoveMouseRandom moves the mouse to a random position within the viewport.
func (e *CDPExecutor) MoveMouseRandom() error {
	targetX := float64(rand.Intn(800) + 100)
	targetY := float64(rand.Intn(500) + 50)
	return e.mouseMove(e.currentX, e.currentY, targetX, targetY)
}

// SimulateNaturalBrowsing performs human-like idle browsing.
func (e *CDPExecutor) SimulateNaturalBrowsing(duration time.Duration) error {
	deadline := time.Now().Add(duration)
	scrollDist := uint32(0)
	for time.Now().Before(deadline) {
		action := rand.Intn(10)
		switch {
		case action < 3:
			_ = e.MoveMouseRandom()
			time.Sleep(time.Duration(200+rand.Intn(800)) * time.Millisecond)
		case action < 5:
			scrollDist = uint32(100 + rand.Intn(300))
			_ = e.ExecuteHumanizedScroll(scrollDist)
			time.Sleep(time.Duration(500+rand.Intn(1500)) * time.Millisecond)
		case action < 7:
			_ = e.ExecuteHumanizedScroll(uint32(40 + rand.Intn(80)))
			time.Sleep(time.Duration(300+rand.Intn(700)) * time.Millisecond)
		case action < 9:
			time.Sleep(time.Duration(800+rand.Intn(2000)) * time.Millisecond)
		default:
			_ = e.MoveMouseRandom()
			time.Sleep(time.Duration(400+rand.Intn(1200)) * time.Millisecond)
		}
	}
	return nil
}

// ─── Wait ─────────────────────────────────────────────────────────────────────

// Wait pauses for the given duration.
func (e *CDPExecutor) Wait(duration time.Duration) error {
	if duration > 0 {
		time.Sleep(duration)
	}
	return nil
}

// WaitForSelector polls the page until the CSS selector exists, or returns an
// error after the timeout. Polls every 200ms.
func (e *CDPExecutor) WaitForSelector(selector string, timeout time.Duration) error {
	deadline := time.Now().Add(timeout)
	js := fmt.Sprintf(`document.querySelector(%q) !== null`, selector)
	for time.Now().Before(deadline) {
		val, err := e.EvaluateJS(js)
		if err == nil && strings.TrimSpace(val) == "true" {
			return nil
		}
		time.Sleep(200 * time.Millisecond)
	}
	return fmt.Errorf("timeout waiting for selector %q after %v", selector, timeout)
}

// WaitForSelectorVisible polls until the element exists AND is visible
// (offsetParent !== null or getClientRects().length > 0).
func (e *CDPExecutor) WaitForSelectorVisible(selector string, timeout time.Duration) error {
	deadline := time.Now().Add(timeout)
	js := fmt.Sprintf(`(function(){
		var el = document.querySelector(%q);
		if (!el) return false;
		var r = el.getBoundingClientRect();
		return r.width > 0 && r.height > 0;
	})()`, selector)
	for time.Now().Before(deadline) {
		val, err := e.EvaluateJS(js)
		if err == nil && strings.TrimSpace(val) == "true" {
			return nil
		}
		time.Sleep(200 * time.Millisecond)
	}
	return fmt.Errorf("timeout waiting for visible selector %q after %v", selector, timeout)
}

// ─── Click variants ───────────────────────────────────────────────────────────

// ExecuteHumanizedClick clicks an element with human-like mouse movement.
func (e *CDPExecutor) ExecuteHumanizedClick(selector string) error {
	bounds, err := e.getElementBounds(selector)
	if err != nil {
		return fmt.Errorf("get bounds %s: %w", selector, err)
	}
	action := humanize.LlmAction{
		Type:     humanize.ActionClick,
		Selector: selector,
	}
	mutated := e.middleware.Mutate(action, bounds)
	return e.ExecuteMutatedAction(mutated)
}

// ClickWithOffset clicks at a specific offset from the element's top-left corner.
func (e *CDPExecutor) ClickWithOffset(selector string, offsetX, offsetY int) error {
	bounds, err := e.getElementBounds(selector)
	if err != nil {
		return fmt.Errorf("get bounds %s: %w", selector, err)
	}
	targetX := float64(bounds.X1) + float64(offsetX)
	targetY := float64(bounds.Y1) + float64(offsetY)
	if err := e.mouseMove(e.currentX, e.currentY, targetX, targetY); err != nil {
		return err
	}
	e.currentX = targetX
	e.currentY = targetY
	if err := e.dispatchMouseEvent("mousePressed", e.currentX, e.currentY, "left", 1); err != nil {
		return err
	}
	if err := e.dispatchMouseEvent("mouseReleased", e.currentX, e.currentY, "left", 1); err != nil {
		return err
	}
	return nil
}

// HoverElement moves the mouse to the element's center without clicking.
func (e *CDPExecutor) HoverElement(selector string) error {
	bounds, err := e.getElementBounds(selector)
	if err != nil {
		return fmt.Errorf("get bounds %s: %w", selector, err)
	}
	targetX := float64(bounds.CenterX())
	targetY := float64(bounds.CenterY())
	if err := e.mouseMove(e.currentX, e.currentY, targetX, targetY); err != nil {
		return err
	}
	e.currentX = targetX
	e.currentY = targetY
	return nil
}

// DoubleClickElement double-clicks an element.
func (e *CDPExecutor) DoubleClickElement(selector string) error {
	bounds, err := e.getElementBounds(selector)
	if err != nil {
		return fmt.Errorf("get bounds %s: %w", selector, err)
	}
	targetX := float64(bounds.CenterX())
	targetY := float64(bounds.CenterY())

	if err := e.mouseMove(e.currentX, e.currentY, targetX, targetY); err != nil {
		return err
	}
	e.currentX = targetX
	e.currentY = targetY

	// Two rapid clicks with clickCount=2 on the second press
	if err := e.dispatchMouseEvent("mousePressed", e.currentX, e.currentY, "left", 1); err != nil {
		return err
	}
	if err := e.dispatchMouseEvent("mouseReleased", e.currentX, e.currentY, "left", 1); err != nil {
		return err
	}
	time.Sleep(50 * time.Millisecond)
	if err := e.dispatchMouseEvent("mousePressed", e.currentX, e.currentY, "left", 2); err != nil {
		return err
	}
	if err := e.dispatchMouseEvent("mouseReleased", e.currentX, e.currentY, "left", 2); err != nil {
		return err
	}
	return nil
}

// RightClickElement right-clicks an element (context menu).
func (e *CDPExecutor) RightClickElement(selector string) error {
	bounds, err := e.getElementBounds(selector)
	if err != nil {
		return fmt.Errorf("get bounds %s: %w", selector, err)
	}
	targetX := float64(bounds.CenterX())
	targetY := float64(bounds.CenterY())

	if err := e.mouseMove(e.currentX, e.currentY, targetX, targetY); err != nil {
		return err
	}
	e.currentX = targetX
	e.currentY = targetY

	if err := e.dispatchMouseEvent("mousePressed", e.currentX, e.currentY, "right", 1); err != nil {
		return err
	}
	if err := e.dispatchMouseEvent("mouseReleased", e.currentX, e.currentY, "right", 1); err != nil {
		return err
	}
	return nil
}

// ─── Type variants ────────────────────────────────────────────────────────────

// ExecuteHumanizedType types text into a CSS selector with human-like behavior.
// clearFirst=true clears the field first, submitOnEnter=true presses Enter after text.
func (e *CDPExecutor) ExecuteHumanizedType(selector, text string) error {
	return e.ExecuteHumanizedTypeEx(selector, text, true, false)
}

// ExecuteHumanizedTypeEx types text with options: clearFirst empties the field,
// submitOnEnter presses Enter after the final character.
func (e *CDPExecutor) ExecuteHumanizedTypeEx(selector, text string, clearFirst, submitOnEnter bool) error {
	if clearFirst {
		js := fmt.Sprintf(`(function() {
			var el = document.querySelector(%q);
			if (!el) return false;
			el.focus();
			el.value = '';
			return true;
		})()`, selector)
		if err := e.evaluateRaw(js); err != nil {
			return fmt.Errorf("focus %s: %w", selector, err)
		}
	} else {
		js := fmt.Sprintf(`(function() {
			var el = document.querySelector(%q);
			if (!el) return false;
			el.focus();
			return true;
		})()`, selector)
		if err := e.evaluateRaw(js); err != nil {
			return fmt.Errorf("focus %s: %w", selector, err)
		}
	}

	action := humanize.LlmAction{
		Type:     humanize.ActionTypeText,
		Text:     text,
		Selector: selector,
	}
	mutated := e.middleware.Mutate(action, nil)
	if err := e.ExecuteMutatedAction(mutated); err != nil {
		return err
	}

	if submitOnEnter {
		if err := e.dispatchKeyEvent("keyDown", "Enter"); err != nil {
			return err
		}
		if err := e.dispatchKeyEvent("keyUp", "Enter"); err != nil {
			return err
		}
	}
	return nil
}

// ─── Scroll ───────────────────────────────────────────────────────────────────

// ExecuteHumanizedScroll scrolls down by the given distance.
func (e *CDPExecutor) ExecuteHumanizedScroll(distancePx uint32) error {
	return e.ExecuteHumanizedScrollDir(distancePx, "down")
}

// ExecuteHumanizedScrollDir scrolls in the given direction ("down", "up", "left", "right").
func (e *CDPExecutor) ExecuteHumanizedScrollDir(distancePx uint32, direction string) error {
	dist := distancePx
	if dist == 0 {
		dist = 300
	}
	var top, left int32
	switch strings.ToLower(strings.TrimSpace(direction)) {
	case "up":
		top = -int32(dist)
	case "left":
		left = -int32(dist)
	case "right":
		left = int32(dist)
	default:
		top = int32(dist)
	}
	js := fmt.Sprintf("window.scrollBy({top: %d, left: %d, behavior: 'smooth'})", top, left)
	if _, err := e.sendCommand("Runtime.evaluate", map[string]interface{}{
		"expression": js,
	}); err != nil {
		return fmt.Errorf("scroll %s: %w", direction, err)
	}

	if rand.Float64() < 0.1 {
		time.Sleep(50 * time.Millisecond)
		adjust := fmt.Sprintf("window.scrollBy({top: %d, left: %d, behavior: 'auto'})", -5-rand.Intn(10), 0)
		e.sendCommand("Runtime.evaluate", map[string]interface{}{"expression": adjust})
	}
	return nil
}

// ─── Page info ────────────────────────────────────────────────────────────────

// GetPageURL returns the current page URL.
func (e *CDPExecutor) GetPageURL() (string, error) {
	return e.EvaluateJS("window.location.href")
}

// GetPageTitle returns the current page title.
func (e *CDPExecutor) GetPageTitle() (string, error) {
	return e.EvaluateJS("document.title")
}

// GetElementBounds returns the bounding rect for the given CSS selector.
func (e *CDPExecutor) GetElementBounds(selector string) (*humanize.ElementBounds, error) {
	return e.getElementBounds(selector)
}

// CaptureScreenshot captures a full-page screenshot and returns a base64 data URL.
func (e *CDPExecutor) CaptureScreenshot() (string, error) {
	raw, err := e.sendCommand("Page.captureScreenshot", map[string]interface{}{
		"format":      "png",
		"fromSurface": true,
	})
	if err != nil {
		return "", fmt.Errorf("captureScreenshot: %w", err)
	}
	var resp struct {
		Result struct {
			Data string `json:"data"`
		} `json:"result"`
	}
	if err := json.Unmarshal(raw, &resp); err != nil {
		return "", fmt.Errorf("parse screenshot response: %w", err)
	}
	return "data:image/png;base64," + resp.Result.Data, nil
}

// GetElementBoundsInFrame finds an element inside an iframe and returns its
// page-level bounding rect (iframe offset + element offset).
func (e *CDPExecutor) GetElementBoundsInFrame(frameSelector, selector string) (*humanize.ElementBounds, error) {
	js := fmt.Sprintf(`(function() {
		var iframe = document.querySelector(%q);
		if (!iframe) return null;
		var iframeRect = iframe.getBoundingClientRect();
		var idoc = iframe.contentDocument || iframe.contentWindow.document;
		if (!idoc) return null;
		var el = idoc.querySelector(%q);
		if (!el) return null;
		var r = el.getBoundingClientRect();
		return {x1: iframeRect.left + r.left, y1: iframeRect.top + r.top,
		        x2: iframeRect.left + r.right, y2: iframeRect.top + r.bottom,
		        width: r.width, height: r.height};
	})()`, frameSelector, selector)

	raw, err := e.sendCommand("Runtime.evaluate", map[string]interface{}{
		"expression":    js,
		"returnByValue": true,
	})
	if err != nil {
		return nil, err
	}
	return parseBoundsResult(raw)
}

// FocusElementInFrame focuses an element inside an iframe (for typing etc.).
func (e *CDPExecutor) FocusElementInFrame(frameSelector, selector string) error {
	js := fmt.Sprintf(`(function() {
		var iframe = document.querySelector(%q);
		if (!iframe) return false;
		var idoc = iframe.contentDocument || iframe.contentWindow.document;
		if (!idoc) return false;
		var el = idoc.querySelector(%q);
		if (!el) return false;
		el.focus();
		return true;
	})()`, frameSelector, selector)
	raw, err := e.sendCommand("Runtime.evaluate", map[string]interface{}{
		"expression":    js,
		"returnByValue": true,
	})
	if err != nil {
		return fmt.Errorf("focus in frame %s: %w", frameSelector, err)
	}
	var result struct {
		Result struct {
			Result struct {
				Value bool `json:"value"`
			} `json:"result"`
		} `json:"result"`
	}
	if err := json.Unmarshal(raw, &result); err != nil {
		return fmt.Errorf("parse focus result: %w", err)
	}
	if !result.Result.Result.Value {
		return fmt.Errorf("element %q not found in iframe %q", selector, frameSelector)
	}
	return nil
}

// ─── Selector helpers ─────────────────────────────────────────────────────────

// ResolveSelector returns a CSS selector from various selector types.
// If byText is set, it generates a CSS selector that matches by text content.
// If byXPath is set, it evaluates the XPath and returns a JS selector.
func (e *CDPExecutor) ResolveSelector(selector, byText, byXPath string) (string, error) {
	if byText != "" {
		return resolveByText(byText), nil
	}
	if byXPath != "" {
		return byXPath, nil // XPath is handled during evaluation
	}
	if selector != "" {
		return selector, nil
	}
	return "", fmt.Errorf("no selector provided")
}

func resolveByText(text string) string {
	escaped := strings.ReplaceAll(text, `\`, `\\`)
	escaped = strings.ReplaceAll(escaped, `"`, `\"`)
	return fmt.Sprintf(`//*[text()=%q]`, escaped)
}

// EvaluateXPath runs an XPath expression and returns the first matching element's
// bounding rect, or an error if not found.
func (e *CDPExecutor) EvaluateXPath(xpath string) (*humanize.ElementBounds, error) {
	js := fmt.Sprintf(`(function() {
		var result = document.evaluate(%q, document, null, XPathResult.FIRST_ORDERED_NODE_TYPE, null);
		var el = result.singleNodeValue;
		if (!el) return null;
		var r = el.getBoundingClientRect();
		return {x1: r.left, y1: r.top, x2: r.right, y2: r.bottom, width: r.width, height: r.height};
	})()`, strings.ReplaceAll(xpath, `\`, `\\`))

	raw, err := e.sendCommand("Runtime.evaluate", map[string]interface{}{
		"expression":    js,
		"returnByValue": true,
	})
	if err != nil {
		return nil, err
	}
	return parseBoundsResult(raw)
}

// EvaluateJS runs JavaScript in the page and returns the result as string.
func (e *CDPExecutor) EvaluateJS(js string) (string, error) {
	raw, err := e.sendCommand("Runtime.evaluate", map[string]interface{}{
		"expression":    js,
		"returnByValue": true,
	})
	if err != nil {
		return "", err
	}
	return extractResultValue(raw), nil
}

// EvaluateRaw runs JavaScript and returns the raw result JSON.
func (e *CDPExecutor) EvaluateRaw(js string) ([]byte, error) {
	return e.sendCommand("Runtime.evaluate", map[string]interface{}{
		"expression":    js,
		"returnByValue": true,
	})
}

// ExecuteMutatedAction dispatches a humanized action to the browser.
func (e *CDPExecutor) ExecuteMutatedAction(action humanize.MutatedAction) error {
	if action.PreGapMs > 0 {
		time.Sleep(time.Duration(action.PreGapMs) * time.Millisecond)
	}

	switch action.Type {
	case humanize.MutatedTypeText:
		if action.TypingPlan == nil {
			return fmt.Errorf("MutatedTypeText without TypingPlan")
		}
		return e.executeTypeText(action.Selector, action.TypingPlan)

	case humanize.MutatedClick:
		if action.ClickTarget == nil {
			return fmt.Errorf("MutatedClick without ClickTarget")
		}
		return e.executeClick(*action.ClickTarget)

	case humanize.MutatedScroll:
		if action.ScrollPlan == nil {
			return fmt.Errorf("MutatedScroll without ScrollPlan")
		}
		return e.executeScroll(action.ScrollPlan)

	case humanize.MutatedWait:
		return e.executeWait(action.JitterMs)

	case humanize.MutatedGoto:
		if _, err := e.sendCommand("Page.navigate", map[string]interface{}{
			"url": action.URL,
		}); err != nil {
			return fmt.Errorf("navigate: %w", err)
		}
		return nil

	case humanize.MutatedExecuteJs:
		return e.evaluateRaw(action.Script)

	case humanize.MutatedScreenshot:
		_, err := e.CaptureScreenshot()
		return err

	case humanize.MutatedGetHtml:
		js := "document.documentElement.outerHTML"
		if action.HTMLSelector != nil && *action.HTMLSelector != "" {
			sel := *action.HTMLSelector
			js = fmt.Sprintf("document.querySelector(%q).outerHTML", sel)
		} else if action.Selector != "" {
			js = fmt.Sprintf("document.querySelector(%q).outerHTML", action.Selector)
		}
		_, err := e.EvaluateJS(js)
		return err

	case humanize.MutatedGetText:
		sel := action.Selector
		if sel == "" {
			sel = "body"
		}
		js := fmt.Sprintf("document.querySelector(%q).textContent", sel)
		_, err := e.EvaluateJS(js)
		return err

	case humanize.MutatedCloseBrowser:
		_, err := e.sendCommand("Browser.close", nil)
		return err

	default:
		return nil
	}
}

// ─── internal dispatchers ─────────────────────────────────────────────────────

func (e *CDPExecutor) executeTypeText(selector string, plan *humanize.TypingPlan) error {
	for _, ev := range plan.Events {
		switch ev.Type {
		case humanize.TypingEventKey:
			key := string(ev.Ch)
			if err := e.dispatchKeyEvent("keyDown", key); err != nil {
				return err
			}
			if err := e.dispatchKeyEvent("char", key); err != nil {
				return err
			}
			if err := e.dispatchKeyEvent("keyUp", key); err != nil {
				return err
			}
			if ev.IntervalMs > 0 {
				time.Sleep(time.Duration(ev.IntervalMs) * time.Millisecond)
			}

		case humanize.TypingEventBackspace:
			if err := e.dispatchKeyEvent("keyDown", "Backspace"); err != nil {
				return err
			}
			if err := e.dispatchKeyEvent("keyUp", "Backspace"); err != nil {
				return err
			}
			if ev.IntervalMs > 0 {
				time.Sleep(time.Duration(ev.IntervalMs) * time.Millisecond)
			}

		case humanize.TypingEventPause:
			if ev.DurationMs > 0 {
				time.Sleep(time.Duration(ev.DurationMs) * time.Millisecond)
			}
		}
	}
	return nil
}

func (e *CDPExecutor) executeClick(target humanize.ClickTarget) error {
	if len(target.Trajectory) >= 2 {
		hoverPt := target.Trajectory[0]
		clickPt := target.Trajectory[1]
		if err := e.mouseMove(e.currentX, e.currentY, float64(hoverPt[0]), float64(hoverPt[1])); err != nil {
			return err
		}
		e.currentX = float64(hoverPt[0])
		e.currentY = float64(hoverPt[1])

		if target.HoverBeforeMs != nil && *target.HoverBeforeMs > 0 {
			time.Sleep(time.Duration(*target.HoverBeforeMs) * time.Millisecond)
		}

		if err := e.mouseMove(e.currentX, e.currentY, float64(clickPt[0]), float64(clickPt[1])); err != nil {
			return err
		}
		e.currentX = float64(clickPt[0])
		e.currentY = float64(clickPt[1])
	} else {
		if err := e.mouseMove(e.currentX, e.currentY, float64(target.X), float64(target.Y)); err != nil {
			return err
		}
		e.currentX = float64(target.X)
		e.currentY = float64(target.Y)
	}

	if err := e.dispatchMouseEvent("mousePressed", e.currentX, e.currentY, "left", 1); err != nil {
		return err
	}
	if err := e.dispatchMouseEvent("mouseReleased", e.currentX, e.currentY, "left", 1); err != nil {
		return err
	}
	return nil
}

func (e *CDPExecutor) executeScroll(plan *humanize.ScrollPlan) error {
	for _, step := range plan.Steps {
		switch step.Type {
		case humanize.ScrollStepBy:
			js := fmt.Sprintf("window.scrollBy({top: %d, left: 0, behavior: 'smooth'})", step.DeltaPx)
			if _, err := e.sendCommand("Runtime.evaluate", map[string]interface{}{
				"expression": js,
			}); err != nil {
				return err
			}
		case humanize.ScrollStepTo:
			js := fmt.Sprintf("window.scrollTo({top: %d, behavior: 'smooth'})", step.Y)
			if _, err := e.sendCommand("Runtime.evaluate", map[string]interface{}{
				"expression": js,
			}); err != nil {
				return err
			}
		case humanize.ScrollStepPause:
			if step.DurationMs > 0 {
				time.Sleep(time.Duration(step.DurationMs) * time.Millisecond)
			}
		}
	}
	return nil
}

func (e *CDPExecutor) executeWait(jitterMs uint32) error {
	if jitterMs > 0 {
		time.Sleep(time.Duration(jitterMs) * time.Millisecond)
	}
	return nil
}

// ─── CDP primitives ────────────────────────────────────────────────────────────

func (e *CDPExecutor) sendCommand(method string, params interface{}) ([]byte, error) {
	e.msgID++
	return sendCDPCommandWS(e.ws, e.msgID, method, params, 15*time.Second)
}

func (e *CDPExecutor) dispatchKeyEvent(typ, key string) error {
	params := map[string]interface{}{
		"type": typ,
	}
	if typ == "char" {
		params["text"] = key
	} else {
		params["key"] = key
	}
	_, err := e.sendCommand("Input.dispatchKeyEvent", params)
	return err
}

func (e *CDPExecutor) dispatchMouseEvent(typ string, x, y float64, button string, clickCount int) error {
	params := map[string]interface{}{
		"type":       typ,
		"x":          x,
		"y":          y,
		"button":     button,
		"clickCount": clickCount,
	}
	_, err := e.sendCommand("Input.dispatchMouseEvent", params)
	return err
}

func (e *CDPExecutor) mouseMove(fromX, fromY, toX, toY float64) error {
	profile := DefaultMouseProfile()
	return e.mouseMoveWithProfile(fromX, fromY, toX, toY, profile)
}

func (e *CDPExecutor) mouseMoveWithProfile(fromX, fromY, toX, toY float64, profile MouseProfile) error {
	dist := math.Sqrt(math.Pow(toX-fromX, 2) + math.Pow(toY-fromY, 2))
	steps := int(math.Max(10, dist/15))
	if steps > 80 {
		steps = 80
	}
	if steps < 5 {
		steps = 5
	}

	for i := 0; i <= steps; i++ {
		t := float64(i) / float64(steps)

		var bx, by float64
		cp1x := fromX + (toX-fromX)*0.3 + randOff(profile.JitterPx)
		cp1y := fromY - 50 + randOff(profile.JitterPx)
		cp2x := fromX + (toX-fromX)*0.7 + randOff(profile.JitterPx)
		cp2y := toY + 50 + randOff(profile.JitterPx)
		u := 1 - t
		bx = u*u*u*fromX + 3*u*u*t*cp1x + 3*u*t*t*cp2x + t*t*t*toX
		by = u*u*u*fromY + 3*u*u*t*cp1y + 3*u*t*t*cp2y + t*t*t*toY

		x := bx + randOff(profile.JitterPx)
		y := by + randOff(profile.JitterPx)

		if err := e.dispatchMouseEvent("mouseMoved", x, y, "none", 0); err != nil {
			return err
		}

		baseDelay := 1000.0 / profile.SpeedMean
		speedFactor := 1.0 + math.Sin(math.Pi*t)*0.5
		delay := time.Duration(baseDelay * speedFactor * float64(time.Millisecond))
		time.Sleep(delay)

		if rand.Float64() < profile.PauseProb {
			pauseMs := rand.Intn(profile.PauseMaxMs + 1)
			if pauseMs > 0 {
				time.Sleep(time.Duration(pauseMs) * time.Millisecond)
			}
		}
	}
	return nil
}

func (e *CDPExecutor) evaluateRaw(js string) error {
	_, err := e.sendCommand("Runtime.evaluate", map[string]interface{}{
		"expression":    js,
		"returnByValue": true,
	})
	return err
}

func (e *CDPExecutor) getElementBounds(selector string) (*humanize.ElementBounds, error) {
	js := fmt.Sprintf(`(function() {
		var el = document.querySelector(%q);
		if (!el) return null;
		var r = el.getBoundingClientRect();
		return {x1: r.left, y1: r.top, x2: r.right, y2: r.bottom,
		        width: r.width, height: r.height};
	})()`, selector)

	raw, err := e.sendCommand("Runtime.evaluate", map[string]interface{}{
		"expression":    js,
		"returnByValue": true,
	})
	if err != nil {
		return nil, err
	}

	return parseBoundsResult(raw)
}

// ─── Package-level helpers ──────────────────────────────────────────────────────

// ─── Frame-aware actions ────────────────────────────────────────────────────────

// ExecuteHumanizedClickInFrame clicks an element inside an iframe.
func (e *CDPExecutor) ExecuteHumanizedClickInFrame(frameSelector, selector string) error {
	bounds, err := e.GetElementBoundsInFrame(frameSelector, selector)
	if err != nil {
		return fmt.Errorf("get bounds in frame %s: %w", frameSelector, err)
	}
	action := humanize.LlmAction{
		Type:     humanize.ActionClick,
		Selector: selector,
	}
	mutated := e.middleware.Mutate(action, bounds)
	return e.ExecuteMutatedAction(mutated)
}

// ExecuteHumanizedTypeInFrame types text into an element inside an iframe.
func (e *CDPExecutor) ExecuteHumanizedTypeInFrame(frameSelector, selector, text string, clearFirst, submitOnEnter bool) error {
	if err := e.FocusElementInFrame(frameSelector, selector); err != nil {
		return fmt.Errorf("focus in frame %s: %w", frameSelector, err)
	}
	if clearFirst {
		js := fmt.Sprintf(`(function() {
			var iframe = document.querySelector(%q);
			if (!iframe) return;
			var idoc = iframe.contentDocument || iframe.contentWindow.document;
			if (!idoc) return;
			var el = idoc.querySelector(%q);
			if (!el) return;
			el.value = '';
		})()`, frameSelector, selector)
		_ = e.evaluateRaw(js)
	}
	action := humanize.LlmAction{
		Type:     humanize.ActionTypeText,
		Text:     text,
		Selector: selector,
	}
	mutated := e.middleware.Mutate(action, nil)
	if err := e.ExecuteMutatedAction(mutated); err != nil {
		return err
	}
	if submitOnEnter {
		if err := e.dispatchKeyEvent("keyDown", "Enter"); err != nil {
			return err
		}
		if err := e.dispatchKeyEvent("keyUp", "Enter"); err != nil {
			return err
		}
	}
	return nil
}

// HoverElementInFrame hovers over an element inside an iframe.
func (e *CDPExecutor) HoverElementInFrame(frameSelector, selector string) error {
	bounds, err := e.GetElementBoundsInFrame(frameSelector, selector)
	if err != nil {
		return fmt.Errorf("get bounds in frame %s: %w", frameSelector, err)
	}
	targetX := float64(bounds.CenterX())
	targetY := float64(bounds.CenterY())
	if err := e.mouseMove(e.currentX, e.currentY, targetX, targetY); err != nil {
		return err
	}
	e.currentX = targetX
	e.currentY = targetY
	return nil
}

// DoubleClickElementInFrame double-clicks an element inside an iframe.
func (e *CDPExecutor) DoubleClickElementInFrame(frameSelector, selector string) error {
	bounds, err := e.GetElementBoundsInFrame(frameSelector, selector)
	if err != nil {
		return fmt.Errorf("get bounds in frame %s: %w", frameSelector, err)
	}
	targetX := float64(bounds.CenterX())
	targetY := float64(bounds.CenterY())
	if err := e.mouseMove(e.currentX, e.currentY, targetX, targetY); err != nil {
		return err
	}
	e.currentX = targetX
	e.currentY = targetY
	if err := e.dispatchMouseEvent("mousePressed", e.currentX, e.currentY, "left", 1); err != nil {
		return err
	}
	if err := e.dispatchMouseEvent("mouseReleased", e.currentX, e.currentY, "left", 1); err != nil {
		return err
	}
	time.Sleep(50 * time.Millisecond)
	if err := e.dispatchMouseEvent("mousePressed", e.currentX, e.currentY, "left", 2); err != nil {
		return err
	}
	if err := e.dispatchMouseEvent("mouseReleased", e.currentX, e.currentY, "left", 2); err != nil {
		return err
	}
	return nil
}

// RightClickElementInFrame right-clicks an element inside an iframe.
func (e *CDPExecutor) RightClickElementInFrame(frameSelector, selector string) error {
	bounds, err := e.GetElementBoundsInFrame(frameSelector, selector)
	if err != nil {
		return fmt.Errorf("get bounds in frame %s: %w", frameSelector, err)
	}
	targetX := float64(bounds.CenterX())
	targetY := float64(bounds.CenterY())
	if err := e.mouseMove(e.currentX, e.currentY, targetX, targetY); err != nil {
		return err
	}
	e.currentX = targetX
	e.currentY = targetY
	if err := e.dispatchMouseEvent("mousePressed", e.currentX, e.currentY, "right", 1); err != nil {
		return err
	}
	if err := e.dispatchMouseEvent("mouseReleased", e.currentX, e.currentY, "right", 1); err != nil {
		return err
	}
	return nil
}

// ClickWithOffsetInFrame clicks at a specific offset from an element inside an iframe.
func (e *CDPExecutor) ClickWithOffsetInFrame(frameSelector, selector string, offsetX, offsetY int) error {
	bounds, err := e.GetElementBoundsInFrame(frameSelector, selector)
	if err != nil {
		return fmt.Errorf("get bounds in frame %s: %w", frameSelector, err)
	}
	targetX := float64(bounds.X1) + float64(offsetX)
	targetY := float64(bounds.Y1) + float64(offsetY)
	if err := e.mouseMove(e.currentX, e.currentY, targetX, targetY); err != nil {
		return err
	}
	e.currentX = targetX
	e.currentY = targetY
	if err := e.dispatchMouseEvent("mousePressed", e.currentX, e.currentY, "left", 1); err != nil {
		return err
	}
	if err := e.dispatchMouseEvent("mouseReleased", e.currentX, e.currentY, "left", 1); err != nil {
		return err
	}
	return nil
}

// WaitForSelectorInFrame polls until the CSS selector exists inside an iframe.
func (e *CDPExecutor) WaitForSelectorInFrame(frameSelector, selector string, timeout time.Duration) error {
	deadline := time.Now().Add(timeout)
	js := fmt.Sprintf(`(function() {
		var iframe = document.querySelector(%q);
		if (!iframe) return false;
		var idoc = iframe.contentDocument || iframe.contentWindow.document;
		if (!idoc) return false;
		return idoc.querySelector(%q) !== null;
	})()`, frameSelector, selector)
	for time.Now().Before(deadline) {
		val, err := e.EvaluateJS(js)
		if err == nil && strings.TrimSpace(val) == "true" {
			return nil
		}
		time.Sleep(200 * time.Millisecond)
	}
	return fmt.Errorf("timeout waiting for selector %q in frame %q after %v", selector, frameSelector, timeout)
}

// WaitForSelectorVisibleInFrame polls until the element is visible inside an iframe.
func (e *CDPExecutor) WaitForSelectorVisibleInFrame(frameSelector, selector string, timeout time.Duration) error {
	deadline := time.Now().Add(timeout)
	js := fmt.Sprintf(`(function() {
		var iframe = document.querySelector(%q);
		if (!iframe) return false;
		var idoc = iframe.contentDocument || iframe.contentWindow.document;
		if (!idoc) return false;
		var el = idoc.querySelector(%q);
		if (!el) return false;
		var r = el.getBoundingClientRect();
		return r.width > 0 && r.height > 0;
	})()`, frameSelector, selector)
	for time.Now().Before(deadline) {
		val, err := e.EvaluateJS(js)
		if err == nil && strings.TrimSpace(val) == "true" {
			return nil
		}
		time.Sleep(200 * time.Millisecond)
	}
	return fmt.Errorf("timeout waiting for visible selector %q in frame %q after %v", selector, frameSelector, timeout)
}

// ─── helpers ─────────────────────────────────────────────────────────────────────

// DefaultMouseProfile returns a balanced mouse profile for human-like movement.
func DefaultMouseProfile() MouseProfile {
	return MouseProfile{
		CurveStyle: "bezier3",
		SpeedMean:  400,
		JitterPx:   3,
		PauseProb:  0.15,
		PauseMaxMs: 200,
	}
}

func extractResultValue(raw []byte) string {
	if value, ok := runtimeEvaluateValue(raw); ok {
		return value
	}
	return ""
}

func runtimeEvaluateValue(raw []byte) (string, bool) {
	valueRaw, ok := runtimeEvaluateValueRaw(raw)
	if !ok {
		return "", false
	}
	var value interface{}
	if err := json.Unmarshal(valueRaw, &value); err != nil {
		return "", false
	}
	switch item := value.(type) {
	case nil:
		return "", true
	case string:
		return item, true
	case bool:
		if item {
			return "true", true
		}
		return "false", true
	default:
		return fmt.Sprint(item), true
	}
}

func runtimeEvaluateValueRaw(raw []byte) (json.RawMessage, bool) {
	var envelope map[string]json.RawMessage
	if err := json.Unmarshal(raw, &envelope); err != nil {
		return nil, false
	}
	if value, ok := envelope["value"]; ok {
		return value, true
	}
	if nested, ok := envelope["result"]; ok {
		var nestedEnvelope map[string]json.RawMessage
		if err := json.Unmarshal(nested, &nestedEnvelope); err == nil {
			if value, ok := nestedEnvelope["value"]; ok {
				return value, true
			}
		}
	}
	return nil, false
}

func parseBoundsResult(raw []byte) (*humanize.ElementBounds, error) {
	var nullCheck struct {
		Result struct {
			Result struct {
				Value interface{} `json:"value"`
			} `json:"result"`
		} `json:"result"`
	}
	if err := json.Unmarshal(raw, &nullCheck); err != nil {
		return nil, fmt.Errorf("parse bounds: %w", err)
	}
	if nullCheck.Result.Result.Value == nil {
		return nil, fmt.Errorf("element not found")
	}

	var resp struct {
		Result struct {
			Result struct {
				Value struct {
					X1     int32  `json:"x1"`
					Y1     int32  `json:"y1"`
					X2     int32  `json:"x2"`
					Y2     int32  `json:"y2"`
					Width  uint32 `json:"width"`
					Height uint32 `json:"height"`
				} `json:"value"`
			} `json:"result"`
		} `json:"result"`
	}
	if err := json.Unmarshal(raw, &resp); err != nil {
		return nil, fmt.Errorf("parse bounds: %w", err)
	}

	v := resp.Result.Result.Value
	return &humanize.ElementBounds{
		X1: v.X1, Y1: v.Y1, X2: v.X2, Y2: v.Y2,
		Width: v.Width, Height: v.Height,
	}, nil
}
