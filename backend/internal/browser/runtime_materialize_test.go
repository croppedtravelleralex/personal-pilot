package browser

import (
	"strings"
	"testing"
)

func TestMaterializeRuntimeArgsDropsIneffectiveFingerprintFlags(t *testing.T) {
	profile := &Profile{
		ProfileId: "p-canvas-noise",
		FingerprintArgs: []string{
			"--fingerprint-canvas-noise=true",
			"--fingerprint=seed123",
		},
	}
	fingerprintArgs, launchArgs := MaterializeRuntimeArgs(profile, nil, nil)
	joined := strings.Join(append(fingerprintArgs, launchArgs...), "\n")
	if strings.Contains(joined, "--fingerprint-canvas-noise") {
		t.Fatalf("ineffective fingerprint flag survived materialization:\n%s", joined)
	}
	if !strings.Contains(joined, "--fingerprint=seed123") {
		t.Fatalf("effective fingerprint flag missing:\n%s", joined)
	}
}

func TestMaterializeRuntimeArgsFullCoverage(t *testing.T) {
	report := FullRuntimeProjectionReport(&Profile{
		ProfileId:         "demo",
		BehaviorProfileID: "bp1",
		HumanizeSeed:      "seed",
		ProxyId:           "proxy-1",
		ProxyConfig:       "socks5://127.0.0.1:1080",
	}, nil, nil)
	if report.DeclaredTotal != 80 {
		t.Fatalf("declared=%d", report.DeclaredTotal)
	}
	if report.AppliedCount != 80 {
		t.Fatalf("applied=%d missing=%v", report.AppliedCount, report.MissingControls)
	}
}

func TestMaterializeRuntimeArgsUsesCoreVersionAsSingleTruth(t *testing.T) {
	profile := &Profile{
		ProfileId: "p139",
		CoreId:    "core-fingerprint-chromium-139-0-7258-154",
		FingerprintArgs: []string{
			"--fingerprint-brand-version=131.0.0.0",
			"--user-agent=Mozilla/5.0 Chrome/131.0.0.0 Safari/537.36",
		},
	}
	fingerprintArgs, launchArgs := MaterializeRuntimeArgs(profile, nil, nil)
	joined := strings.Join(append(fingerprintArgs, launchArgs...), "\n")
	if strings.Contains(joined, "Chrome/131") || strings.Contains(joined, "brand-version=131") {
		t.Fatalf("stale browser major survived materialization:\n%s", joined)
	}
	if !strings.Contains(joined, "--fingerprint-brand-version=139.0.7258.154") {
		t.Fatalf("resolved brand version missing:\n%s", joined)
	}
	if !strings.Contains(joined, "Chrome/139.0.7258.154") {
		t.Fatalf("resolved user agent missing:\n%s", joined)
	}
}

func TestMaterializeRuntimeArgsWithBinaryPrefersBinaryVersion(t *testing.T) {
	profile := &Profile{
		ProfileId: "binary-wins",
		CoreId:    "core-chromium-126-0-6478-271",
		Tags:      []string{"outlook-auth"},
	}
	fingerprintArgs, launchArgs := MaterializeRuntimeArgsWithBinary(
		`D:\SelfMadeTool\personal-pilot\chrome\fingerprint-chromium-139.0.7258.154\chrome.exe`,
		profile,
		nil,
		nil,
	)
	joined := strings.Join(append(fingerprintArgs, launchArgs...), "\n")
	if strings.Contains(joined, "Chrome/126") || strings.Contains(joined, "brand-version=126") {
		t.Fatalf("profile core id overrode concrete binary version:\n%s", joined)
	}
	if !strings.Contains(joined, "Chrome/139.0.7258.154") || !strings.Contains(joined, "brand-version=139.0.7258.154") {
		t.Fatalf("binary version was not materialized:\n%s", joined)
	}
}
