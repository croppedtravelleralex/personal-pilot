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
	ws              *websocket.Conn
	middleware      *humanize.BehavioralMutationMiddleware
	msgID           int
	currentX        float64
	currentY        float64
	level           humanize.HumanizationLevel
	minimalSession  bool
	lastCommandRTT  time.Duration
	sendCommandHook func(string, interface{}) ([]byte, error)
	sleepFn         func(time.Duration)
	// OSClickAtFallback is an optional headed OS click used after CDP press/release retries fail (docs/49 D1).
	OSClickAtFallback func(x, y float64) error
	// ClickFallback records the last click plane used: "" (cdp success), "os", or "cdp".
	ClickFallback string
}

// MutatedActionExecutionResult carries typed payloads produced by read actions.
// Non-read actions return only ActionType when execution succeeds.
type MutatedActionExecutionResult struct {
	ActionType        humanize.MutatedActionType `json:"actionType"`
	ScreenshotDataURL *string                    `json:"screenshotDataUrl,omitempty"`
	HTML              *string                    `json:"html,omitempty"`
	Text              *string                    `json:"text,omitempty"`
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
		sleepFn:    time.Sleep,
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

// EnableMinimalSession enables Page domain only for workbench automation.
// It deliberately avoids Runtime.enable to reduce long-lived CDP listener exposure.
func (e *CDPExecutor) EnableMinimalSession() error {
	if e == nil || e.ws == nil {
		return fmt.Errorf("cdp executor not connected")
	}
	if e.minimalSession {
		return nil
	}
	if _, err := e.sendCommand("Page.enable", map[string]interface{}{}); err != nil {
		return fmt.Errorf("Page.enable: %w", err)
	}
	e.minimalSession = true
	return nil
}

// UsesMinimalSession reports whether the executor is in CDP-minimal workbench mode.
func (e *CDPExecutor) UsesMinimalSession() bool {
	return e != nil && e.minimalSession
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
	return e.clickAtCurrentWithFallback(1)
}

// clickAtCurrentWithFallback presses/releases at current coords; on CDP failure uses OS fallback when configured.
func (e *CDPExecutor) clickAtCurrentWithFallback(clickCount int) error {
	e.ClickFallback = "cdp"
	if err := e.dispatchMouseEvent("mousePressed", e.currentX, e.currentY, "left", clickCount); err != nil {
		return e.maybeOSClickFallback(err)
	}
	if err := e.dispatchMouseEvent("mouseReleased", e.currentX, e.currentY, "left", clickCount); err != nil {
		return e.maybeOSClickFallback(err)
	}
	return nil
}

func (e *CDPExecutor) maybeOSClickFallback(cdpErr error) error {
	if e == nil || e.OSClickAtFallback == nil {
		return cdpErr
	}
	if err := e.OSClickAtFallback(e.currentX, e.currentY); err != nil {
		return fmt.Errorf("cdp click failed (%v); os fallback failed: %w", cdpErr, err)
	}
	e.ClickFallback = "os"
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
	return e.clickAtCurrentWithFallback(1)
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
	sign := int32(1)
	horizontal := false
	switch strings.ToLower(strings.TrimSpace(direction)) {
	case "up":
		sign = -1
	case "left":
		sign = -1
		horizontal = true
	case "right":
		horizontal = true
	}

	cfg := &humanize.HumanizationConfig{Level: e.level}
	plan := humanize.BuildInertialScrollPlan(dist, 0.82, cfg)
	if len(plan.Steps) == 0 {
		plan = humanize.BuildScrollPlan(dist, cfg)
	}
	remaining := int32(dist)
	for _, step := range plan.Steps {
		switch step.Type {
		case humanize.ScrollStepPause:
			time.Sleep(time.Duration(step.DurationMs) * time.Millisecond)
			continue
		case humanize.ScrollStepBy:
			delta := step.DeltaPx
			if delta == 0 {
				continue
			}
			if remaining > 0 && absInt32CDP(delta) > remaining {
				if delta > 0 {
					delta = remaining
				} else {
					delta = -remaining
				}
			}
			dx, dy := float64(0), float64(0)
			if horizontal {
				dx = float64(delta * sign)
			} else {
				dy = float64(delta * sign)
			}
			if err := e.dispatchMouseWheel(dx, dy); err != nil {
				return err
			}
			remaining -= absInt32CDP(delta)
			time.Sleep(time.Duration(12+rand.Intn(20)) * time.Millisecond)
		}
	}
	if remaining > 0 {
		dx, dy := float64(0), float64(0)
		if horizontal {
			dx = float64(remaining * sign)
		} else {
			dy = float64(remaining * sign)
		}
		_ = e.dispatchMouseWheel(dx, dy)
	}
	return nil
}

func (e *CDPExecutor) dispatchMouseWheel(deltaX, deltaY float64) error {
	x, y := e.currentX, e.currentY
	if x == 0 && y == 0 {
		x, y = 400, 300
	}
	_, err := e.sendCommand("Input.dispatchMouseEvent", map[string]interface{}{
		"type":       "mouseWheel",
		"x":          x,
		"y":          y,
		"deltaX":     deltaX,
		"deltaY":     deltaY,
		"pointerType": "mouse",
	})
	if err != nil {
		return fmt.Errorf("mouseWheel: %w", err)
	}
	return nil
}

func absInt32CDP(v int32) int32 {
	if v < 0 {
		return -v
	}
	return v
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
		Data   string `json:"data"`
		Result struct {
			Data string `json:"data"`
		} `json:"result"`
	}
	if err := json.Unmarshal(raw, &resp); err != nil {
		return "", fmt.Errorf("parse screenshot response: %w", err)
	}
	data := strings.TrimSpace(resp.Data)
	if data == "" {
		data = strings.TrimSpace(resp.Result.Data)
	}
	if data == "" {
		return "", fmt.Errorf("screenshot response is empty")
	}
	return "data:image/png;base64," + data, nil
}

