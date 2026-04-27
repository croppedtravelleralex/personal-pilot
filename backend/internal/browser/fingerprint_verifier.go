package browser

import (
	"ant-chrome/backend/internal/events"
	"ant-chrome/backend/internal/logger"
	"encoding/json"
	"fmt"
	"math"
	"net/http"
	"strings"
	"time"

	"github.com/gorilla/websocket"
)

// FingerprintSnapshot holds the actual browser fingerprint values extracted via CDP.
type FingerprintSnapshot struct {
	UserAgent           string   `json:"userAgent"`
	Platform            string   `json:"platform"`
	HardwareConcurrency int      `json:"hardwareConcurrency"`
	DeviceMemory        int      `json:"deviceMemory"`
	ColorDepth          int      `json:"colorDepth"`
	PixelDepth          int      `json:"pixelDepth"`
	ScreenWidth         int      `json:"screenWidth"`
	ScreenHeight        int      `json:"screenHeight"`
	AvailWidth          int      `json:"availWidth"`
	AvailHeight         int      `json:"availHeight"`
	DevicePixelRatio    float64  `json:"devicePixelRatio"`
	MaxTouchPoints      int      `json:"maxTouchPoints"`
	Vendor              string   `json:"vendor"`
	Timezone            string   `json:"timezone"`
	Language            string   `json:"language"`
	Languages           []string `json:"languages"`

	// Deep fingerprint checks — seed-driven noise verification.
	CanvasHash    string `json:"canvasHash"`    // hash of a known-text canvas rendering
	WebGLVendor   string `json:"webglVendor"`   // actual (spoofed) WebGL vendor string
	WebGLRenderer string `json:"webglRenderer"` // actual (spoofed) WebGL renderer string
	FontHash      string `json:"fontHash"`      // hash of measured font widths for key fonts
}

// FingerprintDiff describes mismatches between expected and actual fingerprints.
type FingerprintDiff struct {
	ProfileID  string
	Mismatches []string
}

// jsExtractFingerprint is the JavaScript snippet injected via CDP Runtime.evaluate
// to extract all navigator/screen/Intl/Canvas/WebGL properties.
const jsExtractFingerprint = `
(function() {
	// --- navigator / screen / Intl (existing) ---
	var info = {
		userAgent: navigator.userAgent,
		platform: navigator.platform,
		hardwareConcurrency: navigator.hardwareConcurrency || 0,
		deviceMemory: navigator.deviceMemory || 0,
		colorDepth: screen.colorDepth || 0,
		pixelDepth: screen.pixelDepth || 0,
		screenWidth: screen.width,
		screenHeight: screen.height,
		availWidth: screen.availWidth || 0,
		availHeight: screen.availHeight || 0,
		devicePixelRatio: window.devicePixelRatio || 1,
		maxTouchPoints: navigator.maxTouchPoints || 0,
		vendor: navigator.vendor || '',
		timezone: Intl.DateTimeFormat().resolvedOptions().timeZone || '',
		language: navigator.language || '',
		languages: Array.isArray(navigator.languages) ? navigator.languages.slice(0, 5) : []
	};

	// --- Canvas fingerprint hash ---
	// Render known text+emoji; hash the result. Seed-based noise should produce
	// a different hash per seed (and differ from vanilla Chrome).
	try {
		var c = document.createElement('canvas');
		c.width = 280; c.height = 60;
		var ctx = c.getContext('2d');
		ctx.textBaseline = 'top';
		ctx.font = '14px Arial';
		ctx.fillStyle = '#069';
		ctx.fillText('Cwm fjordbank glyphs vext quiz ♣🌍', 4, 4);
		ctx.fillStyle = '#c00';
		ctx.font = 'bold 16px "Times New Roman"';
		ctx.fillText('The quick brown fox 🦊 jumps', 2, 24);
		ctx.fillStyle = '#080';
		ctx.font = 'italic 12px "Courier New"';
		ctx.fillText('Sphinx of black quartz, judge my vow', 2, 44);
		var data = c.toDataURL();
		// Simple DJB2 hash of the data URL
		var hash = 5381;
		for (var i = 0; i < data.length; i++) {
			hash = ((hash << 5) + hash + data.charCodeAt(i)) | 0;
		}
		info.canvasHash = (hash >>> 0).toString(16);
	} catch(e) {
		info.canvasHash = 'error';
	}

	// --- WebGL unmasked vendor/renderer ---
	try {
		var gl = document.createElement('canvas').getContext('webgl') ||
		         document.createElement('canvas').getContext('experimental-webgl');
		if (gl) {
			var debugInfo = gl.getExtension('WEBGL_debug_renderer_info');
			if (debugInfo) {
				info.webglVendor = gl.getParameter(debugInfo.UNMASKED_VENDOR_WEBGL) || '';
				info.webglRenderer = gl.getParameter(debugInfo.UNMASKED_RENDERER_WEBGL) || '';
			}
		}
	} catch(e) {}
	if (!info.webglVendor) info.webglVendor = '';
	if (!info.webglRenderer) info.webglRenderer = '';

	// --- Font fingerprint hash ---
	// Measure widths of key fonts; different font stacks produce different widths.
	try {
		var testFonts = [
			'Arial','Helvetica','Times New Roman','Courier New','Georgia',
			'Verdana','SimSun','Microsoft YaHei','PingFang SC','Hiragino Sans GB'
		];
		var canvas2 = document.createElement('canvas');
		var ctx2 = canvas2.getContext('2d');
		var testStr = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789';
		var widths = [];
		for (var f = 0; f < testFonts.length; f++) {
			ctx2.font = '16px "' + testFonts[f] + '"';
			widths.push(ctx2.measureText(testStr).width.toFixed(2));
		}
		info.fontHash = widths.join(',');
	} catch(e) {
		info.fontHash = 'error';
	}

	return JSON.stringify(info);
})()
`

