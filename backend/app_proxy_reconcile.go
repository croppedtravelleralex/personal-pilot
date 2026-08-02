package backend

import (
	"strings"
	"time"

	"personal-pilot/backend/internal/browser"
	"personal-pilot/backend/internal/events"
	"personal-pilot/backend/internal/proxy"
)

func (a *App) noteProxyHealthObserved(profileID string, result ProxyIPHealthResult) {
	if a == nil || !result.Ok {
		return
	}
	ttl := time.Duration(browser.StickySessionTTLMinutes(a.getProfileSnapshot(profileID))) * time.Minute
	if a.stickySessions != nil {
		a.stickySessions.Bind(profileID, result.ProxyId, result.IP, ttl)
		if a.ctx != nil {
			events.EmitNetworkProxyStickySessionBound(a.ctx, events.NetworkProxyStickySessionBoundPayload{
				ProfileId: profileID,
				Status:    "bound",
				Message:   "proxy=" + result.ProxyId + " exitIP=" + result.IP,
			})
		}
	}
	a.verifyStreaksMu.Lock()
	if a.verifyStreaks == nil {
		a.verifyStreaks = make(map[string]*proxy.VerifyV2Streak)
	}
	streak := a.verifyStreaks[result.ProxyId]
	if streak == nil {
		streak = &proxy.VerifyV2Streak{RequiredPasses: 3}
		a.verifyStreaks[result.ProxyId] = streak
	}
	streak.Record(proxy.VerifyV2Sample{OK: result.Ok, LatencyMs: result.LatencyMs, ExitIP: result.IP, At: time.Now()})
	a.verifyStreaksMu.Unlock()
}

func (a *App) reconcileRunningProfile(profileID, proxyID string, result ProxyIPHealthResult) {
	if a == nil || a.browserMgr == nil {
		return
	}
	proxies := a.getLatestProxies()
	in := proxy.ReconcileInput{
		Binding:   proxy.RoutingBinding{ProfileID: profileID, ProxyID: proxyID},
		HealthOK:  result.Ok,
		LatencyMs: result.LatencyMs,
		ExitIP:    result.IP,
	}
	reconcile := proxy.ReconcileProfileRouting(in, proxies)
	if !reconcile.SwitchRequired || reconcile.NewProxyID == "" {
		return
	}
	a.browserMgr.Mutex.Lock()
	profile := a.browserMgr.Profiles[profileID]
	if profile != nil && strings.TrimSpace(reconcile.NewProxyID) != "" {
		if p, ok := a.browserMgr.GetProxyByID(reconcile.NewProxyID); ok {
			browser.BindProfileToProxy(profile, p, true)
		}
	}
	a.browserMgr.Mutex.Unlock()
	if a.ctx != nil {
		a.emit(events.EventNetworkExitIpChanged, map[string]interface{}{
			"profileId": profileID,
			"message":   proxy.FormatReconcileNotice(reconcile),
		})
	}
}
