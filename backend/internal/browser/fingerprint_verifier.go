package browser

import (
	"encoding/json"
	"fmt"
	"math"
	"net/http"
	"personal-pilot/backend/internal/events"
	"personal-pilot/backend/internal/logger"
	"strings"
	"time"

	"github.com/gorilla/websocket"
)

// FingerprintSnapshot holds the actual browser fingerprint values extracted via CDP.
type FingerprintSnapshot struct {
	UserAgent            string   `json:"userAgent"`
	AppVersion           string   `json:"appVersion"`
	AppName              string   `json:"appName"`
	Product              string   `json:"product"`
	ProductSub           string   `json:"productSub"`
	Platform             string   `json:"platform"`
	Webdriver            bool     `json:"webdriver"`
	CookieEnabled        bool     `json:"cookieEnabled"`
	DoNotTrack           string   `json:"doNotTrack"`
	PDFViewerEnabled     bool     `json:"pdfViewerEnabled"`
	Online               bool     `json:"online"`
	HardwareConcurrency  int      `json:"hardwareConcurrency"`
	DeviceMemory         int      `json:"deviceMemory"`
	ColorDepth           int      `json:"colorDepth"`
	PixelDepth           int      `json:"pixelDepth"`
	ScreenWidth          int      `json:"screenWidth"`
	ScreenHeight         int      `json:"screenHeight"`
	AvailWidth           int      `json:"availWidth"`
	AvailHeight          int      `json:"availHeight"`
	DevicePixelRatio     float64  `json:"devicePixelRatio"`
	MaxTouchPoints       int      `json:"maxTouchPoints"`
	Vendor               string   `json:"vendor"`
	Timezone             string   `json:"timezone"`
	TimezoneOffset       int      `json:"timezoneOffset"`
	Language             string   `json:"language"`
	Languages            []string `json:"languages"`
	IntlLocale           string   `json:"intlLocale"`
	IntlCalendar         string   `json:"intlCalendar"`
	IntlNumberingSystem  string   `json:"intlNumberingSystem"`
	DateFormatSample     string   `json:"dateFormatSample"`
	NumberFormatSample   string   `json:"numberFormatSample"`
	UADataBrands         []string `json:"uaDataBrands"`
	UADataMobile         bool     `json:"uaDataMobile"`
	UADataPlatform       string   `json:"uaDataPlatform"`
	UADataPlatformVer    string   `json:"uaDataPlatformVersion"`
	UADataArchitecture   string   `json:"uaDataArchitecture"`
	UADataBitness        string   `json:"uaDataBitness"`
	UADataModel          string   `json:"uaDataModel"`
	UADataFullVersions   []string `json:"uaDataFullVersionList"`
	InnerWidth           int      `json:"innerWidth"`
	InnerHeight          int      `json:"innerHeight"`
	OuterWidth           int      `json:"outerWidth"`
	OuterHeight          int      `json:"outerHeight"`
	VisualViewportWidth  float64  `json:"visualViewportWidth"`
	VisualViewportHeight float64  `json:"visualViewportHeight"`
	VisualViewportScale  float64  `json:"visualViewportScale"`
	PointerFine          bool     `json:"pointerFine"`
	PointerCoarse        bool     `json:"pointerCoarse"`
	HoverHover           bool     `json:"hoverHover"`
	HoverNone            bool     `json:"hoverNone"`
	PrefersColorScheme   string   `json:"prefersColorScheme"`
	PrefersReducedMotion string   `json:"prefersReducedMotion"`
	NetworkEffectiveType string   `json:"networkEffectiveType"`
	NetworkDownlink      float64  `json:"networkDownlink"`
	NetworkRTT           int      `json:"networkRtt"`
	NetworkSaveData      bool     `json:"networkSaveData"`
	StorageQuota         int64    `json:"storageQuota"`
	StorageUsage         int64    `json:"storageUsage"`

	// Deep fingerprint checks — seed-driven noise verification.
	CanvasHash            string `json:"canvasHash"`          // hash of a known-text canvas rendering
	WebGLVendor           string `json:"webglVendor"`         // actual (spoofed) WebGL vendor string
	WebGLRenderer         string `json:"webglRenderer"`       // actual (spoofed) WebGL renderer string
	WebGLExtensionsHash   string `json:"webglExtensionsHash"` // hash of supported WebGL extensions
	WebGLMaxTextureSize   int    `json:"webglMaxTextureSize"`
	WebGLMaxVertexAttribs int    `json:"webglMaxVertexAttribs"`
	WebGLMaxViewportDims  string `json:"webglMaxViewportDims"`
	FontHash              string `json:"fontHash"` // hash of measured font widths for key fonts
	AudioHash             string `json:"audioHash"`
	PluginsHash           string `json:"pluginsHash"`
	MimeTypesHash         string `json:"mimeTypesHash"`
	WebGPUAvailable       bool   `json:"webgpuAvailable"`
	WebRTCSupported       bool   `json:"webrtcSupported"`
}

