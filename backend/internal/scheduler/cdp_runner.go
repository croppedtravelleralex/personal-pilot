package scheduler

import (
	"encoding/json"
	"fmt"
	"net/http"
	"strings"
	"time"

	"github.com/gorilla/websocket"
	"personal-pilot/backend/internal/logger"
)

// DebugPortResolver resolves a profile ID to a CDP debug port.
type DebugPortResolver func(profileID string) (int, error)

// CDPTaskRunner executes task actions via CDP.
type CDPTaskRunner struct {
	resolveDebugPort DebugPortResolver
	log              *logger.Logger
}

// NewCDPTaskRunner creates a new CDP-backed task runner.
func NewCDPTaskRunner(resolver DebugPortResolver) *CDPTaskRunner {
	return &CDPTaskRunner{
		resolveDebugPort: resolver,
		log:              logger.New("CDPTaskRunner"),
	}
}

// Run executes the task's actions sequentially via CDP.
func (r *CDPTaskRunner) Run(task *TaskDef) error {
	if task.ProfileID == "" {
		return fmt.Errorf("task %s has no profile ID, cannot run CDP actions", task.ID)
	}

	debugPort, err := r.resolveDebugPort(task.ProfileID)
	if err != nil {
		return fmt.Errorf("resolve debug port for profile %s: %w", task.ProfileID, err)
	}

	conn, err := dialCDP(debugPort)
	if err != nil {
		return fmt.Errorf("connect CDP on port %d: %w", debugPort, err)
	}
	defer conn.Close()

	r.log.Info("开始执行任务",
		logger.F("task_id", task.ID),
		logger.F("task_name", task.Name),
		logger.F("actions", len(task.Actions)),
	)

	for i, action := range task.Actions {
		if err := r.executeAction(conn, action); err != nil {
			r.log.Error("动作执行失败",
				logger.F("task_id", task.ID),
				logger.F("action_index", i),
				logger.F("action_type", action.Type),
				logger.F("error", err),
			)
			return fmt.Errorf("action %d (%s): %w", i, action.Type, err)
		}
	}

	r.log.Info("任务执行完成", logger.F("task_id", task.ID))
	return nil
}

func (r *CDPTaskRunner) executeAction(conn *cdpConn, action TaskAction) error {
	timeout := action.Timeout
	if timeout <= 0 {
		timeout = 30000
	}

	switch action.Type {
	case "navigate":
		return r.actionNavigate(conn, action, timeout)
	case "click":
		return r.actionClick(conn, action, timeout)
	case "wait":
		return r.actionWait(conn, action, timeout)
	case "extract":
		_, err := r.actionExtract(conn, action, timeout)
		return err
	case "cdp":
		return r.actionCDP(conn, action, timeout)
	default:
		return fmt.Errorf("unknown action type: %s", action.Type)
	}
}

// ─── CDP Connection ──────────────────────────────────────────────────────────

// cdpConn wraps a CDP WebSocket connection.
type cdpConn struct {
	ws    *websocket.Conn
	msgID int
}

// dialCDP connects to the browser's CDP endpoint via /json/version discovery.
func dialCDP(debugPort int) (*cdpConn, error) {
	httpClient := &http.Client{Timeout: 5 * time.Second}
	resp, err := httpClient.Get(fmt.Sprintf("http://127.0.0.1:%d/json/version", debugPort))
	if err != nil {
		return nil, fmt.Errorf("http get /json/version: %w", err)
	}
	defer resp.Body.Close()

	var ver struct {
		WebSocketDebuggerURL string `json:"webSocketDebuggerUrl"`
	}
	if err := json.NewDecoder(resp.Body).Decode(&ver); err != nil {
		return nil, fmt.Errorf("decode /json/version: %w", err)
	}
	if ver.WebSocketDebuggerURL == "" {
		return nil, fmt.Errorf("no webSocketDebuggerUrl in /json/version")
	}

	dialer := &websocket.Dialer{HandshakeTimeout: 5 * time.Second}
	ws, _, err := dialer.Dial(ver.WebSocketDebuggerURL, nil)
	if err != nil {
		return nil, fmt.Errorf("ws dial: %w", err)
	}
	return &cdpConn{ws: ws}, nil
}

