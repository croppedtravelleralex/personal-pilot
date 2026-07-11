package browser

import "testing"

func TestBuildRuntimeProjectionReport(t *testing.T) {
	report := BuildRuntimeProjectionReport([]string{
		"--user-agent=Mozilla/5.0",
		"--lang=zh-CN",
		"--timezone=Asia/Shanghai",
		"--fingerprint-hardware-concurrency=8",
		"--window-size=1920,1080",
		"--fingerprint=12345",
	}, []string{"--host-resolver-rules=MAP * ~NOTFOUND , EXCLUDE 127.0.0.1"})
	if report.DeclaredTotal != 80 {
		t.Fatalf("declared total = %d, want 80", report.DeclaredTotal)
	}
	if report.AppliedCount < 8 {
		t.Fatalf("applied count too low: %d", report.AppliedCount)
	}
}