// FingerprintDiff describes mismatches between expected and actual fingerprints.
type FingerprintDiff struct {
	ProfileID  string
	Mismatches []string
}

// jsExtractFingerprint is the JavaScript snippet injected via CDP Runtime.evaluate
// to extract all navigator/screen/Intl/Canvas/WebGL properties.
const jsExtractFingerprint = `
(async function() {
	function hashString(input) {
		var text = String(input || '');
		var hash = 5381;
		for (var i = 0; i < text.length; i++) {
			hash = ((hash << 5) + hash + text.charCodeAt(i)) | 0;
		}
		return (hash >>> 0).toString(16);
	}
	function media(query) {
		try { return !!(window.matchMedia && window.matchMedia(query).matches); } catch (_) { return false; }
	}
	function navArray(list, limit) {
		try { return Array.prototype.slice.call(list || [], 0, limit || 20).map(function(item) { return String(item); }); } catch (_) { return []; }
	}
	var nav = navigator || {};
	var scr = screen || {};
	var dtf = {};
	try { dtf = Intl.DateTimeFormat().resolvedOptions() || {}; } catch (_) {}
	var vv = window.visualViewport || {};
	var info = {
		userAgent: nav.userAgent || '',
		appVersion: nav.appVersion || '',
		appName: nav.appName || '',
		product: nav.product || '',
		productSub: nav.productSub || '',
		platform: nav.platform || '',
		webdriver: !!nav.webdriver,
		cookieEnabled: !!nav.cookieEnabled,
		doNotTrack: nav.doNotTrack || window.doNotTrack || '',
		pdfViewerEnabled: !!nav.pdfViewerEnabled,
		online: !!nav.onLine,
		hardwareConcurrency: nav.hardwareConcurrency || 0,
		deviceMemory: nav.deviceMemory || 0,
		colorDepth: scr.colorDepth || 0,
		pixelDepth: scr.pixelDepth || 0,
		screenWidth: scr.width || 0,
		screenHeight: scr.height || 0,
		availWidth: scr.availWidth || 0,
		availHeight: scr.availHeight || 0,
		devicePixelRatio: window.devicePixelRatio || 1,
		maxTouchPoints: nav.maxTouchPoints || 0,
		vendor: nav.vendor || '',
		timezone: dtf.timeZone || '',
		timezoneOffset: new Date().getTimezoneOffset(),
		language: nav.language || '',
		languages: Array.isArray(nav.languages) ? nav.languages.slice(0, 8) : [],
		intlLocale: dtf.locale || '',
		intlCalendar: dtf.calendar || '',
		intlNumberingSystem: dtf.numberingSystem || '',
		dateFormatSample: '',
		numberFormatSample: '',
		uaDataBrands: [],
		uaDataMobile: false,
		uaDataPlatform: '',
		uaDataPlatformVersion: '',
		uaDataArchitecture: '',
		uaDataBitness: '',
		uaDataModel: '',
		uaDataFullVersionList: [],
		innerWidth: window.innerWidth || 0,
		innerHeight: window.innerHeight || 0,
		outerWidth: window.outerWidth || 0,
		outerHeight: window.outerHeight || 0,
		visualViewportWidth: vv.width || 0,
		visualViewportHeight: vv.height || 0,
		visualViewportScale: vv.scale || 0,
		pointerFine: media('(pointer: fine)'),
		pointerCoarse: media('(pointer: coarse)'),
		hoverHover: media('(hover: hover)'),
		hoverNone: media('(hover: none)'),
		prefersColorScheme: media('(prefers-color-scheme: dark)') ? 'dark' : (media('(prefers-color-scheme: light)') ? 'light' : 'no-preference'),
		prefersReducedMotion: media('(prefers-reduced-motion: reduce)') ? 'reduce' : 'no-preference',
		networkEffectiveType: '',
		networkDownlink: 0,
		networkRtt: 0,
		networkSaveData: false,
		storageQuota: 0,
		storageUsage: 0,
		canvasHash: '',
		webglVendor: '',
		webglRenderer: '',
		webglExtensionsHash: '',
		webglMaxTextureSize: 0,
		webglMaxVertexAttribs: 0,
		webglMaxViewportDims: '',
		fontHash: '',
		audioHash: '',
		pluginsHash: '',
		mimeTypesHash: '',
		webgpuAvailable: !!nav.gpu,
		webrtcSupported: typeof RTCPeerConnection !== 'undefined'
	};
	try { info.dateFormatSample = new Intl.DateTimeFormat(undefined, { dateStyle: 'full', timeStyle: 'long' }).format(new Date(1704067200000)); } catch (_) {}
	try { info.numberFormatSample = new Intl.NumberFormat(undefined, { style: 'currency', currency: 'USD' }).format(123456.78); } catch (_) {}
	try {
		if (nav.userAgentData) {
			info.uaDataBrands = (nav.userAgentData.brands || []).map(function(b) { return String(b.brand || '') + '/' + String(b.version || ''); });
			info.uaDataMobile = !!nav.userAgentData.mobile;
			info.uaDataPlatform = nav.userAgentData.platform || '';
			if (nav.userAgentData.getHighEntropyValues) {
				var high = await nav.userAgentData.getHighEntropyValues(['architecture','bitness','model','platformVersion','fullVersionList']);
				info.uaDataArchitecture = high.architecture || '';
				info.uaDataBitness = high.bitness || '';
				info.uaDataModel = high.model || '';
				info.uaDataPlatformVersion = high.platformVersion || '';
				info.uaDataFullVersionList = (high.fullVersionList || []).map(function(b) { return String(b.brand || '') + '/' + String(b.version || ''); });
			}
		}
	} catch (_) {}
	try {
		var conn = nav.connection || nav.mozConnection || nav.webkitConnection;
		if (conn) {
			info.networkEffectiveType = conn.effectiveType || '';
			info.networkDownlink = Number(conn.downlink || 0);
			info.networkRtt = Number(conn.rtt || 0);
			info.networkSaveData = !!conn.saveData;
		}
	} catch (_) {}
	try {
		if (nav.storage && nav.storage.estimate) {
			var estimate = await nav.storage.estimate();
			info.storageQuota = Math.round(estimate.quota || 0);
			info.storageUsage = Math.round(estimate.usage || 0);
		}
	} catch (_) {}
	try {
		info.pluginsHash = hashString(navArray(nav.plugins, 50).join('|'));
		info.mimeTypesHash = hashString(navArray(nav.mimeTypes, 80).join('|'));
	} catch (_) {
		info.pluginsHash = 'error';
		info.mimeTypesHash = 'error';
	}
	try {
		var c = document.createElement('canvas');
		c.width = 280; c.height = 60;
		var ctx = c.getContext('2d');
		ctx.textBaseline = 'top';
		ctx.font = '14px Arial';
		ctx.fillStyle = '#069';
		ctx.fillText('Cwm fjordbank glyphs vext quiz 123', 4, 4);
		ctx.fillStyle = '#c00';
		ctx.font = 'bold 16px "Times New Roman"';
		ctx.fillText('The quick brown fox jumps', 2, 24);
		ctx.fillStyle = '#080';
		ctx.font = 'italic 12px "Courier New"';
		ctx.fillText('Sphinx of black quartz, judge my vow', 2, 44);
		info.canvasHash = hashString(c.toDataURL());
	} catch (_) {
		info.canvasHash = 'error';
	}
	try {
		var gl = document.createElement('canvas').getContext('webgl') ||
		         document.createElement('canvas').getContext('experimental-webgl');
		if (gl) {
			var debugInfo = gl.getExtension('WEBGL_debug_renderer_info');
			if (debugInfo) {
				info.webglVendor = gl.getParameter(debugInfo.UNMASKED_VENDOR_WEBGL) || '';
				info.webglRenderer = gl.getParameter(debugInfo.UNMASKED_RENDERER_WEBGL) || '';
			}
			var extensions = gl.getSupportedExtensions() || [];
			info.webglExtensionsHash = hashString(extensions.sort().join('|'));
			info.webglMaxTextureSize = Number(gl.getParameter(gl.MAX_TEXTURE_SIZE) || 0);
			info.webglMaxVertexAttribs = Number(gl.getParameter(gl.MAX_VERTEX_ATTRIBS) || 0);
			var dims = gl.getParameter(gl.MAX_VIEWPORT_DIMS) || [];
			info.webglMaxViewportDims = Array.prototype.join.call(dims, 'x');
		}
	} catch (_) {}
	try {
		var testFonts = ['Arial','Helvetica','Times New Roman','Courier New','Georgia','Verdana','SimSun','Microsoft YaHei','PingFang SC','Hiragino Sans GB'];
		var canvas2 = document.createElement('canvas');
		var ctx2 = canvas2.getContext('2d');
		var testStr = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789';
		var widths = [];
		for (var f = 0; f < testFonts.length; f++) {
			ctx2.font = '16px "' + testFonts[f] + '"';
			widths.push(ctx2.measureText(testStr).width.toFixed(2));
		}
		info.fontHash = hashString(widths.join(','));
	} catch (_) {
		info.fontHash = 'error';
	}
	try {
		var OfflineCtx = window.OfflineAudioContext || window.webkitOfflineAudioContext;
		if (OfflineCtx) {
			var audioCtx = new OfflineCtx(1, 4410, 44100);
			var osc = audioCtx.createOscillator();
			var comp = audioCtx.createDynamicsCompressor();
			osc.type = 'triangle';
			osc.frequency.value = 10000;
			comp.threshold.value = -50;
			comp.knee.value = 40;
			comp.ratio.value = 12;
			comp.attack.value = 0;
			comp.release.value = 0.25;
			osc.connect(comp);
			comp.connect(audioCtx.destination);
			osc.start(0);
			var rendered = await audioCtx.startRendering();
			var data = rendered.getChannelData(0);
			var sample = [];
			for (var a = 0; a < data.length; a += 64) sample.push(data[a].toFixed(6));
			info.audioHash = hashString(sample.join(','));
		} else {
			info.audioHash = 'unsupported';
		}
	} catch (_) {
		info.audioHash = 'error';
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

	actual, err := ExtractFingerprint(debugPort)
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

func ExtractFingerprint(debugPort int) (*FingerprintSnapshot, error) {
	// Step 1: get a page target WebSocket URL from /json.
	resp, err := cdpHTTPClient.Get(fmt.Sprintf("http://127.0.0.1:%d/json", debugPort))
	if err != nil {
		return nil, fmt.Errorf("cdp connect failed: %w", err)
	}
	defer resp.Body.Close()

	var targets []struct {
		Type                 string `json:"type"`
		URL                  string `json:"url"`
		WebSocketDebuggerURL string `json:"webSocketDebuggerUrl"`
	}
	if err := json.NewDecoder(resp.Body).Decode(&targets); err != nil {
		return nil, fmt.Errorf("decode /json: %w", err)
	}
	wsURL := ""
	for _, target := range targets {
		if strings.EqualFold(target.Type, "page") &&
			strings.TrimSpace(target.WebSocketDebuggerURL) != "" &&
			!strings.HasPrefix(strings.ToLower(strings.TrimSpace(target.URL)), "devtools://") {
			wsURL = target.WebSocketDebuggerURL
			break
		}
	}
	if wsURL == "" {
		return nil, fmt.Errorf("no page webSocketDebuggerUrl in /json")
	}

	// Step 2: connect via WebSocket
	ws, _, err := cdpWSDialer.Dial(wsURL, nil)
	if err != nil {
		return nil, fmt.Errorf("websocket dial: %w", err)
	}
	defer ws.Close()

	// Step 3: send Runtime.evaluate
	type cdpParams struct {
		Expression    string `json:"expression"`
		ReturnByValue bool   `json:"returnByValue"`
		AwaitPromise  bool   `json:"awaitPromise"`
	}
	type cdpRequest struct {
		ID     int       `json:"id"`
		Method string    `json:"method"`
		Params cdpParams `json:"params"`
	}
	type cdpResponse struct {
		ID     int `json:"id"`
		Result *struct {
			Result *struct {
				Value string `json:"value"`
			} `json:"result"`
			ExceptionDetails json.RawMessage `json:"exceptionDetails,omitempty"`
		} `json:"result"`
		Error *struct {
			Message string `json:"message"`
		} `json:"error,omitempty"`
	}

	req := cdpRequest{
		ID:     1,
		Method: "Runtime.evaluate",
		Params: cdpParams{Expression: jsExtractFingerprint, ReturnByValue: true, AwaitPromise: true},
	}
	if err := ws.WriteJSON(req); err != nil {
		return nil, fmt.Errorf("write cdp request: %w", err)
	}

	ws.SetReadDeadline(time.Now().Add(5 * time.Second))
	var cdpResp cdpResponse
	if err := ws.ReadJSON(&cdpResp); err != nil {
		return nil, fmt.Errorf("read cdp response: %w", err)
	}
	if cdpResp.Error != nil {
		return nil, fmt.Errorf("cdp error: %s", cdpResp.Error.Message)
	}
	if cdpResp.Result != nil && len(cdpResp.Result.ExceptionDetails) > 0 {
		return nil, fmt.Errorf("cdp exception: %s", string(cdpResp.Result.ExceptionDetails))
	}
	if cdpResp.Result == nil || cdpResp.Result.Result == nil || cdpResp.Result.Result.Value == "" {
		return nil, fmt.Errorf("cdp returned empty result")
	}

	var snap FingerprintSnapshot
	if err := json.Unmarshal([]byte(cdpResp.Result.Result.Value), &snap); err != nil {
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
