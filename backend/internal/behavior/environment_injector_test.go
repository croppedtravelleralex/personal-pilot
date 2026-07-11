package behavior

import (
	"strings"
	"testing"
)

func TestCompileEnvironmentInjectionScriptBuildsSingleBatchScript(t *testing.T) {
	offset := -480
	plan, err := CompileEnvironmentInjectionScript(EnvironmentInjectionProfile{
		Seed:                "profile-1",
		Languages:           []string{"en-US", "en"},
		Platform:            "Win32",
		Vendor:              "Google Inc.",
		UserAgent:           "Mozilla/5.0 test",
		HardwareConcurrency: 8,
		DeviceMemory:        8,
		Timezone:            "America/Los_Angeles",
		TimezoneOffset:      &offset,
		WebGLVendor:         "Intel Inc.",
		WebGLRenderer:       "Intel Iris",
		WebGLExtensions:     []string{"WEBGL_debug_renderer_info"},
		AudioNoise:          0.001,
		FontAllowlist:       []string{"Segoe UI", "Arial"},
		MediaPermission:     "allow",
		ScreenWidth:         1920,
		ScreenHeight:        1080,
		BrandVersion:        "120.0.0.0",
		WebRTCPolicy:        WebRTCPolicy{Mode: "filtered", AllowHosts: []string{"203.0.113.1"}},
		MediaDevices: []MediaDeviceSpec{{Kind: "audioinput", Label: "Microphone", ID: "mic-1", GroupID: "grp"}},
	})
	if err != nil {
		t.Fatalf("compile environment injection: %v", err)
	}
	needles := []string{
		"defineGetter(navProto, 'webdriver', undefined)",
		"defineGetter(navProto, 'languages'",
		"root.chrome = {",
		"replaceMethod(root.navigator.permissions, 'query'",
		"replaceMethod(HTMLCanvasElement.prototype, 'toDataURL'",
		"replaceMethod(CanvasRenderingContext2D.prototype, 'getImageData'",
		"replaceMethod(Intl.DateTimeFormat.prototype, 'resolvedOptions'",
		"replaceMethod(Date.prototype, 'getTimezoneOffset'",
		"getSupportedExtensions",
		"replaceMethod(proto, 'getShaderPrecisionFormat'",
		"replaceMethod(proto, 'readPixels'",
		"replaceMethod(AnalyserNode.prototype, 'getFloatFrequencyData'",
		"replaceMethod(AudioBuffer.prototype, 'copyFromChannel'",
		"replaceMethod(speechSynthesis, 'getVoices'",
		"replaceMethod(nav, 'getBattery'",
		"defineGetter(conn, 'effectiveType'",
		"replaceMethod(root, 'matchMedia'",
		"replaceMethod(Element.prototype, 'getBoundingClientRect'",
		"replaceMethod(Element.prototype, 'getClientRects'",
		"replaceMethod(nav.userAgentData, 'getHighEntropyValues'",
		"replaceMethod(CanvasRenderingContext2D.prototype, 'measureText'",
		"defineGetter(root.screen, 'width'",
		"replaceMethod(nav.storage, 'estimate'",
		"Document.prototype, 'fonts'",
		"enumerateDevices",
		"getUserMedia",
		"RTCPeerConnection",
		"a=candidate:",
		"function installEnvironment(profile)",
		"WorkerNavigator",
		"OffscreenCanvas",
		"wrapClassicWorker(Worker, 'Worker')",
		"importScripts(",
		"import ' + JSON.stringify",
		"navigator.serviceWorker.register = async function register",
		"PDF Viewer",
		"WebKit built-in PDF",
	}
	for _, needle := range needles {
		if !strings.Contains(plan.Script, needle) {
			t.Fatalf("script missing %q", needle)
		}
	}
	if plan.HeaderOverrides["Accept-Language"] != "en-US,en;q=0.9" {
		t.Fatalf("Accept-Language = %q", plan.HeaderOverrides["Accept-Language"])
	}
	if len(plan.AppliedFamilies) < 8 {
		t.Fatalf("applied families = %v", plan.AppliedFamilies)
	}
	foundWorker := false
	for _, family := range plan.AppliedFamilies {
		if family == "worker_scope" {
			foundWorker = true
			break
		}
	}
	if !foundWorker {
		t.Fatalf("applied families missing worker_scope: %v", plan.AppliedFamilies)
	}
	if len(plan.Warnings) != 0 {
		t.Fatalf("warnings = %v", plan.Warnings)
	}
}

func TestCompileEnvironmentInjectionScriptWarnsOnIncompleteIdentity(t *testing.T) {
	plan, err := CompileEnvironmentInjectionScript(EnvironmentInjectionProfile{AcceptLanguage: "ja-JP,ja;q=0.9"})
	if err != nil {
		t.Fatalf("compile environment injection: %v", err)
	}
	if plan.HeaderOverrides["Accept-Language"] != "ja-JP,ja;q=0.9" {
		t.Fatalf("Accept-Language header not preserved: %q", plan.HeaderOverrides["Accept-Language"])
	}
	want := map[string]bool{
		"timezone_not_configured":                     false,
		"user_agent_platform_vendor_chain_incomplete": false,
		"webgl_identity_incomplete":                   false,
	}
	for _, warning := range plan.Warnings {
		if _, ok := want[warning]; ok {
			want[warning] = true
		}
	}
	for warning, seen := range want {
		if !seen {
			t.Fatalf("missing warning %s in %v", warning, plan.Warnings)
		}
	}
}

