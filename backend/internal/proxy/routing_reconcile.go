package proxy

import (
	"fmt"
	"strings"
	"time"

	"personal-pilot/backend/internal/config"
)

// RoutingBinding describes a profile's active proxy route.
type RoutingBinding struct {
	ProfileID string
	ProxyID   string
	ProxyURL  string
	Country   string
	BoundAt   time.Time
}

// ReconcileInput carries dependencies for profile routing reconciliation.
type ReconcileInput struct {
	Binding       RoutingBinding
	Proxy         config.BrowserProxy
	HealthOK      bool
	LatencyMs     int64
	ExitIP        string
	PreviousExitIP string
}

// ReconcileResult describes whether routing should switch.
type ReconcileResult struct {
	SwitchRequired bool
	Reason         string
	NewProxyID     string
	OldProxyID     string
	ExitIPChanged  bool
}

// ReconcileProfileRouting decides if a profile binding should rotate due to health/IP drift.
func ReconcileProfileRouting(in ReconcileInput, candidates []config.BrowserProxy) ReconcileResult {
	result := ReconcileResult{
		OldProxyID: strings.TrimSpace(in.Binding.ProxyID),
	}
	if in.PreviousExitIP != "" && in.ExitIP != "" && in.PreviousExitIP != in.ExitIP {
		result.ExitIPChanged = true
	}
	if !in.HealthOK {
		result.SwitchRequired = true
		result.Reason = "proxy_health_failed"
	} else if in.LatencyMs > 8000 {
		result.SwitchRequired = true
		result.Reason = "proxy_latency_high"
	} else if result.ExitIPChanged {
		result.SwitchRequired = true
		result.Reason = "exit_ip_drift"
	}
	if !result.SwitchRequired {
		return result
	}
	for _, item := range candidates {
		if strings.EqualFold(item.ProxyId, in.Binding.ProxyID) {
			continue
		}
		if item.LastTestOk && item.LastLatencyMs >= 0 && item.LastLatencyMs < 5000 {
			result.NewProxyID = item.ProxyId
			break
		}
	}
	if result.NewProxyID == "" && len(candidates) > 0 {
		result.NewProxyID = candidates[0].ProxyId
	}
	return result
}

// FormatReconcileNotice returns an operator-facing message.
func FormatReconcileNotice(result ReconcileResult) string {
	if !result.SwitchRequired {
		return "routing stable"
	}
	if result.NewProxyID == "" {
		return fmt.Sprintf("reconcile required (%s) but no alternate proxy available", result.Reason)
	}
	return fmt.Sprintf("reconcile %s: switch %s -> %s", result.Reason, result.OldProxyID, result.NewProxyID)
}