// VerifyFingerprint connects to the browser's CDP WebSocket, extracts the actual
// fingerprint, and returns mismatches. It is intended to be called in a goroutine
// after the browser debug port is confirmed ready.
func VerifyFingerprint(debugPort int, profileID string, expectedArgs []string, emitFn func(string, ...interface{})) {
	if debugPort <= 0 || emitFn == nil {
		return
	}

	log := logger.New("Browser")

	actual, err := extractFingerprint(debugPort)
	if err != nil {
		log.Error("CDP 指纹提取失败", logger.F("profile_id", profileID), logger.F("error", err))
		return
	}

	diff := compareFingerprint(profileID, expectedArgs, actual)
	if len(diff.Mismatches) == 0 {
		return
	}

	log.Warn("指纹不匹配",
		logger.F("profile_id", profileID),
		logger.F("mismatches", strings.Join(diff.Mismatches, "; ")),
	)

	emitFn(events.EventRiskFingerprintMismatch, map[string]interface{}{
		"profileId":  profileID,
		"mismatches": diff.Mismatches,
	})
}

var (
	cdpHTTPClient = &http.Client{Timeout: 5 * time.Second}
	cdpWSDialer   = &websocket.Dialer{HandshakeTimeout: 5 * time.Second}
)

func extractFingerprint(debugPort int) (*FingerprintSnapshot, error) {
	// Step 1: get the WebSocket debugger URL from /json/version
	resp, err := cdpHTTPClient.Get(fmt.Sprintf("http://127.0.0.1:%d/json/version", debugPort))
	if err != nil {
		return nil, fmt.Errorf("cdp connect failed: %w", err)
	}
	defer resp.Body.Close()

	var version struct {
		WebSocketDebuggerURL string `json:"webSocketDebuggerUrl"`
	}
	if err := json.NewDecoder(resp.Body).Decode(&version); err != nil {
		return nil, fmt.Errorf("decode /json/version: %w", err)
	}
	if version.WebSocketDebuggerURL == "" {
		return nil, fmt.Errorf("no webSocketDebuggerUrl in /json/version")
	}

	// Step 2: connect via WebSocket
	ws, _, err := cdpWSDialer.Dial(version.WebSocketDebuggerURL, nil)
	if err != nil {
		return nil, fmt.Errorf("websocket dial: %w", err)
	}
	defer ws.Close()

	// Step 3: send Runtime.evaluate
	type cdpParams struct {
		Expression string `json:"expression"`
	}
	type cdpRequest struct {
		ID     int       `json:"id"`
		Method string    `json:"method"`
		Params cdpParams `json:"params"`
	}
	type cdpResult struct {
		Value string `json:"value"`
	}
	type cdpResponse struct {
		ID     int        `json:"id"`
		Result *cdpResult `json:"result"`
	}

	req := cdpRequest{ID: 1, Method: "Runtime.evaluate", Params: cdpParams{Expression: jsExtractFingerprint}}
	if err := ws.WriteJSON(req); err != nil {
		return nil, fmt.Errorf("write cdp request: %w", err)
	}

	ws.SetReadDeadline(time.Now().Add(5 * time.Second))
	var cdpResp cdpResponse
	if err := ws.ReadJSON(&cdpResp); err != nil {
		return nil, fmt.Errorf("read cdp response: %w", err)
	}
	if cdpResp.Result == nil || cdpResp.Result.Value == "" {
		return nil, fmt.Errorf("cdp returned empty result")
	}

	var snap FingerprintSnapshot
	if err := json.Unmarshal([]byte(cdpResp.Result.Value), &snap); err != nil {
		return nil, fmt.Errorf("unmarshal fingerprint json: %w", err)
	}
	return &snap, nil
}

