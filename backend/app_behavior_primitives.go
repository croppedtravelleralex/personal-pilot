package backend

import (
	"encoding/json"
	"fmt"
	"strings"

	"personal-pilot/backend/internal/behavior"
	"personal-pilot/backend/internal/events"
	"personal-pilot/backend/internal/proxy"
)

// BehaviorExecutePrimitive runs one shipped behavior primitive on a running profile.
func (a *App) BehaviorExecutePrimitive(profileID, primitiveName string, paramsJSON string) (map[string]interface{}, error) {
	profile, err := a.runningProfileForWorkbench(profileID)
	if err != nil {
		return nil, err
	}
	step := behavior.PrimitiveStep{Primitive: strings.TrimSpace(primitiveName)}
	if strings.TrimSpace(paramsJSON) != "" {
		var payload behavior.PrimitiveStep
		if err := json.Unmarshal([]byte(paramsJSON), &payload); err != nil {
			return nil, fmt.Errorf("decode params: %w", err)
		}
		payload.Primitive = step.Primitive
		step = payload
	}
	executor, err := connectCDPExecutor(profile.DebugPort)
	if err != nil {
		return nil, err
	}
	defer executor.Close()
	return executor.ExecutePrimitive(step)
}

// BehaviorExecutePrimitivePlan runs a JSON array of primitive steps sequentially.
func (a *App) BehaviorExecutePrimitivePlan(profileID, planJSON string) ([]map[string]interface{}, error) {
	profile, err := a.runningProfileForWorkbench(profileID)
	if err != nil {
		return nil, err
	}
	steps, err := behavior.ParsePrimitivePlanJSON(planJSON)
	if err != nil {
		return nil, err
	}
	executor, err := connectCDPExecutor(profile.DebugPort)
	if err != nil {
		return nil, err
	}
	defer executor.Close()
	return executor.ExecutePrimitivePlan(steps)
}

// WorkbenchProbeWebRTC runs an in-browser ICE gather probe on a running profile.
func (a *App) WorkbenchProbeWebRTC(profileID string) (map[string]interface{}, error) {
	profile, err := a.runningProfileForWorkbench(profileID)
	if err != nil {
		return nil, err
	}
	exitIP := ""
	if strings.TrimSpace(profile.ProxyId) != "" {
		health := a.BrowserProxyCheckIPHealth(profile.ProxyId)
		if health.Ok {
			exitIP = health.IP
		}
	}
	probe, err := behavior.ProbeWebRTCFromDebugPort(profile.DebugPort, exitIP)
	if err != nil {
		return nil, err
	}
	a.noteWebRTCProbeResult(profileID, probe)
	return map[string]interface{}{
		"exitIP":       probe.ExitIP,
		"leakSuspect":  probe.LeakSuspect,
		"privateFound": probe.PrivateFound,
		"message":      probe.Message,
		"candidates":   probe.Candidates,
	}, nil
}

func (a *App) noteWebRTCProbeResult(profileID string, probe proxy.WebRTCLeakProbe) {
	if a == nil || !probe.LeakSuspect || a.ctx == nil {
		return
	}
	events.EmitRiskWebrtcLeak(a.ctx, events.RiskWebrtcLeakPayload{
		ProfileId: profileID,
		PublicIP:  probe.ExitIP,
		LocalIP:   strings.Join(probe.LocalIPs, ","),
	})
}

func (a *App) probeWebRTCAfterInjection(profileID string, debugPort int) {
	if a == nil || debugPort <= 0 {
		return
	}
	exitIP := ""
	if profile := a.getProfileSnapshot(profileID); profile != nil && strings.TrimSpace(profile.ProxyId) != "" {
		health := a.BrowserProxyCheckIPHealth(profile.ProxyId)
		if health.Ok {
			exitIP = health.IP
		}
	}
	probe, err := behavior.ProbeWebRTCFromDebugPort(debugPort, exitIP)
	if err != nil {
		return
	}
	a.noteWebRTCProbeResult(profileID, probe)
}
