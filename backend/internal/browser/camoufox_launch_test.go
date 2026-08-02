package browser

import "testing"

func TestFilterLaunchArgsForCamoufox(t *testing.T) {
	in := []string{
		"--remote-debugging-port=9222",
		"--fingerprint=90001",
		"--fingerprint-brand=Chrome",
		"--host-resolver-rules=MAP * ~NOTFOUND , EXCLUDE 127.0.0.1",
		"--disable-blink-features=AutomationControlled",
		"--no-first-run",
		"--user-data-dir=C:\\data\\profile",
	}
	out := FilterLaunchArgsForCamoufox(in)
	if len(out) != 2 {
		t.Fatalf("filtered len=%d want 2: %v", len(out), out)
	}
	if out[0] != "--remote-debugging-port=9222" || out[1] != "--no-first-run" {
		t.Fatalf("unexpected filtered args: %v", out)
	}
}

func TestMaterializeRuntimeArgsForCore_CamoufoxSkipsFingerprint(t *testing.T) {
	profile := &Profile{
		ProfileId: "p1",
		FingerprintArgs: []string{
			"--fingerprint=1",
			"--fingerprint-brand=Chrome",
		},
		LaunchArgs: []string{"--no-first-run"},
	}
	fp, launch := MaterializeRuntimeArgsForCore("camoufox", profile, []string{"--disable-blink-features=AutomationControlled"}, nil)
	if len(fp) != 0 {
		t.Fatalf("camoufox fingerprint args should be empty, got %v", fp)
	}
	for _, arg := range launch {
		if stringsHasPrefixFold(arg, "--fingerprint") {
			t.Fatalf("camoufox launch must not contain fingerprint arg: %s", arg)
		}
	}
}

func stringsHasPrefixFold(s, prefix string) bool {
	return len(s) >= len(prefix) && equalFoldASCII(s[:len(prefix)], prefix)
}

func equalFoldASCII(a, b string) bool {
	if len(a) != len(b) {
		return false
	}
	for i := 0; i < len(a); i++ {
		ca, cb := a[i], b[i]
		if ca >= 'A' && ca <= 'Z' {
			ca += 'a' - 'A'
		}
		if cb >= 'A' && cb <= 'Z' {
			cb += 'a' - 'A'
		}
		if ca != cb {
			return false
		}
	}
	return true
}
