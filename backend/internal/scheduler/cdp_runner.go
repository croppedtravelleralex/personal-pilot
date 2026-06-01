package scheduler

import (
	"encoding/json"
	"fmt"
	"net/http"
	"os"
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
	case "select":
		return r.actionSelect(conn, action, timeout)
	case "dialog":
		return r.actionDialog(conn, action, timeout)
	case "download":
		return r.actionDownload(conn, action, timeout)
	case "upload":
		return r.actionUpload(conn, action, timeout)
	case "iframe":
		return r.actionIframe(conn, action, timeout)
	case "tab":
		return r.actionTab(conn, action, timeout)
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

func (r *CDPTaskRunner) actionSelect(conn *cdpConn, action TaskAction, timeout int) error {
	sel := strings.TrimSpace(action.Target)
	if sel == "" {
		return fmt.Errorf("select target CSS selector is empty")
	}
	optionValue, err := actionStringValue(action.Value, "value")
	if err != nil {
		return fmt.Errorf("select %s: %w", sel, err)
	}
	if optionValue == "" {
		return fmt.Errorf("select value is empty")
	}

	js := fmt.Sprintf(`(function(sel, wanted) {
		var el = document.querySelector(sel);
		if (!el) return {ok:false,error:"select element not found"};
		var match = Array.prototype.slice.call(el.options || []).find(function(option) {
			return option.value === wanted || option.text === wanted;
		});
		if (!match) return {ok:false,error:"option not found"};
		el.value = match.value;
		el.dispatchEvent(new Event("input", {bubbles:true}));
		el.dispatchEvent(new Event("change", {bubbles:true}));
		return {ok:true,value:el.value};
	})(%q, %q)`, sel, optionValue)

	result, err := conn.sendCommand("Runtime.evaluate", map[string]interface{}{
		"expression":    js,
		"returnByValue": true,
	}, timeout)
	if err != nil {
		return fmt.Errorf("select %s: %w", sel, err)
	}
	if err := requireRuntimeObjectOK(result); err != nil {
		return fmt.Errorf("select %s: %w", sel, err)
	}
	return nil
}

func (r *CDPTaskRunner) actionDialog(conn *cdpConn, action TaskAction, timeout int) error {
	params := map[string]interface{}{"accept": true}
	switch strings.ToLower(strings.TrimSpace(action.Target)) {
	case "", "accept", "ok", "confirm":
		params["accept"] = true
	case "dismiss", "cancel", "reject":
		params["accept"] = false
	default:
		return fmt.Errorf("dialog target must be accept or dismiss, got %q", action.Target)
	}

	if strings.TrimSpace(action.Value) != "" {
		decoded, isObject, err := actionObjectValue(action.Value)
		if err != nil {
			return fmt.Errorf("dialog params: %w", err)
		}
		if isObject {
			if accept, ok := decoded["accept"].(bool); ok {
				params["accept"] = accept
			}
			if promptText, ok := decoded["promptText"].(string); ok {
				params["promptText"] = promptText
			}
		} else {
			params["promptText"] = action.Value
		}
	}

	if _, err := conn.sendCommand("Page.handleJavaScriptDialog", params, timeout); err != nil {
		return fmt.Errorf("dialog: %w", err)
	}
	return nil
}

func (r *CDPTaskRunner) actionDownload(conn *cdpConn, action TaskAction, timeout int) error {
	params := map[string]interface{}{"behavior": "allow"}
	downloadPath := strings.TrimSpace(action.Target)
	if strings.TrimSpace(action.Value) != "" {
		decoded, isObject, err := actionObjectValue(action.Value)
		if err != nil {
			return fmt.Errorf("download params: %w", err)
		}
		if isObject {
			if behavior, ok := decoded["behavior"].(string); ok && strings.TrimSpace(behavior) != "" {
				params["behavior"] = behavior
			}
			if path, ok := decoded["downloadPath"].(string); ok {
				downloadPath = strings.TrimSpace(path)
			}
			if eventsEnabled, ok := decoded["eventsEnabled"].(bool); ok {
				params["eventsEnabled"] = eventsEnabled
			}
		} else if downloadPath == "" {
			downloadPath = strings.TrimSpace(action.Value)
		}
	}
	if downloadPath == "" {
		return fmt.Errorf("download path is empty")
	}
	params["downloadPath"] = downloadPath
	if _, err := conn.sendCommand("Page.setDownloadBehavior", params, timeout); err != nil {
		return fmt.Errorf("download behavior: %w", err)
	}
	return nil
}

func (r *CDPTaskRunner) actionUpload(conn *cdpConn, action TaskAction, timeout int) error {
	sel := strings.TrimSpace(action.Target)
	if sel == "" {
		return fmt.Errorf("upload target CSS selector is empty")
	}
	files, err := actionFileList(action.Value)
	if err != nil {
		return fmt.Errorf("upload %s: %w", sel, err)
	}
	if len(files) == 0 {
		return fmt.Errorf("upload files are empty")
	}
	for _, file := range files {
		if _, err := os.Stat(file); err != nil {
			return fmt.Errorf("upload file %q: %w", file, err)
		}
	}

	document, err := conn.sendCommand("DOM.getDocument", map[string]interface{}{"depth": 1}, timeout)
	if err != nil {
		return fmt.Errorf("upload get document: %w", err)
	}
	rootID, err := domRootNodeID(document)
	if err != nil {
		return fmt.Errorf("upload get document: %w", err)
	}

	queryResult, err := conn.sendCommand("DOM.querySelector", map[string]interface{}{
		"nodeId":   rootID,
		"selector": sel,
	}, timeout)
	if err != nil {
		return fmt.Errorf("upload query %s: %w", sel, err)
	}
	nodeID, err := domQueryNodeID(queryResult)
	if err != nil {
		return fmt.Errorf("upload query %s: %w", sel, err)
	}

	if _, err := conn.sendCommand("DOM.setFileInputFiles", map[string]interface{}{
		"nodeId": nodeID,
		"files":  files,
	}, timeout); err != nil {
		return fmt.Errorf("upload set files %s: %w", sel, err)
	}
	return nil
}

func (r *CDPTaskRunner) actionIframe(conn *cdpConn, action TaskAction, timeout int) error {
	payload, err := parseIframeAction(action)
	if err != nil {
		return err
	}
	if payload.Timeout > 0 {
		timeout = payload.Timeout
	}

	switch payload.Action {
	case "wait":
		js := fmt.Sprintf(`(function(frameSelector, selector, timeout) {
			var iframe = document.querySelector(frameSelector);
			if (!iframe) return Promise.resolve(false);
			var doc = iframe.contentDocument || (iframe.contentWindow && iframe.contentWindow.document);
			if (!doc) return Promise.resolve(false);
			var start = Date.now();
			return new Promise(function(resolve) {
				var iv = setInterval(function() {
					if (doc.querySelector(selector)) {
						clearInterval(iv);
						resolve(true);
					} else if (Date.now() - start > timeout) {
						clearInterval(iv);
						resolve(false);
					}
				}, 100);
			});
		})(%q, %q, %d)`, payload.FrameSelector, payload.Selector, timeout)
		result, err := conn.sendCommand("Runtime.evaluate", map[string]interface{}{"expression": js, "returnByValue": true, "awaitPromise": true}, timeout+5000)
		if err != nil {
			return fmt.Errorf("iframe wait %s %s: %w", payload.FrameSelector, payload.Selector, err)
		}
		ok, err := runtimeEvaluateBool(result)
		if err != nil {
			return fmt.Errorf("iframe wait parse result: %w", err)
		}
		if !ok {
			return fmt.Errorf("iframe wait for %s in %s timed out after %dms", payload.Selector, payload.FrameSelector, timeout)
		}
		return nil
	case "click":
		js := fmt.Sprintf(`(function(frameSelector, selector) {
			var iframe = document.querySelector(frameSelector);
			var doc = iframe && (iframe.contentDocument || (iframe.contentWindow && iframe.contentWindow.document));
			var el = doc && doc.querySelector(selector);
			if (!el) return {ok:false,error:"iframe element not found"};
			el.click();
			return {ok:true};
		})(%q, %q)`, payload.FrameSelector, payload.Selector)
		result, err := conn.sendCommand("Runtime.evaluate", map[string]interface{}{"expression": js, "returnByValue": true}, timeout)
		if err != nil {
			return fmt.Errorf("iframe click %s %s: %w", payload.FrameSelector, payload.Selector, err)
		}
		return requireRuntimeObjectOK(result)
	case "type":
		text := payload.Text
		if text == "" {
			text = payload.Value
		}
		js := fmt.Sprintf(`(function(frameSelector, selector, text) {
			var iframe = document.querySelector(frameSelector);
			var doc = iframe && (iframe.contentDocument || (iframe.contentWindow && iframe.contentWindow.document));
			var el = doc && doc.querySelector(selector);
			if (!el) return {ok:false,error:"iframe element not found"};
			el.focus();
			if ("value" in el) el.value = text;
			else el.textContent = text;
			el.dispatchEvent(new Event("input", {bubbles:true}));
			el.dispatchEvent(new Event("change", {bubbles:true}));
			return {ok:true};
		})(%q, %q, %q)`, payload.FrameSelector, payload.Selector, text)
		result, err := conn.sendCommand("Runtime.evaluate", map[string]interface{}{"expression": js, "returnByValue": true}, timeout)
		if err != nil {
			return fmt.Errorf("iframe type %s %s: %w", payload.FrameSelector, payload.Selector, err)
		}
		return requireRuntimeObjectOK(result)
	case "extract":
		js := fmt.Sprintf(`(function(frameSelector, selector) {
			var iframe = document.querySelector(frameSelector);
			var doc = iframe && (iframe.contentDocument || (iframe.contentWindow && iframe.contentWindow.document));
			var el = doc && doc.querySelector(selector);
			if (!el) return "";
			return el.textContent || el.value || el.outerHTML || "";
		})(%q, %q)`, payload.FrameSelector, payload.Selector)
		_, err := conn.sendCommand("Runtime.evaluate", map[string]interface{}{"expression": js, "returnByValue": true}, timeout)
		if err != nil {
			return fmt.Errorf("iframe extract %s %s: %w", payload.FrameSelector, payload.Selector, err)
		}
		return nil
	default:
		return fmt.Errorf("iframe action must be wait, click, type, or extract, got %q", payload.Action)
	}
}

func (r *CDPTaskRunner) actionTab(conn *cdpConn, action TaskAction, timeout int) error {
	payload, err := parseTabAction(action)
	if err != nil {
		return err
	}
	switch payload.Action {
	case "new", "create":
		url := payload.URL
		if url == "" {
			url = "about:blank"
		}
		_, err := conn.sendCommand("Target.createTarget", map[string]interface{}{"url": url}, timeout)
		if err != nil {
			return fmt.Errorf("tab create: %w", err)
		}
		return nil
	case "activate", "switch":
		if payload.TargetID == "" {
			return fmt.Errorf("tab targetId is empty")
		}
		_, err := conn.sendCommand("Target.activateTarget", map[string]interface{}{"targetId": payload.TargetID}, timeout)
		if err != nil {
			return fmt.Errorf("tab activate %s: %w", payload.TargetID, err)
		}
		return nil
	case "close":
		if payload.TargetID == "" {
			return fmt.Errorf("tab targetId is empty")
		}
		_, err := conn.sendCommand("Target.closeTarget", map[string]interface{}{"targetId": payload.TargetID}, timeout)
		if err != nil {
			return fmt.Errorf("tab close %s: %w", payload.TargetID, err)
		}
		return nil
	case "list":
		_, err := conn.sendCommand("Target.getTargets", nil, timeout)
		if err != nil {
			return fmt.Errorf("tab list: %w", err)
		}
		return nil
	default:
		return fmt.Errorf("tab action must be new, activate, close, or list, got %q", payload.Action)
	}
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

func requireRuntimeObjectOK(result json.RawMessage) error {
	var envelope struct {
		ExceptionDetails json.RawMessage `json:"exceptionDetails"`
	}
	if err := json.Unmarshal(result, &envelope); err == nil && len(envelope.ExceptionDetails) > 0 && string(envelope.ExceptionDetails) != "null" {
		return fmt.Errorf("Runtime.evaluate exception: %s", string(envelope.ExceptionDetails))
	}

	valueRaw, ok := runtimeEvaluateValueRaw(result)
	if !ok {
		return fmt.Errorf("missing Runtime.evaluate result.value")
	}

	var boolValue bool
	if err := json.Unmarshal(valueRaw, &boolValue); err == nil {
		if boolValue {
			return nil
		}
		return fmt.Errorf("runtime action returned false")
	}

	var objectValue map[string]json.RawMessage
	if err := json.Unmarshal(valueRaw, &objectValue); err != nil {
		return fmt.Errorf("parse Runtime.evaluate value: %w", err)
	}
	if okRaw, hasOK := objectValue["ok"]; hasOK {
		var okValue bool
		if err := json.Unmarshal(okRaw, &okValue); err != nil {
			return fmt.Errorf("parse Runtime.evaluate ok: %w", err)
		}
		if okValue {
			return nil
		}
	}

	if errRaw, hasErr := objectValue["error"]; hasErr {
		var msg string
		if err := json.Unmarshal(errRaw, &msg); err == nil && msg != "" {
			return fmt.Errorf("%s", msg)
		}
	}
	return fmt.Errorf("runtime action did not return ok=true")
}

func actionStringValue(value string, keys ...string) (string, error) {
	trimmed := strings.TrimSpace(value)
	if trimmed == "" {
		return "", nil
	}
	if strings.HasPrefix(trimmed, "{") {
		decoded := map[string]interface{}{}
		if err := json.Unmarshal([]byte(trimmed), &decoded); err != nil {
			return "", err
		}
		for _, key := range keys {
			if value, ok := decoded[key].(string); ok {
				return strings.TrimSpace(value), nil
			}
		}
		if value, ok := decoded["text"].(string); ok {
			return strings.TrimSpace(value), nil
		}
		return "", nil
	}
	if strings.HasPrefix(trimmed, `"`) {
		var decoded string
		if err := json.Unmarshal([]byte(trimmed), &decoded); err != nil {
			return "", err
		}
		return strings.TrimSpace(decoded), nil
	}
	return trimmed, nil
}

func actionObjectValue(value string) (map[string]interface{}, bool, error) {
	trimmed := strings.TrimSpace(value)
	if trimmed == "" {
		return nil, false, nil
	}
	if !strings.HasPrefix(trimmed, "{") {
		return nil, false, nil
	}
	decoded := map[string]interface{}{}
	if err := json.Unmarshal([]byte(trimmed), &decoded); err != nil {
		return nil, true, err
	}
	return decoded, true, nil
}

func actionFileList(value string) ([]string, error) {
	trimmed := strings.TrimSpace(value)
	if trimmed == "" {
		return nil, nil
	}

	if strings.HasPrefix(trimmed, "[") {
		var decoded []string
		if err := json.Unmarshal([]byte(trimmed), &decoded); err != nil {
			return nil, err
		}
		return compactStrings(decoded), nil
	}
	if strings.HasPrefix(trimmed, "{") {
		decoded := map[string]interface{}{}
		if err := json.Unmarshal([]byte(trimmed), &decoded); err != nil {
			return nil, err
		}
		for _, key := range []string{"files", "paths"} {
			if raw, ok := decoded[key].([]interface{}); ok {
				out := make([]string, 0, len(raw))
				for _, item := range raw {
					if s, ok := item.(string); ok {
						out = append(out, s)
					}
				}
				return compactStrings(out), nil
			}
		}
		for _, key := range []string{"file", "path"} {
			if path, ok := decoded[key].(string); ok {
				return compactStrings([]string{path}), nil
			}
		}
		return nil, nil
	}
	if strings.HasPrefix(trimmed, `"`) {
		var decoded string
		if err := json.Unmarshal([]byte(trimmed), &decoded); err != nil {
			return nil, err
		}
		return compactStrings([]string{decoded}), nil
	}

	parts := strings.FieldsFunc(trimmed, func(r rune) bool {
		return r == '\n' || r == '\r' || r == ','
	})
	return compactStrings(parts), nil
}

func compactStrings(values []string) []string {
	out := make([]string, 0, len(values))
	for _, value := range values {
		value = strings.TrimSpace(value)
		if value != "" {
			out = append(out, value)
		}
	}
	return out
}

func domRootNodeID(result json.RawMessage) (int, error) {
	var resp struct {
		Root struct {
			NodeID int `json:"nodeId"`
		} `json:"root"`
	}
	if err := json.Unmarshal(result, &resp); err != nil {
		return 0, err
	}
	if resp.Root.NodeID <= 0 {
		return 0, fmt.Errorf("missing DOM root nodeId")
	}
	return resp.Root.NodeID, nil
}

func domQueryNodeID(result json.RawMessage) (int, error) {
	var resp struct {
		NodeID int `json:"nodeId"`
	}
	if err := json.Unmarshal(result, &resp); err != nil {
		return 0, err
	}
	if resp.NodeID <= 0 {
		return 0, fmt.Errorf("target node not found")
	}
	return resp.NodeID, nil
}

type iframeActionPayload struct {
	Action        string `json:"action"`
	FrameSelector string `json:"frameSelector"`
	Frame         string `json:"frame"`
	Iframe        string `json:"iframe"`
	Selector      string `json:"selector"`
	Target        string `json:"target"`
	Text          string `json:"text"`
	Value         string `json:"value"`
	Timeout       int    `json:"timeout"`
}

func parseIframeAction(action TaskAction) (iframeActionPayload, error) {
	payload := iframeActionPayload{Action: "click"}
	target := strings.TrimSpace(action.Target)
	if isIframeVerb(target) {
		payload.Action = strings.ToLower(target)
	} else {
		payload.FrameSelector = target
	}

	value := strings.TrimSpace(action.Value)
	if value != "" {
		if strings.HasPrefix(value, "{") {
			if err := json.Unmarshal([]byte(value), &payload); err != nil {
				return payload, fmt.Errorf("iframe params: %w", err)
			}
			if payload.Action == "" {
				if isIframeVerb(target) {
					payload.Action = strings.ToLower(target)
				} else {
					payload.Action = "click"
				}
			}
		} else if payload.Selector == "" {
			payload.Selector = value
		}
	}

	if payload.FrameSelector == "" {
		payload.FrameSelector = firstNonEmpty(payload.Frame, payload.Iframe)
	}
	if payload.Selector == "" {
		payload.Selector = payload.Target
	}
	payload.Action = strings.ToLower(strings.TrimSpace(payload.Action))
	if !isIframeVerb(payload.Action) {
		return payload, fmt.Errorf("iframe action must be wait, click, type, or extract, got %q", payload.Action)
	}
	if strings.TrimSpace(payload.FrameSelector) == "" {
		return payload, fmt.Errorf("iframe frameSelector is empty")
	}
	if strings.TrimSpace(payload.Selector) == "" {
		return payload, fmt.Errorf("iframe selector is empty")
	}
	return payload, nil
}

func isIframeVerb(value string) bool {
	switch strings.ToLower(strings.TrimSpace(value)) {
	case "wait", "click", "type", "extract":
		return true
	default:
		return false
	}
}

type tabActionPayload struct {
	Action   string `json:"action"`
	Type     string `json:"type"`
	URL      string `json:"url"`
	TargetID string `json:"targetId"`
	ID       string `json:"id"`
}

func parseTabAction(action TaskAction) (tabActionPayload, error) {
	payload := tabActionPayload{Action: "new"}
	target := strings.TrimSpace(action.Target)
	if isTabVerb(target) {
		payload.Action = strings.ToLower(target)
	} else if target != "" {
		payload.URL = target
	}

	value := strings.TrimSpace(action.Value)
	if value != "" {
		if strings.HasPrefix(value, "{") {
			if err := json.Unmarshal([]byte(value), &payload); err != nil {
				return payload, fmt.Errorf("tab params: %w", err)
			}
			if payload.Action == "" {
				payload.Action = payload.Type
			}
			if payload.Action == "" {
				if isTabVerb(target) {
					payload.Action = strings.ToLower(target)
				} else {
					payload.Action = "new"
				}
			}
		} else {
			switch payload.Action {
			case "new", "create":
				payload.URL = value
			default:
				payload.TargetID = value
			}
		}
	}

	if payload.TargetID == "" {
		payload.TargetID = payload.ID
	}
	payload.Action = strings.ToLower(strings.TrimSpace(payload.Action))
	if !isTabVerb(payload.Action) {
		return payload, fmt.Errorf("tab action must be new, activate, close, or list, got %q", payload.Action)
	}
	return payload, nil
}

func isTabVerb(value string) bool {
	switch strings.ToLower(strings.TrimSpace(value)) {
	case "new", "create", "activate", "switch", "close", "list":
		return true
	default:
		return false
	}
}

func firstNonEmpty(values ...string) string {
	for _, value := range values {
		if strings.TrimSpace(value) != "" {
			return strings.TrimSpace(value)
		}
	}
	return ""
}
