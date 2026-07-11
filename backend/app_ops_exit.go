package backend

import (
	"context"
	"fmt"
	"io"
	"net/http"
	"strings"
	"time"

	"personal-pilot/backend/internal/logger"
	"personal-pilot/backend/internal/transport"
)

// OpsExitMatchResult compares browser proxy exit IP vs Graph/ops egress IP.
type OpsExitMatchResult struct {
	ProfileID  string `json:"profileId"`
	BrowserIP  string `json:"browserIp"`
	OpsIP      string `json:"opsIp"`
	Matched    bool   `json:"matched"`
	Status     string `json:"status"` // matched | mismatch | skipped | error
	Detail     string `json:"detail,omitempty"`
}

// AssertSameExitIP checks browser proxy health IP against an ops client fetch (docs/50 A3).
func (a *App) AssertSameExitIP(profileID string) OpsExitMatchResult {
	out := OpsExitMatchResult{ProfileID: profileID, Status: "skipped"}
	profile := a.getProfileSnapshot(profileID)
	if profile == nil {
		out.Status = "error"
		out.Detail = "profile not found"
		return out
	}
	if strings.TrimSpace(profile.ProxyId) == "" {
		out.Status = "skipped"
		out.Detail = "no proxy bound"
		return out
	}
	health := a.BrowserProxyCheckIPHealth(profile.ProxyId)
	out.BrowserIP = strings.TrimSpace(health.IP)
	client, _, err := a.httpClientForProfile(profileID, 20*time.Second)
	if err != nil {
		out.Status = "error"
		out.Detail = err.Error()
		return out
	}
	opsIP, err := fetchPublicIP(client)
	if err != nil {
		out.Status = "error"
		out.Detail = err.Error()
		return out
	}
	out.OpsIP = opsIP
	if out.BrowserIP == "" || out.OpsIP == "" {
		out.Status = "skipped"
		out.Detail = "missing ip observation"
		return out
	}
	out.Matched = out.BrowserIP == out.OpsIP
	if out.Matched {
		out.Status = "matched"
	} else {
		out.Status = "mismatch"
		out.Detail = fmt.Sprintf("browser=%s ops=%s", out.BrowserIP, out.OpsIP)
	}
	return out
}

func fetchPublicIP(client *http.Client) (string, error) {
	if client == nil {
		client = &http.Client{Timeout: 15 * time.Second}
	}
	ctx, cancel := context.WithTimeout(context.Background(), 15*time.Second)
	defer cancel()
	req, err := http.NewRequestWithContext(ctx, http.MethodGet, "https://api.ipify.org", nil)
	if err != nil {
		return "", err
	}
	req.Header.Set("User-Agent", transport.ProductUserAgent)
	resp, err := client.Do(req)
	if err != nil {
		return "", err
	}
	defer resp.Body.Close()
	body, err := io.ReadAll(io.LimitReader(resp.Body, 64))
	if err != nil {
		return "", err
	}
	return strings.TrimSpace(string(body)), nil
}

// runVerifyV2Bootstrap samples proxy health 3 times after bind (docs/49 C3).
func (a *App) runVerifyV2Bootstrap(proxyID string) {
	proxyID = strings.TrimSpace(proxyID)
	if a == nil || proxyID == "" {
		return
	}
	go func() {
		for i := 0; i < 3; i++ {
			_ = a.BrowserProxyCheckIPHealth(proxyID)
			time.Sleep(2 * time.Second)
		}
		logger.New("Proxy").Info("verify_v2 bootstrap samples completed",
			logger.F("proxy_id", proxyID),
			logger.F("samples", 3),
		)
	}()
}
