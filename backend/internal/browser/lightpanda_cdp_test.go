package browser

import (
	"encoding/json"
	"testing"
)

func TestLightpandaCDPConn_NavigateEmptyURL(t *testing.T) {
	conn := &LightpandaCDPConn{port: 9222}
	err := conn.Navigate("", 5000)
	if err == nil {
		t.Fatal("Navigate with empty URL should return error")
	}
}

func TestLightpandaCDPConn_EvaluateJSEmpty(t *testing.T) {
	conn := &LightpandaCDPConn{port: 9222}
	_, err := conn.EvaluateJS("", 5000)
	if err == nil {
		t.Fatal("EvaluateJS with empty expression should return error")
	}
}

func TestLightpandaCDPConn_ClickElementEmpty(t *testing.T) {
	conn := &LightpandaCDPConn{port: 9222}
	err := conn.ClickElement("", 5000)
	if err == nil {
		t.Fatal("ClickElement with empty selector should return error")
	}
}

func TestLightpandaCDPConn_WaitForSelectorEmpty(t *testing.T) {
	conn := &LightpandaCDPConn{port: 9222}
	err := conn.WaitForSelector("", 5000)
	if err == nil {
		t.Fatal("WaitForSelector with empty selector should return error")
	}
}

func TestLightpandaCDPConn_Port(t *testing.T) {
	conn := &LightpandaCDPConn{port: 9222}
	if conn.Port() != 9222 {
		t.Fatalf("Port = %d, want 9222", conn.Port())
	}
}

func TestLightpandaCDPConn_Close(t *testing.T) {
	conn := &LightpandaCDPConn{port: 9222}
	conn.Close() // should not panic with nil ws
}

func TestLightpandaCDPConn_GetHTML_ErrorOnNoConnection(t *testing.T) {
	conn := &LightpandaCDPConn{port: 9222}
	_, err := conn.GetHTML(1000)
	if err == nil {
		t.Fatal("GetHTML without connection should return error")
	}
}

func TestLightpandaCDPConn_GetText_ErrorOnNoConnection(t *testing.T) {
	conn := &LightpandaCDPConn{port: 9222}
	_, err := conn.GetText(1000)
	if err == nil {
		t.Fatal("GetText without connection should return error")
	}
}

func TestLightpandaCDPConn_GetTitle_ErrorOnNoConnection(t *testing.T) {
	conn := &LightpandaCDPConn{port: 9222}
	_, err := conn.GetTitle(1000)
	if err == nil {
		t.Fatal("GetTitle without connection should return error")
	}
}

func TestLightpandaCDPConn_Screenshot_ErrorOnNoConnection(t *testing.T) {
	conn := &LightpandaCDPConn{port: 9222}
	_, err := conn.Screenshot(1000)
	if err == nil {
		t.Fatal("Screenshot without connection should return error")
	}
}

func TestLightpandaCDPConn_SetViewport_ErrorOnNoConnection(t *testing.T) {
	conn := &LightpandaCDPConn{port: 9222}
	err := conn.SetViewport(1920, 1080, 1000)
	if err == nil {
		t.Fatal("SetViewport without connection should return error")
	}
}

func TestDialLightpandaCDP_InvalidPort(t *testing.T) {
	// Try connecting to a port that's unlikely to have Lightpanda running
	_, err := DialLightpandaCDP(19999)
	if err == nil {
		t.Fatal("DialLightpandaCDP to unused port should return error")
	}
}

func TestIsLightpandaReachable_False(t *testing.T) {
	if IsLightpandaReachable(19999) {
		t.Fatal("IsLightpandaReachable on unused port should return false")
	}
}

func TestNavigateResponseParsing(t *testing.T) {
	// Verify we can parse a typical Page.navigate response
	resp := `{"id":1,"result":{"frameId":"ABC123","loaderId":"LOAD1"}}`
	var r struct {
		ID     int             `json:"id"`
		Result json.RawMessage `json:"result"`
	}
	if err := json.Unmarshal([]byte(resp), &r); err != nil {
		t.Fatalf("Failed to parse navigate response: %v", err)
	}
}

func TestGetHTMLResponseParsing(t *testing.T) {
	resp := `{"id":1,"result":{"value":"<html><body>hello</body></html>"}}`
	type evalResp struct {
		Value string `json:"value"`
	}
	var r struct {
		Result evalResp `json:"result"`
	}
	if err := json.Unmarshal([]byte(resp), &r); err != nil {
		t.Fatalf("Failed to parse getHTML response: %v", err)
	}
	if r.Result.Value != "<html><body>hello</body></html>" {
		t.Fatalf("Value = %q", r.Result.Value)
	}
}

func TestScreenshotResponseParsing(t *testing.T) {
	resp := `{"id":1,"result":{"data":"iVBORw0KGgo..."}}`
	var r struct {
		Result struct {
			Data string `json:"data"`
		} `json:"result"`
	}
	if err := json.Unmarshal([]byte(resp), &r); err != nil {
		t.Fatalf("Failed to parse screenshot response: %v", err)
	}
	if r.Result.Data != "iVBORw0KGgo..." {
		t.Fatalf("Data = %q", r.Result.Data)
	}
}
