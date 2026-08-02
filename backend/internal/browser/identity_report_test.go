package browser

import (
	"testing"
	"time"
)

func TestIdentityStrengthReportIncludesExpandedDimensions(t *testing.T) {
	fp := healthyIdentityFingerprintSnapshot()
	profile := &Profile{
		ProfileId:          "profile-1",
		ProfileName:        "Identity One",
		UserDataDir:        "data/profile-1",
		FingerprintArgs:    []string{"--fingerprint-platform=windows", "--fingerprint-hardware-concurrency=8", "--timezone=Asia/Shanghai"},
		ProxyId:            "proxy-1",
		ProxyBindName:      "proxy-sticky",
		ProxyBindUpdatedAt: "2026-04-29T00:00:00Z",
		BehaviorProfileID:  "office-worker",
		HumanizeSeed:       "seed-1",
		LaunchAudit: &LaunchAuditSnapshot{
			Timestamp: "2026-04-30T00:00:00Z",
		},
		CreatedAt:   "2026-04-01T00:00:00Z",
		UpdatedAt:   "2026-04-29T00:00:00Z",
		LastStartAt: "2026-04-30T11:00:00Z",
	}

	report := NewIdentityStrengthReport(profile, fp, time.Date(2026, 4, 30, 12, 0, 0, 0, time.UTC), IdentityReportContext{})

	if report.Source != IdentityReportSourceLocalCDP {
		t.Fatalf("source = %q, want %q", report.Source, IdentityReportSourceLocalCDP)
	}
	if report.ProfileID != "profile-1" || report.Fingerprint != fp {
		t.Fatalf("bad report identity: %+v", report)
	}
	if len(report.Dimensions) < 35 {
		t.Fatalf("dimensions = %d, want >= 35", len(report.Dimensions))
	}
	if report.Score < 75 {
		t.Fatalf("score = %d level=%s, want usable identity score; summary=%v", report.Score, report.Level, report.Summary)
	}
	assertIdentityDimension(t, report, "webdriver", "pass")
	assertIdentityDimension(t, report, "canvas_hash", "pass")
	assertIdentityDimension(t, report, "webgl_extensions_hash", "pass")
	assertIdentityDimension(t, report, "audio_hash", "pass")
	assertIdentityDimension(t, report, "user_data_dir_no_traversal", "pass")
	assertIdentityDimension(t, report, "long_term_behavior_seed_stable", "pass")
	if report.Subscores.LongTermCoherence < 90 {
		t.Fatalf("longTermCoherence = %d, want strong long-term score", report.Subscores.LongTermCoherence)
	}
}

func TestIdentityStrengthReportRiskOnWebdriverAndAuditMismatch(t *testing.T) {
	fp := healthyIdentityFingerprintSnapshot()
	fp.Webdriver = true
	profile := &Profile{
		ProfileId:       "profile-risk",
		UserDataDir:     "profile-risk",
		FingerprintArgs: []string{"--fingerprint-platform=windows"},
		HumanizeSeed:    "seed-risk",
	}

	report := NewIdentityStrengthReport(profile, fp, time.Now(), IdentityReportContext{LaunchAuditError: "fingerprint args hash mismatch"})

	if report.Level != IdentityLevelRisk {
		t.Fatalf("level = %q score=%d, want risk", report.Level, report.Score)
	}
	assertIdentityDimension(t, report, "webdriver", "fail")
	assertIdentityDimension(t, report, "launch_audit_valid", "fail")
}

func TestIdentityStrengthReportRiskOnUserDataDirParentSegment(t *testing.T) {
	fp := healthyIdentityFingerprintSnapshot()
	profile := &Profile{
		ProfileId:       "profile-traversal",
		UserDataDir:     `data\profiles\..\other-profile`,
		FingerprintArgs: []string{"--fingerprint-platform=windows"},
		HumanizeSeed:    "seed-traversal",
		LaunchAudit: &LaunchAuditSnapshot{
			Timestamp: "2026-04-30T00:00:00Z",
		},
	}

	report := NewIdentityStrengthReport(profile, fp, time.Now(), IdentityReportContext{})

	if report.Level != IdentityLevelRisk {
		t.Fatalf("level = %q score=%d, want risk", report.Level, report.Score)
	}
	assertIdentityDimension(t, report, "user_data_dir_no_traversal", "fail")
}

