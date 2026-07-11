package backend

import (
	"context"
	"strings"
	"time"

	"personal-pilot/backend/internal/browser"
	"personal-pilot/backend/internal/events"
	"personal-pilot/backend/internal/logger"
	"personal-pilot/backend/internal/proxy"
)

func (a *App) startProxyIPMonitor(ctx context.Context) {
	if a == nil {
		return
	}
	a.proxyIPMonitor = browser.NewProxyIPMonitor(60*time.Second, func(profileID, proxyID string) {
		if strings.TrimSpace(proxyID) == "" {
			return
		}
		result := a.BrowserProxyCheckIPHealth(proxyID)
		if !result.Ok || strings.TrimSpace(result.IP) == "" {
			return
		}
		a.noteProxyHealthObserved(profileID, result)
		oldIP, changed := a.proxyIPMonitor.NoteExitIP(profileID+":"+proxyID, result.IP)
		if changed && a.ctx != nil {
			events.EmitProxyQualityNodeRotated(a.ctx, events.ProxyQualityNodeRotatedPayload{
				ProxyId: proxyID,
				OldIP:   oldIP,
				NewIP:   result.IP,
			})
			a.emit(events.EventNetworkExitIpChanged, map[string]interface{}{
				"profileId": profileID,
				"proxyId":   proxyID,
				"oldIP":     oldIP,
				"newIP":     result.IP,
				"message":   "running instance exit IP drift detected",
			})
			a.runProxyLeakProbe(profileID, proxyID, result.IP)
		}
		a.reconcileRunningProfile(profileID, proxyID, result)
	}, nil)
	a.proxyIPMonitor.Start(ctx, a.listRunningProfileProxies)
}

func (a *App) listRunningProfileProxies() []browser.RunningProfileProxy {
	if a == nil || a.browserMgr == nil {
		return nil
	}
	a.browserMgr.Mutex.Lock()
	defer a.browserMgr.Mutex.Unlock()
	out := make([]browser.RunningProfileProxy, 0, len(a.browserMgr.Profiles))
	for _, profile := range a.browserMgr.Profiles {
		if profile == nil || !profile.Running || !profile.DebugReady {
			continue
		}
		proxyID := strings.TrimSpace(profile.ProxyId)
		if proxyID == "" {
			continue
		}
		out = append(out, browser.RunningProfileProxy{
			ProfileID: profile.ProfileId,
			ProxyID:   proxyID,
		})
	}
	return out
}

func (a *App) runProxyLeakProbe(profileID, proxyID, exitIP string) {
	if a == nil || a.ctx == nil {
		return
	}
	dns := proxy.ProbeDNSConsistency(a.ctx, proxyID, exitIP, "api.ipify.org")
	if dns.DNSLeakSuspect {
		events.EmitRiskDnsLeak(a.ctx, events.RiskDnsLeakPayload{
			ProfileId:     profileID,
			LeakedDomains: dns.DNSResolvedIPs,
		})
		logger.New("Proxy").Warn("DNS leak suspect",
			logger.F("profile_id", profileID),
			logger.F("proxy_id", proxyID),
			logger.F("exit_ip", exitIP),
			logger.F("resolved", dns.DNSResolvedIPs),
		)
	}
}

func (a *App) stopProxyIPMonitor() {
	if a != nil && a.proxyIPMonitor != nil {
		a.proxyIPMonitor.Stop()
		a.proxyIPMonitor = nil
	}
}
