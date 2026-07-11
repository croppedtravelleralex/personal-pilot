package behavior

import (
	"encoding/json"
	"fmt"
	"strings"

	"personal-pilot/backend/internal/behavior/humanize"
	"personal-pilot/backend/internal/proxy"
)

const iceGatherProbeJS = `(async function(){
  if (!window.RTCPeerConnection) return { candidates: [], error: 'no RTCPeerConnection' };
  const pc = new RTCPeerConnection({ iceServers: [] });
  const candidates = [];
  pc.createDataChannel('pp-ice-probe');
  pc.onicecandidate = (event) => {
    if (event.candidate && event.candidate.candidate) candidates.push(event.candidate.candidate);
  };
  const offer = await pc.createOffer();
  await pc.setLocalDescription(offer);
  await new Promise((resolve) => setTimeout(resolve, 900));
  pc.close();
  return { candidates };
})()`

// ProbeWebRTCFromCDP gathers ICE candidates in the active browser page.
func ProbeWebRTCFromCDP(executor *CDPExecutor) (proxy.WebRTCLeakProbe, error) {
	if executor == nil {
		return proxy.WebRTCLeakProbe{}, fmt.Errorf("cdp executor is nil")
	}
	raw, err := executor.EvaluateRaw(iceGatherProbeJS)
	if err != nil {
		return proxy.WebRTCLeakProbe{LeakSuspect: true, Message: err.Error()}, err
	}
	var payload struct {
		Candidates []string `json:"candidates"`
		Error      string   `json:"error"`
	}
	if err := json.Unmarshal(raw, &payload); err != nil {
		return proxy.WebRTCLeakProbe{LeakSuspect: true, Message: "parse failed"}, err
	}
	if payload.Error != "" {
		return proxy.WebRTCLeakProbe{Message: payload.Error}, nil
	}
	return proxy.AnalyzeICECandidates("", payload.Candidates), nil
}

// ProbeWebRTCFromCDPWithExitIP compares ICE candidates against a known exit IP.
func ProbeWebRTCFromCDPWithExitIP(executor *CDPExecutor, exitIP string) (proxy.WebRTCLeakProbe, error) {
	probe, err := ProbeWebRTCFromCDP(executor)
	if err != nil {
		return probe, err
	}
	if strings.TrimSpace(exitIP) != "" {
		probe.ExitIP = strings.TrimSpace(exitIP)
		reanalyzed := proxy.AnalyzeICECandidates(probe.ExitIP, probe.Candidates)
		reanalyzed.Candidates = probe.Candidates
		return reanalyzed, nil
	}
	return probe, nil
}

// ProbeWebRTCFromDebugPort gathers ICE candidates via CDP on a running instance.
func ProbeWebRTCFromDebugPort(debugPort int, exitIP string) (proxy.WebRTCLeakProbe, error) {
	ws, err := ConnectPageCDP(debugPort)
	if err != nil {
		return proxy.WebRTCLeakProbe{}, err
	}
	defer ws.Close()
	executor := NewCDPExecutor(ws, humanize.ConfigForLevel(humanize.LevelMedium))
	return ProbeWebRTCFromCDPWithExitIP(executor, exitIP)
}