func TestIdentityStrengthReportLongTermCoherenceWarnsOnMissingHistory(t *testing.T) {
	fp := healthyIdentityFingerprintSnapshot()
	profile := &Profile{
		ProfileId:       "profile-history-gap",
		UserDataDir:     "data/profile-history-gap",
		FingerprintArgs: []string{"--fingerprint-platform=windows"},
	}

	report := NewIdentityStrengthReport(profile, fp, time.Now(), IdentityReportContext{})

	assertIdentityDimension(t, report, "long_term_profile_timeline", "warning")
	assertIdentityDimension(t, report, "long_term_behavior_seed_stable", "fail")
	if report.Subscores.LongTermCoherence >= 80 {
		t.Fatalf("longTermCoherence = %d, want score to reflect missing long-term history", report.Subscores.LongTermCoherence)
	}
}

func assertIdentityDimension(t *testing.T, report *IdentityStrengthReport, id string, status string) {
	t.Helper()
	for _, dim := range report.Dimensions {
		if dim.ID == id {
			if dim.Status != status {
				t.Fatalf("dimension %s status = %q, want %q; dim=%+v", id, dim.Status, status, dim)
			}
			return
		}
	}
	t.Fatalf("dimension %s not found", id)
}

func healthyIdentityFingerprintSnapshot() *FingerprintSnapshot {
	fp := healthyFingerprintSnapshot()
	fp.AppVersion = "5.0 (Windows)"
	fp.AppName = "Netscape"
	fp.Product = "Gecko"
	fp.ProductSub = "20030107"
	fp.Webdriver = false
	fp.CookieEnabled = true
	fp.DoNotTrack = "unspecified"
	fp.PDFViewerEnabled = true
	fp.Online = true
	fp.TimezoneOffset = -480
	fp.IntlLocale = "zh-CN"
	fp.IntlCalendar = "gregory"
	fp.IntlNumberingSystem = "latn"
	fp.DateFormatSample = "2026"
	fp.NumberFormatSample = "$123,456.78"
	fp.UADataBrands = []string{"Chromium/130", "Google Chrome/130"}
	fp.UADataPlatform = "Windows"
	fp.UADataPlatformVer = "15.0.0"
	fp.UADataArchitecture = "x86"
	fp.UADataBitness = "64"
	fp.UADataFullVersions = []string{"Chromium/130.0.0.0"}
	fp.InnerWidth = 1280
	fp.InnerHeight = 720
	fp.OuterWidth = 1296
	fp.OuterHeight = 808
	fp.VisualViewportWidth = 1280
	fp.VisualViewportHeight = 720
	fp.VisualViewportScale = 1
	fp.PointerFine = true
	fp.HoverHover = true
	fp.PrefersColorScheme = "light"
	fp.PrefersReducedMotion = "no-preference"
	fp.NetworkEffectiveType = "4g"
	fp.NetworkDownlink = 10
	fp.NetworkRTT = 50
	fp.StorageQuota = 1000000
	fp.StorageUsage = 1000
	fp.WebGLExtensionsHash = "webgl-ext"
	fp.WebGLMaxTextureSize = 16384
	fp.WebGLMaxVertexAttribs = 16
	fp.WebGLMaxViewportDims = "32767x32767"
	fp.AudioHash = "audio"
	fp.PluginsHash = "plugins"
	fp.MimeTypesHash = "mime"
	fp.WebRTCSupported = true
	return fp
}

func TestIdentityStrengthReportUAAndCoreCoherence(t *testing.T) {
	fp := healthyIdentityFingerprintSnapshot()
	fp.UserAgent = "Mozilla/5.0 Chrome/139.0.7258.154 Safari/537.36"
	fp.UADataFullVersions = []string{"Chromium/139.0.7258.154", "Google Chrome/139.0.7258.154"}
	profile := &Profile{
		ProfileId:    "profile-ua-core",
		CoreId:       "core-fingerprint-chromium-139-0-7258-154",
		UserDataDir:  "profile-ua-core",
		HumanizeSeed: "seed",
	}
	report := NewIdentityStrengthReport(profile, fp, time.Now(), IdentityReportContext{})
	assertIdentityDimension(t, report, "ua_core_coherent", "pass")
}

func TestIdentityStrengthReportUAAndCoreMismatchFails(t *testing.T) {
	fp := healthyIdentityFingerprintSnapshot()
	fp.UserAgent = "Mozilla/5.0 Chrome/131.0.0.0 Safari/537.36"
	fp.UADataFullVersions = []string{"Chromium/131.0.0.0"}
	profile := &Profile{
		ProfileId:    "profile-ua-mismatch",
		CoreId:       "core-fingerprint-chromium-139-0-7258-154",
		UserDataDir:  "profile-ua-mismatch",
		HumanizeSeed: "seed",
	}
	report := NewIdentityStrengthReport(profile, fp, time.Now(), IdentityReportContext{})
	assertIdentityDimension(t, report, "ua_core_coherent", "fail")
}
