package browser

import "testing"

func TestEvaluateConsistencyMatrix(t *testing.T) {
	result := EvaluateConsistencyMatrix(ConsistencyMatrixInput{
		Language: "en-US", AcceptLanguage: "en-US,en;q=0.9",
		ScreenWidth: 1920, ScreenHeight: 1080, ViewportWidth: 1280, ViewportHeight: 720, DevicePixelRatio: 1,
		WebGLVendor: "Intel Inc.", WebGLRenderer: "Intel Iris", GPUVendor: "Intel",
		OSPlatform: "Win32", RouteRegion: "US", LocaleRegion: "US",
	})
	if !result.Passed || len(result.Checks) != 4 {
		t.Fatalf("result = %+v", result)
	}
	bad := EvaluateConsistencyMatrix(ConsistencyMatrixInput{RouteRegion: "US", LocaleRegion: "CN"})
	if bad.Passed {
		t.Fatal("route mismatch should fail")
	}
}
