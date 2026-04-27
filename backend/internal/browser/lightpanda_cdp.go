package browser

import (
	"encoding/json"
	"fmt"
	"net/http"
	"time"

	"github.com/gorilla/websocket"
)

// LightpandaCDPConn wraps a CDP WebSocket connection to a Lightpanda browser.
// Lightpanda exposes CDP directly at ws://127.0.0.1:<port>/ without the
// /json/version discovery endpoint that Chromium uses.
type LightpandaCDPConn struct {
	ws    *websocket.Conn
	msgID int
	port  int
}

var lightpandaWSDialer = &websocket.Dialer{HandshakeTimeout: 5 * time.Second}

// DialLightpandaCDP connects to a Lightpanda browser's CDP WebSocket.
func DialLightpandaCDP(port int) (*LightpandaCDPConn, error) {
	url := fmt.Sprintf("ws://127.0.0.1:%d/", port)

	ws, _, err := lightpandaWSDialer.Dial(url, nil)
	if err != nil {
		return nil, fmt.Errorf("lightpanda CDP dial ws://127.0.0.1:%d: %w", port, err)
	}

	return &LightpandaCDPConn{ws: ws, port: port}, nil
}

// Close closes the CDP WebSocket connection.
func (c *LightpandaCDPConn) Close() {
	if c.ws != nil {
		c.ws.Close()
	}
}

// Port returns the debug port this connection is using.
func (c *LightpandaCDPConn) Port() int {
	return c.port
}

// sendCommand sends a CDP command and returns the raw result.
func (c *LightpandaCDPConn) sendCommand(method string, params interface{}, timeoutMs int) (json.RawMessage, error) {
	if c.ws == nil {
		return nil, fmt.Errorf("not connected")
	}
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

// Navigate tells Lightpanda to navigate to a URL via Page.navigate.
func (c *LightpandaCDPConn) Navigate(url string, timeoutMs int) error {
	if url == "" {
		return fmt.Errorf("navigate URL is empty")
	}
	_, err := c.sendCommand("Page.navigate", map[string]interface{}{
		"url": url,
	}, timeoutMs)
	return err
}

// EvaluateJS executes a JavaScript expression in the page context.
func (c *LightpandaCDPConn) EvaluateJS(expression string, timeoutMs int) (json.RawMessage, error) {
	if expression == "" {
		return nil, fmt.Errorf("evaluate expression is empty")
	}
	return c.sendCommand("Runtime.evaluate", map[string]interface{}{
		"expression":    expression,
		"returnByValue": true,
	}, timeoutMs)
}

// GetHTML returns the full page HTML via Runtime.evaluate.
func (c *LightpandaCDPConn) GetHTML(timeoutMs int) (string, error) {
	result, err := c.EvaluateJS("document.documentElement.outerHTML", timeoutMs)
	if err != nil {
		return "", fmt.Errorf("getHTML: %w", err)
	}
	var resp struct {
		Value string `json:"value"`
	}
	if err := json.Unmarshal(result, &resp); err != nil {
		return "", fmt.Errorf("getHTML parse: %w", err)
	}
	return resp.Value, nil
}

// GetText returns the page text content.
func (c *LightpandaCDPConn) GetText(timeoutMs int) (string, error) {
	result, err := c.EvaluateJS("document.body ? document.body.innerText : ''", timeoutMs)
	if err != nil {
		return "", fmt.Errorf("getText: %w", err)
	}
	var resp struct {
		Value string `json:"value"`
	}
	if err := json.Unmarshal(result, &resp); err != nil {
		return "", fmt.Errorf("getText parse: %w", err)
	}
	return resp.Value, nil
}

// GetTitle returns the page title.
func (c *LightpandaCDPConn) GetTitle(timeoutMs int) (string, error) {
	result, err := c.EvaluateJS("document.title", timeoutMs)
	if err != nil {
		return "", fmt.Errorf("getTitle: %w", err)
	}
	var resp struct {
		Value string `json:"value"`
	}
	if err := json.Unmarshal(result, &resp); err != nil {
		return "", fmt.Errorf("getTitle parse: %w", err)
	}
	return resp.Value, nil
}

// ClickElement clicks an element by CSS selector.
func (c *LightpandaCDPConn) ClickElement(selector string, timeoutMs int) error {
	if selector == "" {
		return fmt.Errorf("click selector is empty")
	}
	_, err := c.EvaluateJS(
		fmt.Sprintf(`(function(){var el=document.querySelector(%q);if(el)el.click();else throw new Error('element not found: '+%q)})()`, selector, selector),
		timeoutMs,
	)
	return err
}

// SetViewport sets the visible viewport size.
func (c *LightpandaCDPConn) SetViewport(width, height int, timeoutMs int) error {
	_, err := c.sendCommand("Emulation.setDeviceMetricsOverride", map[string]interface{}{
		"width":             width,
		"height":            height,
		"deviceScaleFactor": 1,
		"mobile":            false,
	}, timeoutMs)
	return err
}

// WaitForSelector polls until a CSS selector matches an element or times out.
func (c *LightpandaCDPConn) WaitForSelector(selector string, timeoutMs int) error {
	if selector == "" {
		return fmt.Errorf("waitForSelector selector is empty")
	}
	pollJS := fmt.Sprintf(`(function(){return new Promise(function(resolve){var start=Date.now();var iv=setInterval(function(){if(document.querySelector(%q)){clearInterval(iv);resolve(true)}else if(Date.now()-start>%d){clearInterval(iv);resolve(false)}},100)})})()`, selector, timeoutMs)

	result, err := c.sendCommand("Runtime.evaluate", map[string]interface{}{
		"expression":    pollJS,
		"returnByValue": true,
		"awaitPromise":  true,
	}, timeoutMs+5000)
	if err != nil {
		return fmt.Errorf("waitForSelector %s: %w", selector, err)
	}
	var resp struct {
		Value bool `json:"value"`
	}
	if err := json.Unmarshal(result, &resp); err != nil {
		return fmt.Errorf("waitForSelector parse: %w", err)
	}
	if !resp.Value {
		return fmt.Errorf("waitForSelector %s timed out after %dms", selector, timeoutMs)
	}
	return nil
}

// Screenshot captures a page screenshot via Page.captureScreenshot.
// Returns base64-encoded image data.
func (c *LightpandaCDPConn) Screenshot(timeoutMs int) (string, error) {
	result, err := c.sendCommand("Page.captureScreenshot", map[string]interface{}{
		"format": "png",
	}, timeoutMs)
	if err != nil {
		return "", fmt.Errorf("screenshot: %w", err)
	}
	var resp struct {
		Data string `json:"data"`
	}
	if err := json.Unmarshal(result, &resp); err != nil {
		return "", fmt.Errorf("screenshot parse: %w", err)
	}
	return resp.Data, nil
}

// IsLightpandaReachable checks if a Lightpanda instance is accepting CDP connections.
func IsLightpandaReachable(port int) bool {
	httpClient := &http.Client{Timeout: 2 * time.Second}
	resp, err := httpClient.Get(fmt.Sprintf("http://127.0.0.1:%d/", port))
	if err != nil {
		return false
	}
	resp.Body.Close()
	return true
}
