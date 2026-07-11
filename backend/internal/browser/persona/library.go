package persona

// DevicePersona is a coherent device template (docs/52 DP1).
// Source must cite a public reference or observed probe; do not invent hardware combos.
type DevicePersona struct {
	ID                   string   `json:"id"`
	Source               string   `json:"source"`
	OSName               string   `json:"osName"`
	OSVersion            string   `json:"osVersion"`
	Platform             string   `json:"platform"` // windows | macos
	UAPlatformToken      string   `json:"uaPlatformToken"`
	GPUVendor            string   `json:"gpuVendor"`
	GPURenderer          string   `json:"gpuRenderer"`
	ScreenWidth          int      `json:"screenWidth"`
	ScreenHeight         int      `json:"screenHeight"`
	DevicePixelRatio     float64  `json:"devicePixelRatio"`
	HardwareConcurrency  int      `json:"hardwareConcurrency"`
	DeviceMemoryGB       int      `json:"deviceMemoryGB"`
	MaxTouchPoints       int      `json:"maxTouchPoints"`
	FontAllowlist        []string `json:"fontAllowlist"`
	AcceptLang           string   `json:"acceptLang"`
	Timezone             string   `json:"timezone"`
}

// Library returns the built-in persona set (≥12).
func Library() []DevicePersona {
	winFonts := []string{"Arial", "Calibri", "Cambria", "Consolas", "Courier New", "Georgia", "Segoe UI", "Tahoma", "Times New Roman", "Verdana"}
	macFonts := []string{"Helvetica", "Helvetica Neue", "Menlo", "Monaco", "SF Pro Text", "Times", "Arial", "Courier New", "Georgia", "Verdana"}
	return []DevicePersona{
		{
			ID: "win11-uhd620-1080-8c", Source: "public-spec: Intel UHD Graphics 620 + Win11 22H2 reference SKU",
			OSName: "Windows", OSVersion: "15.0.0", Platform: "windows", UAPlatformToken: "Windows NT 10.0; Win64; x64",
			GPUVendor: "Google Inc. (Intel)", GPURenderer: "ANGLE (Intel, Intel(R) UHD Graphics 620 Direct3D11 vs_5_0 ps_5_0, D3D11)",
			ScreenWidth: 1920, ScreenHeight: 1080, DevicePixelRatio: 1, HardwareConcurrency: 8, DeviceMemoryGB: 8,
			FontAllowlist: winFonts, AcceptLang: "zh-CN,zh;q=0.9,en;q=0.8", Timezone: "Asia/Shanghai",
		},
		{
			ID: "win11-irisxe-1440-12c", Source: "public-spec: Intel Iris Xe + 2560x1440 Win11 laptop class",
			OSName: "Windows", OSVersion: "15.0.0", Platform: "windows", UAPlatformToken: "Windows NT 10.0; Win64; x64",
			GPUVendor: "Google Inc. (Intel)", GPURenderer: "ANGLE (Intel, Intel(R) Iris(R) Xe Graphics Direct3D11 vs_5_0 ps_5_0, D3D11)",
			ScreenWidth: 2560, ScreenHeight: 1440, DevicePixelRatio: 1.25, HardwareConcurrency: 12, DeviceMemoryGB: 16,
			FontAllowlist: winFonts, AcceptLang: "en-US,en;q=0.9", Timezone: "America/New_York",
		},
		{
			ID: "win10-gtx1650-1080-8c", Source: "public-spec: NVIDIA GTX 1650 + Win10 desktop class",
			OSName: "Windows", OSVersion: "10.0.0", Platform: "windows", UAPlatformToken: "Windows NT 10.0; Win64; x64",
			GPUVendor: "Google Inc. (NVIDIA)", GPURenderer: "ANGLE (NVIDIA, NVIDIA GeForce GTX 1650 Direct3D11 vs_5_0 ps_5_0, D3D11)",
			ScreenWidth: 1920, ScreenHeight: 1080, DevicePixelRatio: 1, HardwareConcurrency: 8, DeviceMemoryGB: 16,
			FontAllowlist: winFonts, AcceptLang: "zh-CN,zh;q=0.9,en;q=0.8", Timezone: "Asia/Shanghai",
		},
		{
			ID: "win11-rtx3060-1440-16c", Source: "public-spec: NVIDIA RTX 3060 + Win11 gaming desktop class",
			OSName: "Windows", OSVersion: "15.0.0", Platform: "windows", UAPlatformToken: "Windows NT 10.0; Win64; x64",
			GPUVendor: "Google Inc. (NVIDIA)", GPURenderer: "ANGLE (NVIDIA, NVIDIA GeForce RTX 3060 Direct3D11 vs_5_0 ps_5_0, D3D11)",
			ScreenWidth: 2560, ScreenHeight: 1440, DevicePixelRatio: 1, HardwareConcurrency: 16, DeviceMemoryGB: 32,
			FontAllowlist: winFonts, AcceptLang: "en-US,en;q=0.9", Timezone: "Europe/London",
		},
		{
			ID: "win11-radeon780m-1080-12c", Source: "public-spec: AMD Radeon 780M iGPU + Win11 APU laptop",
			OSName: "Windows", OSVersion: "15.0.0", Platform: "windows", UAPlatformToken: "Windows NT 10.0; Win64; x64",
			GPUVendor: "Google Inc. (AMD)", GPURenderer: "ANGLE (AMD, AMD Radeon 780M Graphics Direct3D11 vs_5_0 ps_5_0, D3D11)",
			ScreenWidth: 1920, ScreenHeight: 1200, DevicePixelRatio: 1.25, HardwareConcurrency: 12, DeviceMemoryGB: 16,
			FontAllowlist: winFonts, AcceptLang: "ja-JP,ja;q=0.9,en;q=0.8", Timezone: "Asia/Tokyo",
		},
		{
			ID: "win11-uhd770-1080-16c", Source: "public-spec: Intel UHD Graphics 770 + Win11 desktop iGPU",
			OSName: "Windows", OSVersion: "15.0.0", Platform: "windows", UAPlatformToken: "Windows NT 10.0; Win64; x64",
			GPUVendor: "Google Inc. (Intel)", GPURenderer: "ANGLE (Intel, Intel(R) UHD Graphics 770 Direct3D11 vs_5_0 ps_5_0, D3D11)",
			ScreenWidth: 1920, ScreenHeight: 1080, DevicePixelRatio: 1, HardwareConcurrency: 16, DeviceMemoryGB: 32,
			FontAllowlist: winFonts, AcceptLang: "zh-CN,zh;q=0.9,en;q=0.8", Timezone: "Asia/Shanghai",
		},
		{
			ID: "win10-rx580-1080-8c", Source: "public-spec: AMD RX 580 + Win10 mid-range desktop",
			OSName: "Windows", OSVersion: "10.0.0", Platform: "windows", UAPlatformToken: "Windows NT 10.0; Win64; x64",
			GPUVendor: "Google Inc. (AMD)", GPURenderer: "ANGLE (AMD, AMD Radeon RX 580 Series Direct3D11 vs_5_0 ps_5_0, D3D11)",
			ScreenWidth: 1920, ScreenHeight: 1080, DevicePixelRatio: 1, HardwareConcurrency: 8, DeviceMemoryGB: 16,
			FontAllowlist: winFonts, AcceptLang: "en-GB,en;q=0.9", Timezone: "Europe/London",
		},
		{
			ID: "win11-mx450-1080-8c", Source: "public-spec: NVIDIA GeForce MX450 + Win11 thin laptop",
			OSName: "Windows", OSVersion: "15.0.0", Platform: "windows", UAPlatformToken: "Windows NT 10.0; Win64; x64",
			GPUVendor: "Google Inc. (NVIDIA)", GPURenderer: "ANGLE (NVIDIA, NVIDIA GeForce MX450 Direct3D11 vs_5_0 ps_5_0, D3D11)",
			ScreenWidth: 1920, ScreenHeight: 1080, DevicePixelRatio: 1.25, HardwareConcurrency: 8, DeviceMemoryGB: 16,
			FontAllowlist: winFonts, AcceptLang: "zh-CN,zh;q=0.9,en;q=0.8", Timezone: "Asia/Shanghai",
		},
		{
			ID: "mac-m1-air-13-8c", Source: "public-spec: Apple M1 MacBook Air 13-inch display class",
			OSName: "macOS", OSVersion: "14.5.0", Platform: "macos", UAPlatformToken: "Macintosh; Intel Mac OS X 10_15_7",
			GPUVendor: "Google Inc. (Apple)", GPURenderer: "ANGLE (Apple, ANGLE Metal Renderer: Apple M1, Unspecified Version)",
			ScreenWidth: 1440, ScreenHeight: 900, DevicePixelRatio: 2, HardwareConcurrency: 8, DeviceMemoryGB: 8,
			FontAllowlist: macFonts, AcceptLang: "en-US,en;q=0.9", Timezone: "America/Los_Angeles",
		},
		{
			ID: "mac-m2-pro-14-12c", Source: "public-spec: Apple M2 Pro MacBook Pro 14-inch class",
			OSName: "macOS", OSVersion: "14.5.0", Platform: "macos", UAPlatformToken: "Macintosh; Intel Mac OS X 10_15_7",
			GPUVendor: "Google Inc. (Apple)", GPURenderer: "ANGLE (Apple, ANGLE Metal Renderer: Apple M2 Pro, Unspecified Version)",
			ScreenWidth: 1512, ScreenHeight: 982, DevicePixelRatio: 2, HardwareConcurrency: 12, DeviceMemoryGB: 16,
			FontAllowlist: macFonts, AcceptLang: "zh-CN,zh;q=0.9,en;q=0.8", Timezone: "Asia/Shanghai",
		},
		{
			ID: "mac-m3-air-15-8c", Source: "public-spec: Apple M3 MacBook Air 15-inch class",
			OSName: "macOS", OSVersion: "15.0.0", Platform: "macos", UAPlatformToken: "Macintosh; Intel Mac OS X 10_15_7",
			GPUVendor: "Google Inc. (Apple)", GPURenderer: "ANGLE (Apple, ANGLE Metal Renderer: Apple M3, Unspecified Version)",
			ScreenWidth: 1680, ScreenHeight: 1050, DevicePixelRatio: 2, HardwareConcurrency: 8, DeviceMemoryGB: 16,
			FontAllowlist: macFonts, AcceptLang: "en-US,en;q=0.9", Timezone: "America/New_York",
		},
		{
			ID: "mac-intel-iris-13-8c", Source: "public-spec: Intel Iris Plus legacy MacBook Pro 13 class",
			OSName: "macOS", OSVersion: "13.6.0", Platform: "macos", UAPlatformToken: "Macintosh; Intel Mac OS X 10_15_7",
			GPUVendor: "Google Inc. (Intel)", GPURenderer: "ANGLE (Intel, ANGLE Metal Renderer: Intel(R) Iris(TM) Plus Graphics, Unspecified Version)",
			ScreenWidth: 1440, ScreenHeight: 900, DevicePixelRatio: 2, HardwareConcurrency: 8, DeviceMemoryGB: 16,
			FontAllowlist: macFonts, AcceptLang: "en-US,en;q=0.9", Timezone: "Europe/Berlin",
		},
	}
}
