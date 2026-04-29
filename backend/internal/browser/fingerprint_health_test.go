package browser

import (
	"testing"
	"time"
)

func TestNewFingerprintHealthProfileGood(t *testing.T) {
	fingerprint := healthyFingerprintSnapshot()
	profile := NewFingerprintHealthProfile("profile-1", []string{
		"--fingerprint-brand=Chrome",
		"--fingerprint-platform=windows",
		"--fingerprint-hardware-concurrency=8",
		"--timezone=Asia/Shanghai",
		"--accept-lang=zh-CN,zh;q=0.9",
		"--force-device-scale-factor=1",
		"--window-size=1920,1080",
	}, fingerprint, time.Date(2026, 4, 29, 12, 0, 0, 0, time.FixedZone("CST", 8*60*60)))

	if profile.Source != FingerprintHealthSourceLocalCDP {
		t.Fatalf("Source = %q, want %q", profile.Source, FingerprintHealthSourceLocalCDP)
	}
	if profile.CapturedAt == "" {
		t.Fatal("CapturedAt should be set")
	}
	if profile.Fingerprint != fingerprint {
		t.Fatal("Fingerprint should be preserved")
	}
	if profile.Level != "good" || profile.Score < 90 {
		t.Fatalf("health = %s/%d, want good >= 90; checks=%+v", profile.Level, profile.Score, profile.Checks)
	}
}

func TestNewFingerprintHealthProfileWarning(t *testing.T) {
	fingerprint := healthyFingerprintSnapshot()
	fingerprint.Timezone = ""
	fingerprint.WebGLVendor = ""
	fingerprint.WebGLRenderer = ""

	profile := NewFingerprintHealthProfile("profile-1", nil, fingerprint, time.Now())
	if profile.Level != "warning" {
		t.Fatalf("health level = %q score=%d, want warning; checks=%+v", profile.Level, profile.Score, profile.Checks)
	}
	if profile.Score < 70 || profile.Score >= 90 {
		t.Fatalf("score = %d, want warning band [70,90)", profile.Score)
	}
}

func TestNewFingerprintHealthProfileRisk(t *testing.T) {
	fingerprint := healthyFingerprintSnapshot()
	fingerprint.CanvasHash = ""
	fingerprint.FontHash = "error"
	fingerprint.ScreenWidth = 0
	fingerprint.ScreenHeight = 0

	profile := NewFingerprintHealthProfile("profile-1", []string{
		"--fingerprint-platform=mac",
		"--fingerprint-hardware-concurrency=16",
		"--timezone=America/New_York",
	}, fingerprint, time.Now())

	if profile.Level != "risk" {
		t.Fatalf("health level = %q score=%d, want risk; checks=%+v", profile.Level, profile.Score, profile.Checks)
	}
	if profile.Score >= 70 {
		t.Fatalf("score = %d, want < 70", profile.Score)
	}
}

func TestFingerprintHealthLevelThresholds(t *testing.T) {
	tests := []struct {
		score int
		want  string
	}{
		{100, "good"},
		{90, "good"},
		{89, "warning"},
		{70, "warning"},
		{69, "risk"},
		{0, "risk"},
	}

	for _, tt := range tests {
		if got := fingerprintHealthLevel(tt.score); got != tt.want {
			t.Fatalf("fingerprintHealthLevel(%d) = %q, want %q", tt.score, got, tt.want)
		}
	}
}

func healthyFingerprintSnapshot() *FingerprintSnapshot {
	return &FingerprintSnapshot{
		UserAgent:           "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Chrome/130.0.0.0 Safari/537.36",
		Platform:            "Win32",
		HardwareConcurrency: 8,
		DeviceMemory:        8,
		ColorDepth:          24,
		PixelDepth:          24,
		ScreenWidth:         1920,
		ScreenHeight:        1080,
		AvailWidth:          1920,
		AvailHeight:         1040,
		DevicePixelRatio:    1,
		MaxTouchPoints:      0,
		Vendor:              "Google Inc.",
		Timezone:            "Asia/Shanghai",
		Language:            "zh-CN",
		Languages:           []string{"zh-CN", "zh"},
		CanvasHash:          "abc123",
		WebGLVendor:         "Google Inc.",
		WebGLRenderer:       "ANGLE",
		FontHash:            "10.00,20.00",
	}
}