func TestStableProfileSeedDeterministic(t *testing.T) {
	if stableProfileSeed("profile") != stableProfileSeed("profile") {
		t.Fatal("stableProfileSeed should be deterministic")
	}
	if stableProfileSeed("profile") == stableProfileSeed("other") {
		t.Fatal("stableProfileSeed should differ for distinct input")
	}
}

func TestCompileEnvironmentInjectionScriptNativeizesReplacedFunctions(t *testing.T) {
	plan, err := CompileEnvironmentInjectionScript(EnvironmentInjectionProfile{
		Seed:            "native-test",
		Timezone:        "Asia/Shanghai",
		FontAllowlist:   []string{"Segoe UI"},
		MediaPermission: "prompt",
		MediaDevices:    []MediaDeviceSpec{{Kind: "audioinput", ID: "mic"}},
		WebRTCPolicy:    WebRTCPolicy{Mode: "block_private_candidates"},
	})
	if err != nil {
		t.Fatalf("compile environment injection: %v", err)
	}
	needles := []string{
		"const nativeFunctionMap = new WeakMap()",
		"nativeFunctionMap.set(patchedFunctionToString, originalFunctionToString)",
		"Object.defineProperty(Function.prototype, 'toString'",
		"makeNative(getter, originalGetter || originalFunctionToString)",
		"replaceMethod(Intl.DateTimeFormat.prototype, 'resolvedOptions'",
		"replaceMethod(HTMLCanvasElement.prototype, 'toDataURL'",
		"replaceMethod(proto, 'getParameter'",
		"replaceMethod(AnalyserNode.prototype, 'getFloatFrequencyData'",
		"replaceMethod(nav.mediaDevices, 'enumerateDevices'",
		"root.RTCPeerConnection = makeNative(",
		"wrapClassicWorker(Worker, 'Worker')",
	}
	for _, needle := range needles {
		if !strings.Contains(plan.Script, needle) {
			t.Errorf("script missing nativeization marker %q", needle)
		}
	}
}

func TestShouldPatchCurrentDocument(t *testing.T) {
	cases := []struct {
		url  string
		want bool
	}{
		{"", true},
		{"about:blank", true},
		{"chrome://newtab", true},
		{"https://example.com", false},
		{"http://localhost:8080", false},
	}
	for _, tc := range cases {
		if got := shouldPatchCurrentDocument(tc.url); got != tc.want {
			t.Fatalf("shouldPatchCurrentDocument(%q) = %v, want %v", tc.url, got, tc.want)
		}
	}
}

func TestApplyEnvironmentInjectionSkipsCurrentPageOnHTTPS(t *testing.T) {
	var methods []string
	executor := &CDPExecutor{
		sendCommandHook: func(method string, params interface{}) ([]byte, error) {
			methods = append(methods, method)
			switch method {
			case "Runtime.evaluate":
				expr := ""
				if m, ok := params.(map[string]interface{}); ok {
					if v, ok := m["expression"].(string); ok {
						expr = v
					}
				}
				if expr == `location.href` {
					return []byte(`{"result":{"value":"https://example.com/"}}`), nil
				}
				return []byte(`{}`), nil
			default:
				return []byte(`{}`), nil
			}
		},
	}
	plan, err := executor.ApplyEnvironmentInjection(EnvironmentInjectionProfile{
		Seed:          "skip-test",
		Timezone:      "Asia/Shanghai",
		Platform:      "Win32",
		Vendor:        "Google Inc.",
		UserAgent:     "Mozilla/5.0 test",
		WebGLVendor:   "Intel Inc.",
		WebGLRenderer: "Intel Iris",
	})
	if err != nil {
		t.Fatalf("ApplyEnvironmentInjection: %v", err)
	}
	scriptEvalCount := 0
	for _, method := range methods {
		if method != "Runtime.evaluate" {
			continue
		}
		scriptEvalCount++
	}
	if scriptEvalCount != 1 {
		t.Fatalf("expected only location.href Runtime.evaluate, methods=%v", methods)
	}
	foundDeferred := false
	for _, warning := range plan.Warnings {
		if warning == "current_page_environment_patch_deferred_to_navigation" {
			foundDeferred = true
			break
		}
	}
	if !foundDeferred {
		t.Fatalf("expected deferred warning, got %v", plan.Warnings)
	}
}

func TestApplyEnvironmentInjectionPatchesAboutBlank(t *testing.T) {
	var methods []string
	executor := &CDPExecutor{
		sendCommandHook: func(method string, params interface{}) ([]byte, error) {
			methods = append(methods, method)
			switch method {
			case "Runtime.evaluate":
				expr := ""
				if m, ok := params.(map[string]interface{}); ok {
					if v, ok := m["expression"].(string); ok {
						expr = v
					}
				}
				if expr == `location.href` {
					return []byte(`{"result":{"value":"about:blank"}}`), nil
				}
				return []byte(`{}`), nil
			default:
				return []byte(`{}`), nil
			}
		},
	}
	_, err := executor.ApplyEnvironmentInjection(EnvironmentInjectionProfile{
		Seed:        "blank-test",
		Timezone:    "Asia/Shanghai",
		Platform:    "Win32",
		Vendor:      "Google Inc.",
		UserAgent:   "Mozilla/5.0 test",
		WebGLVendor: "Intel Inc.",
		WebGLRenderer: "Intel Iris",
	})
	if err != nil {
		t.Fatalf("ApplyEnvironmentInjection: %v", err)
	}
	evalCount := 0
	for _, method := range methods {
		if method == "Runtime.evaluate" {
			evalCount++
		}
	}
	if evalCount < 2 {
		t.Fatalf("expected URL probe + script evaluate on about:blank, methods=%v", methods)
	}
}
