package proxy

import (
	"context"
	"encoding/json"
	"fmt"
	"net"
	"net/http"
	"strings"
	"time"
)

const (
	DNSProbeStatusUnavailable  = "unavailable"
	DNSProbeStatusInconclusive = "inconclusive"
	DNSProbeStatusConsistent   = "consistent"
	DNSProbeStatusSuspect      = "suspect"
)

// LeakProbeResult summarizes DNS/WebRTC leak checks for a proxy exit.
type LeakProbeResult struct {
	ProxyID        string   `json:"proxyId"`
	ExitIP         string   `json:"exitIP"`
	DNSResolvedIPs []string `json:"dnsResolvedIPs"`
	DNSStatus      string   `json:"dnsStatus"`
	DNSConsistent  bool     `json:"dnsConsistent"`
	DNSLeakSuspect bool     `json:"dnsLeakSuspect"`
	Message        string   `json:"message"`
	CheckedAt      string   `json:"checkedAt"`
}

// ProbeDNSConsistency records a system-resolver observation for diagnostics.
// A CDN A/AAAA record cannot prove whether browser DNS followed the proxy, so this
// probe stays inconclusive until a proxy-aware or authoritative token probe exists.
func ProbeDNSConsistency(ctx context.Context, proxyID, exitIP, hostname string) LeakProbeResult {
	result := LeakProbeResult{
		ProxyID:   strings.TrimSpace(proxyID),
		ExitIP:    strings.TrimSpace(exitIP),
		DNSStatus: DNSProbeStatusUnavailable,
		CheckedAt: time.Now().Format(time.RFC3339),
	}
	host := strings.TrimSpace(hostname)
	if host == "" {
		host = "api.ipify.org"
	}
	if result.ExitIP == "" {
		result.Message = "exit IP unknown"
		return result
	}

	lookupCtx, cancel := context.WithTimeout(ctx, 8*time.Second)
	defer cancel()
	addrs, err := net.DefaultResolver.LookupIPAddr(lookupCtx, host)
	if err != nil {
		result.Message = fmt.Sprintf("dns lookup failed: %v", err)
		return result
	}
	seen := make(map[string]struct{})
	for _, addr := range addrs {
		ip := strings.TrimSpace(addr.IP.String())
		if ip == "" {
			continue
		}
		if _, ok := seen[ip]; ok {
			continue
		}
		seen[ip] = struct{}{}
		result.DNSResolvedIPs = append(result.DNSResolvedIPs, ip)
	}
	return classifySystemDNSObservation(result)
}

func classifySystemDNSObservation(result LeakProbeResult) LeakProbeResult {
	result.DNSConsistent = false
	result.DNSLeakSuspect = false
	if len(result.DNSResolvedIPs) == 0 {
		result.DNSStatus = DNSProbeStatusUnavailable
		result.Message = "dns resolution unavailable"
		return result
	}
	result.DNSStatus = DNSProbeStatusInconclusive
	result.Message = "system resolver observation cannot prove proxy DNS routing"
	return result
}

// WebRTCLeakProbe collects ICE candidates via CDP evaluate script result.
type WebRTCLeakProbe struct {
	Candidates   []string `json:"candidates"`
	LocalIPs     []string `json:"localIPs"`
	ExitIP       string   `json:"exitIP"`
	LeakSuspect  bool     `json:"leakSuspect"`
	PrivateFound bool     `json:"privateFound"`
	Message      string   `json:"message"`
}

// AnalyzeICECandidates compares ICE candidate lines against exit and local IPs.
func AnalyzeICECandidates(exitIP string, candidates []string) WebRTCLeakProbe {
	probe := WebRTCLeakProbe{
		ExitIP:     strings.TrimSpace(exitIP),
		Candidates: append([]string(nil), candidates...),
		LocalIPs:   localInterfaceIPs(),
	}
	privatePrefixes := []string{"10.", "172.16.", "172.17.", "172.18.", "172.19.", "172.20.", "172.21.", "172.22.", "172.23.", "172.24.", "172.25.", "172.26.", "172.27.", "172.28.", "172.29.", "172.30.", "172.31.", "192.168.", "127.", "169.254."}
	for _, line := range candidates {
		lower := strings.ToLower(line)
		for _, prefix := range privatePrefixes {
			if strings.Contains(lower, prefix) {
				probe.PrivateFound = true
				break
			}
		}
		if probe.ExitIP != "" && strings.Contains(line, probe.ExitIP) {
			continue
		}
		for _, local := range probe.LocalIPs {
			if local != "" && strings.Contains(line, local) && local != probe.ExitIP {
				probe.LeakSuspect = true
			}
		}
	}
	if probe.PrivateFound && probe.ExitIP != "" {
		probe.LeakSuspect = true
	}
	switch {
	case probe.LeakSuspect:
		probe.Message = "local or private ICE candidates detected"
	case probe.PrivateFound:
		probe.Message = "private ICE candidates present"
	default:
		probe.Message = "no obvious webrtc leak in candidate sample"
	}
	return probe
}

func localInterfaceIPs() []string {
	ifaces, err := net.Interfaces()
	if err != nil {
		return nil
	}
	out := make([]string, 0, 4)
	for _, iface := range ifaces {
		addrs, err := iface.Addrs()
		if err != nil {
			continue
		}
		for _, addr := range addrs {
			var ip net.IP
			switch v := addr.(type) {
			case *net.IPNet:
				ip = v.IP
			case *net.IPAddr:
				ip = v.IP
			}
			if ip == nil || ip.IsLoopback() {
				continue
			}
			out = append(out, ip.String())
		}
	}
	return out
}

// FetchExitIPViaHTTP returns the public IP seen through the given HTTP client.
func FetchExitIPViaHTTP(ctx context.Context, client *http.Client) (string, error) {
	if client == nil {
		client = &http.Client{Timeout: 12 * time.Second}
	}
	req, err := http.NewRequestWithContext(ctx, http.MethodGet, "https://api.ipify.org?format=json", nil)
	if err != nil {
		return "", err
	}
	resp, err := client.Do(req)
	if err != nil {
		return "", err
	}
	defer resp.Body.Close()
	var payload struct {
		IP string `json:"ip"`
	}
	if err := json.NewDecoder(resp.Body).Decode(&payload); err != nil {
		return "", err
	}
	return strings.TrimSpace(payload.IP), nil
}
