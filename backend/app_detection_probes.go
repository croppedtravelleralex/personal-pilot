package backend

import (
	"context"
	"strings"
	"time"

	"personal-pilot/backend/internal/behavior"
	"personal-pilot/backend/internal/browser"
	"personal-pilot/backend/internal/detection"
	"personal-pilot/backend/internal/proxy"
)

const webrtcHardeningFlag = "--webrtc-ip-handling-policy=disable_non_proxied_udp"

// LiveDetectionSignals are probe inputs gathered from proxy health, DNS, and CDP snapshot.
type LiveDetectionSignals struct {
	ExitIP           string `json:"exitIP,omitempty"`
	DNSObserved      bool   `json:"dnsObserved"`
	DNSStatus        string `json:"dnsStatus,omitempty"`
	DNSConsistent    bool   `json:"dnsConsistent"`
	DNSLeakSuspect   bool   `json:"dnsLeakSuspect"`
	WebdriverHidden  bool   `json:"webdriverHidden"`
	WebrtcClean      bool   `json:"webrtcClean"`
	CanvasConsistent bool   `json:"canvasConsistent"`
	VerifyV2Passed   bool   `json:"verifyV2Passed"`
	ProxyQuality     string `json:"proxyQuality"`
	ProxyReason      string `json:"proxyReason,omitempty"`
}

func (a *App) collectLiveDetectionSignals(profileID string, fp *browser.FingerprintSnapshot) LiveDetectionSignals {
	out := LiveDetectionSignals{
		DNSStatus:        proxy.DNSProbeStatusUnavailable,
		DNSConsistent:    false,
		WebrtcClean:      true,
		WebdriverHidden:  true,
		CanvasConsistent: true,
		VerifyV2Passed:   true,
	}
	profile := a.getProfileSnapshot(profileID)
	if profile != nil {
		quality, reason := a.workbenchProxyQuality(profile)
		out.ProxyQuality = quality
		out.ProxyReason = reason
		out.WebrtcClean = profileLaunchArgsContain(profile, webrtcHardeningFlag)
		if profile.DebugReady && profile.DebugPort > 0 {
			exitIP := out.ExitIP
			if exitIP == "" && strings.TrimSpace(profile.ProxyId) != "" {
				// Prefer cached exit IP; avoid live health probe on every signal collection.
				if ip := a.lookupProxyStoredExitIP(profile.ProxyId); ip != "" {
					exitIP = ip
				}
			}
			if probe, err := behavior.ProbeWebRTCFromDebugPort(profile.DebugPort, exitIP); err == nil {
				out.WebrtcClean = !probe.LeakSuspect
			}
		}
	}
	if fp != nil {
		out.WebdriverHidden = !fp.Webdriver
		out.CanvasConsistent = strings.TrimSpace(fp.CanvasHash) != ""
	}
	proxyID := ""
	if profile != nil {
		proxyID = strings.TrimSpace(profile.ProxyId)
	}
	if proxyID == "" || proxyID == "__direct__" {
		// Direct / no-proxy: DNS consistency is not applicable, but do not
		// invent a WebRTC leak when the live probe already looks clean.
		out.DNSConsistent = false
		out.DNSStatus = proxy.DNSProbeStatusUnavailable
		return out
	}
	// Prefer cached exit IP for DNS classification.
	if ip := a.lookupProxyStoredExitIP(proxyID); ip != "" {
		out.ExitIP = ip
	} else {
		health := a.BrowserProxyCheckIPHealth(proxyID)
		if !health.Ok || strings.TrimSpace(health.IP) == "" {
			out.DNSConsistent = false
			return out
		}
		out.ExitIP = health.IP
	}
	// Policy-applied DNS routing: Chromium host-resolver MAP * ~NOTFOUND + local proxy
	// bridge means browser DNS is forced off the system resolver. Treat as consistent
	// when exit IP is known and WebRTC is clean. This is policy evidence, not DNS-token proof.
	if out.ExitIP != "" && profileLaunchArgsContain(profile, "--host-resolver-rules") && out.WebrtcClean {
		out.DNSStatus = proxy.DNSProbeStatusConsistent
		out.DNSObserved = true
		out.DNSConsistent = true
		out.DNSLeakSuspect = false
	} else {
		ctx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
		defer cancel()
		dns := proxy.ProbeDNSConsistency(ctx, proxyID, out.ExitIP, "api.ipify.org")
		applyDNSProbeResult(&out, dns)
	}
	a.verifyStreaksMu.RLock()
	if a.verifyStreaks != nil {
		if streak := a.verifyStreaks[proxyID]; streak != nil {
			passed, _, _ := streak.Evaluate()
			out.VerifyV2Passed = passed
		}
	}
	a.verifyStreaksMu.RUnlock()
	return out
}

func applyDNSProbeResult(out *LiveDetectionSignals, dns proxy.LeakProbeResult) {
	if out == nil {
		return
	}
	out.DNSStatus = dns.DNSStatus
	out.DNSObserved = dns.DNSStatus == proxy.DNSProbeStatusConsistent || dns.DNSStatus == proxy.DNSProbeStatusSuspect
	out.DNSConsistent = out.DNSObserved && dns.DNSConsistent
	out.DNSLeakSuspect = out.DNSObserved && dns.DNSLeakSuspect
}

func (a *App) liveAutoScoreInput(profileID string, fp *browser.FingerprintSnapshot, trust int) detection.AutoScoreInput {
	signals := a.collectLiveDetectionSignals(profileID, fp)
	scoreTrust := trust
	if !signals.VerifyV2Passed && signals.ExitIP != "" {
		scoreTrust -= 10
	}
	if signals.ProxyQuality == "risk" {
		scoreTrust -= 15
	}
	return detection.AutoScoreInput{
		SiteID:           "live-profile-gate",
		WebdriverHidden:  signals.WebdriverHidden,
		WebrtcClean:      signals.WebrtcClean && !signals.DNSLeakSuspect,
		DNSObserved:      signals.DNSObserved,
		DNSConsistent:    signals.DNSConsistent,
		CanvasConsistent: signals.CanvasConsistent,
		TrustScore:       scoreTrust,
	}
}

func profileLaunchArgsContain(profile *BrowserProfile, needle string) bool {
	if profile == nil || needle == "" {
		return false
	}
	prefix := strings.Split(needle, "=")[0]
	for _, arg := range append(append([]string{}, profile.FingerprintArgs...), profile.LaunchArgs...) {
		arg = strings.TrimSpace(arg)
		if arg == needle || strings.HasPrefix(arg, prefix+"=") {
			return true
		}
	}
	fp, launch := browser.MaterializeRuntimeArgs(profile, nil, nil)
	for _, arg := range append(append([]string{}, fp...), launch...) {
		if strings.TrimSpace(arg) == needle || strings.HasPrefix(strings.TrimSpace(arg), prefix+"=") {
			return true
		}
	}
	return false
}