func (c *cdpConn) Close() {
	if c.ws != nil {
		c.ws.Close()
	}
}

// sendCommand sends a CDP command and waits for the response.
func (c *cdpConn) sendCommand(method string, params interface{}, timeoutMs int) (json.RawMessage, error) {
	c.msgID++
	type cdpReq struct {
		ID     int         `json:"id"`
		Method string      `json:"method"`
		Params interface{} `json:"params,omitempty"`
	}
	type cdpResp struct {
		ID     int             `json:"id"`
		Result json.RawMessage `json:"result"`
		Error  *struct {
			Message string `json:"message"`
		} `json:"error"`
	}

	if err := c.ws.WriteJSON(cdpReq{ID: c.msgID, Method: method, Params: params}); err != nil {
		return nil, fmt.Errorf("write %s: %w", method, err)
	}

	if timeoutMs > 0 {
		c.ws.SetReadDeadline(time.Now().Add(time.Duration(timeoutMs) * time.Millisecond))
	} else {
		c.ws.SetReadDeadline(time.Time{})
	}

	var r cdpResp
	if err := c.ws.ReadJSON(&r); err != nil {
		return nil, fmt.Errorf("read %s resp: %w", method, err)
	}
	if r.Error != nil {
		return nil, fmt.Errorf("cdp error %s: %s", method, r.Error.Message)
	}
	return r.Result, nil
}

// ─── Action Handlers ────────────────────────────────────────────────────────

// actionNavigate navigates to a URL via Page.navigate.
func (r *CDPTaskRunner) actionNavigate(conn *cdpConn, action TaskAction, timeout int) error {
	url := strings.TrimSpace(action.Target)
	if url == "" {
		return fmt.Errorf("navigate target URL is empty")
	}

	result, err := conn.sendCommand("Page.navigate", map[string]interface{}{
		"url": url,
	}, timeout)
	if err != nil {
		return fmt.Errorf("page.navigate %s: %w", url, err)
	}

	// Check for navigation error in response
	var navResp struct {
		ErrorText string `json:"errorText"`
	}
	if err := json.Unmarshal(result, &navResp); err == nil && navResp.ErrorText != "" {
		return fmt.Errorf("page.navigate %s failed: %s", url, navResp.ErrorText)
	}

	return nil
}

// actionClick clicks an element by CSS selector via Runtime.evaluate.
func (r *CDPTaskRunner) actionClick(conn *cdpConn, action TaskAction, timeout int) error {
	sel := action.Target
	if sel == "" {
		return fmt.Errorf("click target CSS selector is empty")
	}

	js := fmt.Sprintf(`document.querySelector(%q).click()`, sel)
	result, err := conn.sendCommand("Runtime.evaluate", map[string]interface{}{
		"expression":    js,
		"returnByValue": false,
	}, timeout)
	if err != nil {
		return fmt.Errorf("click %s: %w", sel, err)
	}

	// Check for JS exception
	var evalResp struct {
		ExceptionDetails map[string]interface{} `json:"exceptionDetails"`
	}
	if err := json.Unmarshal(result, &evalResp); err == nil && evalResp.ExceptionDetails != nil {
		return fmt.Errorf("click %s JS error: %v", sel, evalResp.ExceptionDetails)
	}

	return nil
}