func (e *CDPExecutor) ShowMousePointerOverlay() error {
	return e.evaluateRaw(mousePointerOverlayInstallScript())
}

func (e *CDPExecutor) HideMousePointerOverlay() error {
	return e.evaluateRaw(`(function(){
		if (window.__personalPilotPointerOverlay && typeof window.__personalPilotPointerOverlay.destroy === 'function') {
			window.__personalPilotPointerOverlay.destroy();
		}
		return true;
	})()`)
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

// GetPageHTML returns outerHTML for selector or full document.
func (e *CDPExecutor) GetPageHTML(selector string, htmlSelector *string) (string, error) {
	js := "document.documentElement.outerHTML"
	if htmlSelector != nil && strings.TrimSpace(*htmlSelector) != "" {
		js = fmt.Sprintf("document.querySelector(%q).outerHTML", strings.TrimSpace(*htmlSelector))
	} else if strings.TrimSpace(selector) != "" {
		js = fmt.Sprintf("document.querySelector(%q).outerHTML", strings.TrimSpace(selector))
	}
	return e.EvaluateJSString(js)
}

// GetElementText returns textContent for selector (defaults to body).
func (e *CDPExecutor) GetElementText(selector string) (string, error) {
	sel := strings.TrimSpace(selector)
	if sel == "" {
		sel = "body"
	}
	js := fmt.Sprintf("document.querySelector(%q) ? document.querySelector(%q).textContent : ''", sel, sel)
	return e.EvaluateJSString(js)
}

// EvaluateJSString evaluates JS and returns string result.
func (e *CDPExecutor) EvaluateJSString(expression string) (string, error) {
	raw, err := e.sendCommand("Runtime.evaluate", map[string]interface{}{
		"expression":    expression,
		"returnByValue": true,
	})
	if err != nil {
		return "", err
	}
	value, ok := runtimeEvaluateValue(raw)
	if !ok {
		return "", fmt.Errorf("empty evaluate result")
	}
	return value, nil
}

// CaptureDOMSnapshot returns a compact DOM tree summary for complete capture flows.
func (e *CDPExecutor) CaptureDOMSnapshot() (string, error) {
	js := `(function(){
		function walk(node, depth) {
			if (!node || depth > 4) return '';
			var name = node.nodeName || '';
			if (name === '#text') return '';
			var id = node.id ? ('#' + node.id) : '';
			var cls = node.className && typeof node.className === 'string' ? ('.' + node.className.trim().split(/\s+/).slice(0,2).join('.')) : '';
			var line = '  '.repeat(depth) + name.toLowerCase() + id + cls + '\n';
			var children = node.children || [];
			for (var i = 0; i < children.length && i < 40; i++) line += walk(children[i], depth + 1);
			return line;
		}
		return walk(document.documentElement, 0);
	})()`
	return e.EvaluateJSString(js)
}

// ExecuteMutatedAction preserves the legacy error-only contract.
// Call ExecuteMutatedActionWithResult when the action can produce a payload.
func (e *CDPExecutor) ExecuteMutatedAction(action humanize.MutatedAction) error {
	_, err := e.ExecuteMutatedActionWithResult(action)
	return err
}

// ExecuteMutatedActionWithResult executes a humanized action and returns any
// typed payload produced by screenshot, HTML, or text reads.
func (e *CDPExecutor) ExecuteMutatedActionWithResult(action humanize.MutatedAction) (MutatedActionExecutionResult, error) {
	result := MutatedActionExecutionResult{ActionType: action.Type}
	if action.PreGapMs > 0 {
		time.Sleep(time.Duration(action.PreGapMs) * time.Millisecond)
	}

	switch action.Type {
	case humanize.MutatedTypeText:
		if action.TypingPlan == nil {
			return result, fmt.Errorf("MutatedTypeText without TypingPlan")
		}
		return result, e.executeTypeText(action.Selector, action.TypingPlan)

	case humanize.MutatedClick:
		if action.ClickTarget == nil {
			return result, fmt.Errorf("MutatedClick without ClickTarget")
		}
		return result, e.executeClick(*action.ClickTarget)

	case humanize.MutatedScroll:
		if action.ScrollPlan == nil {
			return result, fmt.Errorf("MutatedScroll without ScrollPlan")
		}
		return result, e.executeScroll(action.ScrollPlan)

	case humanize.MutatedWait:
		return result, e.executeWait(action.JitterMs)

	case humanize.MutatedGoto:
		if _, err := e.sendCommand("Page.navigate", map[string]interface{}{
			"url": action.URL,
		}); err != nil {
			return result, fmt.Errorf("navigate: %w", err)
		}
		return result, nil

	case humanize.MutatedExecuteJs:
		return result, e.evaluateRaw(action.Script)

	case humanize.MutatedScreenshot:
		screenshot, err := e.CaptureScreenshot()
		if err != nil {
			return result, err
		}
		result.ScreenshotDataURL = &screenshot
		return result, nil

	case humanize.MutatedGetHtml:
		html, err := e.GetPageHTML(action.Selector, action.HTMLSelector)
		if err != nil {
			return result, err
		}
		result.HTML = &html
		return result, nil

	case humanize.MutatedGetText:
		text, err := e.GetElementText(action.Selector)
		if err != nil {
			return result, err
		}
		result.Text = &text
		return result, nil

	case humanize.MutatedCloseBrowser:
		_, err := e.sendCommand("Browser.close", nil)
		return result, err

	default:
		return result, fmt.Errorf("unsupported mutated action type %d", action.Type)
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
	targetX := float64(target.X)
	targetY := float64(target.Y)
	if len(target.Trajectory) >= 2 {
		hoverPt := target.Trajectory[0]
		clickPt := target.Trajectory[1]
		if err := e.mouseMove(e.currentX, e.currentY, float64(hoverPt[0]), float64(hoverPt[1])); err != nil {
			return err
		}
		e.currentX = float64(hoverPt[0])
		e.currentY = float64(hoverPt[1])

		if target.HoverBeforeMs != nil && *target.HoverBeforeMs > 0 {
			sleep := e.sleepFn
			if sleep == nil {
				sleep = time.Sleep
			}
			sleep(time.Duration(*target.HoverBeforeMs) * time.Millisecond)
		}

		targetX = float64(clickPt[0])
		targetY = float64(clickPt[1])
	}
	if e.level.IsActive() {
		return e.clickAtCurrentWithFourPhase(targetX, targetY)
	}
	if err := e.mouseMove(e.currentX, e.currentY, targetX, targetY); err != nil {
		return err
	}
	e.currentX = targetX
	e.currentY = targetY
	return e.clickAtCurrentWithFallback(1)
}

// clickAtCurrentWithFourPhase applies Fitts arrival then humanized pre-press / post-press gaps (docs/53 L5-2).
func (e *CDPExecutor) clickAtCurrentWithFourPhase(targetX, targetY float64) error {
	seed := uint64(0x44ab13)
	if e.middleware != nil && e.middleware.Config() != nil {
		seed ^= e.middleware.Config().Click.Seed
	}
	plan := humanize.BuildFourPhaseClickPlan(e.currentX, e.currentY, targetX, targetY, 24, seed)
	if err := e.mouseMove(e.currentX, e.currentY, targetX, targetY); err != nil {
		return err
	}
	e.currentX = targetX
	e.currentY = targetY
	sleep := e.sleepFn
	if sleep == nil {
		sleep = time.Sleep
	}
	for _, phase := range plan.Phases {
		switch phase.Type {
		case humanize.ClickPhasePrePress, humanize.ClickPhasePostPress:
			if phase.DurationMs > 0 {
				d := time.Duration(phase.DurationMs) * time.Millisecond
				if d > 120*time.Millisecond {
					d = 120 * time.Millisecond
				}
				sleep(d)
			}
		}
	}
	return e.clickAtCurrentWithFallback(1)
}

func (e *CDPExecutor) executeScroll(plan *humanize.ScrollPlan) error {
	for _, step := range plan.Steps {
		switch step.Type {
		case humanize.ScrollStepBy:
			if err := e.dispatchMouseWheel(0, float64(step.DeltaPx)); err != nil {
				return err
			}
			time.Sleep(time.Duration(12+rand.Intn(18)) * time.Millisecond)
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
	started := time.Now()
	defer func() { e.lastCommandRTT = time.Since(started) }()
	if e.sendCommandHook != nil {
		return e.sendCommandHook(method, params)
	}
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
	retryDelays := []time.Duration(nil)
	if typ == "mousePressed" || typ == "mouseReleased" {
		retryDelays = []time.Duration{200 * time.Millisecond, 400 * time.Millisecond, 800 * time.Millisecond}
	}
	var lastErr error
	for attempt := 0; attempt <= len(retryDelays); attempt++ {
		if _, err := e.sendCommand("Input.dispatchMouseEvent", params); err == nil {
			return nil
		} else {
			lastErr = err
		}
		if attempt < len(retryDelays) {
			sleep := e.sleepFn
			if sleep == nil {
				sleep = time.Sleep
			}
			sleep(retryDelays[attempt])
		}
	}
	return fmt.Errorf("dispatch mouse event %s failed after %d attempts: %w", typ, len(retryDelays)+1, lastErr)
}

func mouseMoveStepCount(distance float64, level humanize.HumanizationLevel, rtt time.Duration) int {
	maxSteps := 32
	switch level {
	case humanize.LevelNone:
		maxSteps = 8
	case humanize.LevelMinimal:
		maxSteps = 16
	case humanize.LevelMedium:
		maxSteps = 24
	}
	switch {
	case rtt >= 500*time.Millisecond && maxSteps > 12:
		maxSteps = 12
	case rtt >= 250*time.Millisecond && maxSteps > 16:
		maxSteps = 16
	case rtt >= 100*time.Millisecond && maxSteps > 24:
		maxSteps = 24
	}
	steps := int(math.Ceil(distance / 30))
	if steps < 4 {
		steps = 4
	}
	if steps > maxSteps {
		steps = maxSteps
	}
	return steps
}

func (e *CDPExecutor) mouseMove(fromX, fromY, toX, toY float64) error {
	if e != nil && e.level.IsActive() {
		return e.mouseMoveFitts(fromX, fromY, toX, toY)
	}
	profile := DefaultMouseProfile()
	return e.mouseMoveWithProfile(fromX, fromY, toX, toY, profile)
}

func (e *CDPExecutor) mouseMoveFitts(fromX, fromY, toX, toY float64) error {
	seed := uint64(0x51ed2705)
	if e.middleware != nil && e.middleware.Config() != nil {
		seed ^= e.middleware.Config().Click.Seed
	}
	seed ^= uint64(int64(fromX*10)) ^ (uint64(int64(toY*10)) << 17)
	plan := humanize.BuildFittsTrajectory(fromX, fromY, toX, toY, 24, seed)
	maxSteps := mouseMoveStepCount(plan.DistancePx, e.level, e.lastCommandRTT)
	points := plan.Points
	if len(points) == 0 {
		return e.dispatchMouseEvent("mouseMoved", toX, toY, "none", 0)
	}
	// Downsample Fitts points to respect CDP RTT budget.
	stride := 1
	if len(points)-1 > maxSteps && maxSteps > 0 {
		stride = (len(points) - 1 + maxSteps - 1) / maxSteps
		if stride < 1 {
			stride = 1
		}
	}
	var prevMs uint32
	for i := 0; i < len(points); i += stride {
		pt := points[i]
		if err := e.dispatchMouseEvent("mouseMoved", pt.X, pt.Y, "none", 0); err != nil {
			return err
		}
		e.currentX = pt.X
		e.currentY = pt.Y
		if i > 0 {
			delta := time.Duration(pt.Ms-prevMs) * time.Millisecond
			if delta > 40*time.Millisecond {
				delta = 40 * time.Millisecond
			}
			if delta > 0 {
				sleep := e.sleepFn
				if sleep == nil {
					sleep = time.Sleep
				}
				sleep(delta)
			}
		}
		prevMs = pt.Ms
	}
	last := points[len(points)-1]
	if last.X != e.currentX || last.Y != e.currentY {
		if err := e.dispatchMouseEvent("mouseMoved", last.X, last.Y, "none", 0); err != nil {
			return err
		}
		e.currentX = last.X
		e.currentY = last.Y
	}
	return nil
}

func (e *CDPExecutor) mouseMoveWithProfile(fromX, fromY, toX, toY float64, profile MouseProfile) error {
	dist := math.Sqrt(math.Pow(toX-fromX, 2) + math.Pow(toY-fromY, 2))
	steps := mouseMoveStepCount(dist, e.level, e.lastCommandRTT)

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
		e.currentX = x
		e.currentY = y

		baseDelay := 1000.0 / profile.SpeedMean
		speedFactor := 1.0 + math.Sin(math.Pi*t)*0.5
		delay := time.Duration(baseDelay * speedFactor * float64(time.Millisecond))
		sleep := e.sleepFn
		if sleep == nil {
			sleep = time.Sleep
		}
		sleep(delay)

		if rand.Float64() < profile.PauseProb {
			pauseMs := rand.Intn(profile.PauseMaxMs + 1)
			if pauseMs > 0 {
				sleep(time.Duration(pauseMs) * time.Millisecond)
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

// GetElementCenter returns the center of an element in viewport CSS coordinates.
func (e *CDPExecutor) GetElementCenter(selector string) (float64, float64, error) {
	bounds, err := e.getElementBounds(selector)
	if err != nil {
		return 0, 0, err
	}
	return float64(bounds.CenterX()), float64(bounds.CenterY()), nil
}

// GetViewportMetrics returns innerWidth, innerHeight, and devicePixelRatio.
func (e *CDPExecutor) GetViewportMetrics() (int32, int32, float64, error) {
	raw, err := e.EvaluateJS(`JSON.stringify({
		vw: window.innerWidth,
		vh: window.innerHeight,
		dpr: window.devicePixelRatio || 1
	})`)
	if err != nil {
		return 0, 0, 0, fmt.Errorf("viewport metrics: %w", err)
	}
	var m struct {
		Vw  int32   `json:"vw"`
		Vh  int32   `json:"vh"`
		Dpr float64 `json:"dpr"`
	}
	if err := json.Unmarshal([]byte(raw), &m); err != nil {
		return 0, 0, 0, fmt.Errorf("parse viewport metrics: %w", err)
	}
	if m.Dpr < 0.5 {
		m.Dpr = 1.0
	}
	if m.Vw <= 0 || m.Vh <= 0 {
		return 0, 0, 0, fmt.Errorf("invalid viewport metrics: %dx%d", m.Vw, m.Vh)
	}
	return m.Vw, m.Vh, m.Dpr, nil
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

func mousePointerOverlayInstallScript() string {
	return `(function(){
		if (window.__personalPilotPointerOverlay && typeof window.__personalPilotPointerOverlay.destroy === 'function') {
			window.__personalPilotPointerOverlay.destroy();
		}
		var dot = document.createElement('div');
		dot.setAttribute('data-personal-pilot-pointer', 'true');
		dot.style.cssText = [
			'position:fixed',
			'left:0',
			'top:0',
			'width:18px',
			'height:18px',
			'border-radius:999px',
			'border:2px solid rgba(255,255,255,.95)',
			'background:rgba(20,120,255,.80)',
			'box-shadow:0 0 0 2px rgba(20,120,255,.28),0 2px 10px rgba(0,0,0,.22)',
			'transform:translate(-9999px,-9999px)',
			'z-index:2147483647',
			'pointer-events:none',
			'transition:width 80ms ease,height 80ms ease,background 80ms ease',
			'mix-blend-mode:normal'
		].join(';');
		(document.documentElement || document.body).appendChild(dot);
		function move(e) {
			if (!e) return;
			dot.style.transform = 'translate(' + (Number(e.clientX || 0) - 9) + 'px,' + (Number(e.clientY || 0) - 9) + 'px)';
		}
		function down(e) {
			move(e);
			dot.style.width = '14px';
			dot.style.height = '14px';
			dot.style.background = 'rgba(255,90,40,.88)';
		}
		function up(e) {
			move(e);
			dot.style.width = '18px';
			dot.style.height = '18px';
			dot.style.background = 'rgba(20,120,255,.80)';
		}
		window.addEventListener('mousemove', move, true);
		window.addEventListener('pointermove', move, true);
		window.addEventListener('mousedown', down, true);
		window.addEventListener('pointerdown', down, true);
		window.addEventListener('mouseup', up, true);
		window.addEventListener('pointerup', up, true);
		window.__personalPilotPointerOverlay = {
			moveTo: function(x, y, phase) {
				var px = Number(x || 0);
				var py = Number(y || 0);
				dot.style.transform = 'translate(' + (px - 9) + 'px,' + (py - 9) + 'px)';
				if (phase === 'down') {
					dot.style.width = '14px';
					dot.style.height = '14px';
					dot.style.background = 'rgba(255,90,40,.88)';
				} else if (phase === 'up' || phase === 'move') {
					dot.style.width = '18px';
					dot.style.height = '18px';
					dot.style.background = 'rgba(20,120,255,.80)';
				}
			},
			destroy: function() {
				window.removeEventListener('mousemove', move, true);
				window.removeEventListener('pointermove', move, true);
				window.removeEventListener('mousedown', down, true);
				window.removeEventListener('pointerdown', down, true);
				window.removeEventListener('mouseup', up, true);
				window.removeEventListener('pointerup', up, true);
				if (dot && dot.parentNode) dot.parentNode.removeChild(dot);
				delete window.__personalPilotPointerOverlay;
			}
		};
		return true;
	})()`
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
	valueRaw, ok := runtimeEvaluateValueRaw(raw)
	if !ok || string(valueRaw) == "null" {
		return nil, fmt.Errorf("element not found")
	}

	var v struct {
		X1     float64 `json:"x1"`
		Y1     float64 `json:"y1"`
		X2     float64 `json:"x2"`
		Y2     float64 `json:"y2"`
		Width  float64 `json:"width"`
		Height float64 `json:"height"`
	}
	if err := json.Unmarshal(valueRaw, &v); err != nil {
		return nil, fmt.Errorf("parse bounds: %w", err)
	}
	return &humanize.ElementBounds{
		X1: int32(math.Round(v.X1)), Y1: int32(math.Round(v.Y1)),
		X2: int32(math.Round(v.X2)), Y2: int32(math.Round(v.Y2)),
		Width: uint32(math.Round(v.Width)), Height: uint32(math.Round(v.Height)),
	}, nil
}