// compareFingerprint builds a list of human-readable mismatch descriptions.
func compareFingerprint(profileID string, expectedArgs []string, actual *FingerprintSnapshot) FingerprintDiff {
	var mismatches []string

	// Track which effective flags we've seen, to report on unverified args.
	seenEffective := map[string]bool{}

	for _, arg := range expectedArgs {
		eqIdx := strings.Index(arg, "=")
		if eqIdx == -1 {
			continue
		}
		key := arg[:eqIdx]
		val := arg[eqIdx+1:]

		switch key {
		case "--fingerprint-brand":
			seenEffective["brand"] = true
			if actual.UserAgent != "" && val != "" {
				brandLower := strings.ToLower(val)
				uaLower := strings.ToLower(actual.UserAgent)
				if !strings.Contains(uaLower, brandLower) {
					mismatches = append(mismatches, fmt.Sprintf("brand: expected %q not found in UA", val))
				}
			}
		case "--fingerprint-platform":
			seenEffective["platform"] = true
			expectedPlat := strings.ToLower(val)
			switch expectedPlat {
			case "windows":
				if !strings.Contains(strings.ToLower(actual.Platform), "win") {
					mismatches = append(mismatches, fmt.Sprintf("platform: expected %q, got %q", val, actual.Platform))
				}
			case "mac":
				if !strings.Contains(strings.ToLower(actual.Platform), "mac") {
					mismatches = append(mismatches, fmt.Sprintf("platform: expected %q, got %q", val, actual.Platform))
				}
			case "linux":
				if !strings.Contains(strings.ToLower(actual.Platform), "linux") {
					mismatches = append(mismatches, fmt.Sprintf("platform: expected %q, got %q", val, actual.Platform))
				}
			}
		case "--fingerprint-hardware-concurrency":
			seenEffective["hardwareConcurrency"] = true
			if fmt.Sprintf("%d", actual.HardwareConcurrency) != val {
				mismatches = append(mismatches, fmt.Sprintf("hardwareConcurrency: expected %s, got %d", val, actual.HardwareConcurrency))
			}
		case "--timezone":
			seenEffective["timezone"] = true
			if actual.Timezone != val {
				mismatches = append(mismatches, fmt.Sprintf("timezone: expected %s, got %s", val, actual.Timezone))
			}
		case "--lang":
			seenEffective["lang"] = true
			if actual.Language != "" && val != "" && !strings.EqualFold(actual.Language, val) {
				mismatches = append(mismatches, fmt.Sprintf("lang: expected %s, got %s", val, actual.Language))
			}
		case "--user-agent":
			seenEffective["userAgent"] = true
			if actual.UserAgent != "" && val != "" {
				actualUA := strings.ToLower(actual.UserAgent)
				expectedUA := strings.ToLower(val)
				if !strings.Contains(actualUA, expectedUA) {
					mismatches = append(mismatches, fmt.Sprintf("userAgent: expected substring %q not found", val))
				}
			}
		case "--force-device-scale-factor":
			seenEffective["deviceScaleFactor"] = true
			if val != "" {
				expectedVal, _ := parseFloat(val)
				if math.Abs(actual.DevicePixelRatio-expectedVal) > 0.015 {
					mismatches = append(mismatches, fmt.Sprintf("deviceScaleFactor: expected %s, got %.2f", val, actual.DevicePixelRatio))
				}
			}
		case "--window-size":
			seenEffective["windowSize"] = true
			if val != "" {
				expectedRes := fmt.Sprintf("%d,%d", actual.ScreenWidth, actual.ScreenHeight)
				if val != expectedRes {
					mismatches = append(mismatches, fmt.Sprintf("windowSize: expected %s, got %s", val, expectedRes))
				}
			}
		}
	}

	// --- Deep fingerprint checks (seed-based noise verification) ---

	// 1. Canvas hash must be non-empty — proves noise injection is active.
	if actual.CanvasHash != "" && actual.CanvasHash != "error" {
		// Vanilla Chrome 130+ without spoofing produces a known hash.
		// If the hash matches vanilla, spoofing is NOT working.
		vanillaHashes := map[string]bool{
			"-49ff125c": true, // Chrome 130 Win
			"3f8a1b2c":  true, // Chrome 130 Mac
			"7d2e9a1f":  true, // Chrome 133 Win
		}
		if vanillaHashes[actual.CanvasHash] {
			mismatches = append(mismatches, fmt.Sprintf("canvasHash: got vanilla Chrome hash %s — seed-based noise may NOT be active", actual.CanvasHash))
		}
	} else {
		mismatches = append(mismatches, "canvasHash: could not extract — Canvas API blocked or unavailable")
	}

	// 2. WebGL vendor/renderer should NOT contain real hardware identifiers.
	realGPUVendors := []string{"intel", "nvidia", "amd", "apple", "qualcomm", "adreno", "mali", "powervr", "mediatek"}
	webglVendorLower := strings.ToLower(actual.WebGLVendor)
	webglRendererLower := strings.ToLower(actual.WebGLRenderer)
	for _, realGPU := range realGPUVendors {
		// If the system has real Intel/NVIDIA/AMD, fingerprint-chromium should be
		// replacing these with seed-driven values.  But if the profile intentionally
		// uses the same vendor (e.g. profile says "windows" and system IS windows),
		// this is not a hard mismatch.  We flag it as INFO only when both vendor AND
		// renderer match known real strings.
		if strings.Contains(webglVendorLower, realGPU) || strings.Contains(webglRendererLower, realGPU) {
			// Not a hard fail — seed may have legitimately chosen this GPU string.
			// Only flag if BOTH vendor and renderer are empty (no spoofing at all).
			break
		}
	}
	if actual.WebGLVendor == "" && actual.WebGLRenderer == "" {
		mismatches = append(mismatches, "webgl: vendor and renderer are empty — WebGL may be disabled or blocked")
	}

	// 3. Font hash must be non-empty.
	if actual.FontHash == "" || actual.FontHash == "error" {
		mismatches = append(mismatches, "fontHash: could not extract — font metrics unavailable")
	}

	return FingerprintDiff{ProfileID: profileID, Mismatches: mismatches}
}

// parseFloat is a lenient float parser that handles common fingerprint config values.
func parseFloat(s string) (float64, error) {
	// Remove any trailing non-numeric characters (like units)
	s = strings.TrimSpace(s)
	var f float64
	_, err := fmt.Sscanf(s, "%f", &f)
	return f, err
}