// actionWait waits for an element or a duration.
// If Target (CSS selector) is set, waits for element to appear in DOM.
// If Target is empty, simply sleeps for Timeout milliseconds.
func (r *CDPTaskRunner) actionWait(conn *cdpConn, action TaskAction, timeout int) error {
	sel := strings.TrimSpace(action.Target)
	if sel == "" {
		// No selector — just sleep for the timeout duration
		time.Sleep(time.Duration(timeout) * time.Millisecond)
		return nil
	}

	// Poll for element presence via Runtime.evaluate
	pollJS := fmt.Sprintf(`(function(sel, timeout) {
		var start = Date.now();
		return new Promise(function(resolve) {
			var iv = setInterval(function() {
				if (document.querySelector(sel)) {
					clearInterval(iv);
					resolve(true);
				} else if (Date.now() - start > timeout) {
					clearInterval(iv);
					resolve(false);
				}
			}, 100);
		});
	})("%s", %d)`, sel, timeout)

	result, err := conn.sendCommand("Runtime.evaluate", map[string]interface{}{
		"expression":    pollJS,
		"returnByValue": true,
		"awaitPromise":  true,
	}, timeout+5000)
	if err != nil {
		return fmt.Errorf("wait for %s: %w", sel, err)
	}

	value, err := runtimeEvaluateBool(result)
	if err != nil {
		return fmt.Errorf("wait parse result: %w", err)
	}
	if !value {
		return fmt.Errorf("wait for %s timed out after %dms", sel, timeout)
	}

	return nil
}

// actionExtract extracts data from an element via Runtime.evaluate.
// Target is the CSS selector.
// Value specifies what to extract: "text" (default), "html", "href", "attr:<name>".
func (r *CDPTaskRunner) actionExtract(conn *cdpConn, action TaskAction, timeout int) (string, error) {
	sel := action.Target
	if sel == "" {
		return "", fmt.Errorf("extract target CSS selector is empty")
	}

	extractType := action.Value
	if extractType == "" {
		extractType = "text"
	}

	js := fmt.Sprintf(`JSON.stringify((function(sel) {
		var el = document.querySelector(sel);
		if (!el) return {error: "element not found"};
		return {
			text: el.textContent ? el.textContent.trim().substring(0, 5000) : "",
			html: el.innerHTML ? el.innerHTML.substring(0, 5000) : "",
			href: el.href || "",
			outerHTML: el.outerHTML ? el.outerHTML.substring(0, 5000) : "",
			tagName: el.tagName || "",
		};
	})("%s"))`, sel)

	result, err := conn.sendCommand("Runtime.evaluate", map[string]interface{}{
		"expression":    js,
		"returnByValue": true,
	}, timeout)
	if err != nil {
		return "", fmt.Errorf("extract %s: %w", sel, err)
	}

	value, err := runtimeEvaluateString(result)
	if err != nil {
		return "", fmt.Errorf("extract parse result: %w", err)
	}

	return value, nil
}

// actionCDP sends a generic CDP command.
// Target is the CDP method name, Value is JSON-encoded params.
func (r *CDPTaskRunner) actionCDP(conn *cdpConn, action TaskAction, timeout int) error {
	method := action.Target
	if method == "" {
		return fmt.Errorf("cdp action target (method) is empty")
	}

	var params map[string]interface{}
	if action.Value != "" {
		if err := json.Unmarshal([]byte(action.Value), &params); err != nil {
			return fmt.Errorf("cdp %s: parse params: %w", method, err)
		}
	}

	_, err := conn.sendCommand(method, params, timeout)
	if err != nil {
		return fmt.Errorf("cdp %s: %w", method, err)
	}
	return nil
}

func runtimeEvaluateValueRaw(result json.RawMessage) (json.RawMessage, bool) {
	var envelope map[string]json.RawMessage
	if err := json.Unmarshal(result, &envelope); err != nil {
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

func runtimeEvaluateBool(result json.RawMessage) (bool, error) {
	valueRaw, ok := runtimeEvaluateValueRaw(result)
	if !ok {
		return false, fmt.Errorf("missing Runtime.evaluate result.value")
	}
	var value bool
	if err := json.Unmarshal(valueRaw, &value); err != nil {
		return false, err
	}
	return value, nil
}

func runtimeEvaluateString(result json.RawMessage) (string, error) {
	valueRaw, ok := runtimeEvaluateValueRaw(result)
	if !ok {
		return "", fmt.Errorf("missing Runtime.evaluate result.value")
	}
	var value string
	if err := json.Unmarshal(valueRaw, &value); err != nil {
		return "", err
	}
	return value, nil
}
