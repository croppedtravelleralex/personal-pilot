package browser

// ConsistencyMatrixInput is the roadmap-level contract for checking whether the
// profile environment, viewport, GPU, locale, and route fields agree before launch.
type ConsistencyMatrixInput struct {
	Timezone         string
	Language         string
	AcceptLanguage   string
	ScreenWidth      int
	ScreenHeight     int
	ViewportWidth    int
	ViewportHeight   int
	DevicePixelRatio float64
	WebGLVendor      string
	WebGLRenderer    string
	GPUVendor        string
	OSPlatform       string
	RouteRegion      string
	LocaleRegion     string
}

type ConsistencyMatrixResult struct {
	Passed bool
	Checks []ConsistencyMatrixCheck
}

type ConsistencyMatrixCheck struct {
	ID     string
	Passed bool
	Detail string
}

func EvaluateConsistencyMatrix(input ConsistencyMatrixInput) ConsistencyMatrixResult {
	checks := []ConsistencyMatrixCheck{
		checkTimezoneLanguage(input),
		checkScreenViewportDPR(input),
		checkWebGLGPUOS(input),
		checkRouteLocale(input),
	}
	passed := true
	for _, check := range checks {
		if !check.Passed {
			passed = false
		}
	}
	return ConsistencyMatrixResult{Passed: passed, Checks: checks}
}

func checkTimezoneLanguage(input ConsistencyMatrixInput) ConsistencyMatrixCheck {
	if input.Language == "" || input.AcceptLanguage == "" {
		return ConsistencyMatrixCheck{ID: "timezone_language_accept_language", Passed: true, Detail: "skipped_missing_language"}
	}
	prefix := input.Language
	if len(prefix) > 2 {
		prefix = prefix[:2]
	}
	return ConsistencyMatrixCheck{ID: "timezone_language_accept_language", Passed: len(input.AcceptLanguage) >= 2 && input.AcceptLanguage[:2] == prefix, Detail: input.Language + " vs " + input.AcceptLanguage}
}

func checkScreenViewportDPR(input ConsistencyMatrixInput) ConsistencyMatrixCheck {
	if input.ScreenWidth <= 0 || input.ScreenHeight <= 0 || input.ViewportWidth <= 0 || input.ViewportHeight <= 0 {
		return ConsistencyMatrixCheck{ID: "screen_viewport_dpr", Passed: true, Detail: "skipped_missing_dimensions"}
	}
	if input.DevicePixelRatio <= 0 {
		input.DevicePixelRatio = 1
	}
	passed := float64(input.ViewportWidth) <= float64(input.ScreenWidth)*input.DevicePixelRatio && float64(input.ViewportHeight) <= float64(input.ScreenHeight)*input.DevicePixelRatio
	return ConsistencyMatrixCheck{ID: "screen_viewport_dpr", Passed: passed, Detail: "viewport_must_fit_screen_dpr"}
}

func checkWebGLGPUOS(input ConsistencyMatrixInput) ConsistencyMatrixCheck {
	if input.WebGLVendor == "" || input.GPUVendor == "" {
		return ConsistencyMatrixCheck{ID: "webgl_gpu_os", Passed: true, Detail: "skipped_missing_gpu"}
	}
	passed := containsFoldMatrix(input.WebGLVendor, input.GPUVendor) || containsFoldMatrix(input.WebGLRenderer, input.GPUVendor)
	if input.OSPlatform != "" && containsFoldMatrix(input.WebGLRenderer, "Apple") && !containsFoldMatrix(input.OSPlatform, "mac") {
		passed = false
	}
	return ConsistencyMatrixCheck{ID: "webgl_gpu_os", Passed: passed, Detail: input.WebGLVendor + " / " + input.WebGLRenderer + " / " + input.OSPlatform}
}

func checkRouteLocale(input ConsistencyMatrixInput) ConsistencyMatrixCheck {
	if input.RouteRegion == "" || input.LocaleRegion == "" {
		return ConsistencyMatrixCheck{ID: "route_timezone_language", Passed: true, Detail: "skipped_missing_region"}
	}
	return ConsistencyMatrixCheck{ID: "route_timezone_language", Passed: input.RouteRegion == input.LocaleRegion, Detail: input.RouteRegion + " vs " + input.LocaleRegion}
}

func containsFoldMatrix(text, part string) bool {
	if part == "" {
		return true
	}
	if len(part) > len(text) {
		return false
	}
	for i := 0; i <= len(text)-len(part); i++ {
		match := true
		for j := range part {
			a, b := text[i+j], part[j]
			if a >= 'A' && a <= 'Z' {
				a += 32
			}
			if b >= 'A' && b <= 'Z' {
				b += 32
			}
			if a != b {
				match = false
				break
			}
		}
		if match {
			return true
		}
	}
	return false
}
