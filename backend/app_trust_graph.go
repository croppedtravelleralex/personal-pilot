package backend

import (
	"context"
	"encoding/json"
	"fmt"
	"net/http"
	"os"
	"strings"
	"time"

	"personal-pilot/backend/internal/browser"
	"personal-pilot/backend/internal/graphapi"
	"personal-pilot/backend/internal/logger"
	"personal-pilot/backend/internal/proxy"
	"personal-pilot/backend/internal/transport"
	"personal-pilot/backend/internal/transport/impersonate"
	"personal-pilot/backend/internal/trust"
)

// GraphAPIMailList uses trust-inherited Microsoft token to list messages (browser bypass).
func (a *App) GraphAPIMailList(profileID string, top int) (map[string]interface{}, error) {
	bundle, err := a.ProfileTrustBundleGet(profileID, false)
	if err != nil {
		return nil, fmt.Errorf("trust bundle required: %w", err)
	}
	if bundle.Provider != trust.ProviderMicrosoft {
		return nil, fmt.Errorf("microsoft trust bundle required")
	}
	client, err := a.graphClientForProfile(profileID)
	if err != nil {
		return nil, err
	}
	ctx, cancel := context.WithTimeout(context.Background(), 45*time.Second)
	defer cancel()
	token := bundle.AccessToken
	if !bundle.ValidAccessToken(time.Now()) {
		if !bundle.HasRefreshToken() {
			return nil, fmt.Errorf("access token expired and no refresh_token")
		}
		refreshed, err := client.RefreshAccessToken(ctx, bundle.RefreshToken)
		if err != nil {
			return nil, err
		}
		refreshed.ProfileID = profileID
		refreshed.Provider = trust.ProviderMicrosoft
		refreshed.ProxyID = bundle.ProxyID
		_ = a.ProfileTrustBundleSave(profileID, refreshed)
		token = refreshed.AccessToken
	}
	data, err := client.ListMessages(ctx, token, top)
	if err != nil {
		_ = a.AsymmetricRecordChallenge(profileID, "graph.microsoft.com", "api_error")
		return nil, err
	}
	return map[string]interface{}{
		"source": "microsoft_graph_api",
		"top":    top,
		"data":   data,
		"note":   "API path bypasses browser login surface; keep token refresh on schedule",
	}, nil
}

// ProfileTrustBundleImportFromJSON imports tokens extracted from manual bootstrap tools.
func (a *App) ProfileTrustBundleImportFromJSON(profileID, jsonPayload string) (*trust.Bundle, error) {
	bundle, err := trust.ParseBundleJSON(jsonPayload)
	if err != nil {
		return nil, err
	}
	bundle.ProfileID = profileID
	if bundle.Provider == "" {
		bundle.Provider = trust.ProviderMicrosoft
	}
	if err := a.ProfileTrustBundleSave(profileID, bundle); err != nil {
		return nil, err
	}
	redacted := bundle.RedactedCopy()
	return &redacted, nil
}

func (a *App) httpClientForProfile(profileID string, timeout time.Duration) (*http.Client, transport.EgressIdentity, error) {
	if timeout <= 0 {
		timeout = 30 * time.Second
	}
	profile := a.getProfileSnapshot(profileID)
	proxies := a.getLatestProxies()
	src := ""
	proxyID := ""
	ua := ""
	acceptLang := ""
	if profile != nil {
		src = profile.ProxyConfig
		proxyID = profile.ProxyId
		fp, launch := browser.MaterializeRuntimeArgs(profile, nil, nil)
		all := append(append([]string{}, fp...), launch...)
		for _, arg := range all {
			if strings.HasPrefix(arg, "--user-agent=") {
				ua = strings.TrimPrefix(arg, "--user-agent=")
			}
			if strings.HasPrefix(arg, "--accept-lang=") {
				acceptLang = strings.TrimPrefix(arg, "--accept-lang=")
			}
		}
	}
	src = proxy.ResolveProxyConfig(src, proxies, proxyID)
	identity := transport.EgressFromUserAgent(ua, acceptLang)

	dialer, err := proxy.ContextDialerForProxy(src, proxyID, proxies, a.bridgeManagers())
	if err != nil {
		client, buildErr := proxy.BuildHTTPClient(src, proxyID, proxies, a.bridgeManagers(), timeout)
		if buildErr != nil {
			return nil, identity, fmt.Errorf("profile egress proxy unavailable: %w", buildErr)
		}
		logger.New("Graph").Warn("profile egress using non-impersonating HTTP proxy client",
			logger.F("profile_id", profileID),
			logger.F("reason", err.Error()),
		)
		return client, identity, nil
	}
	if dialer == nil {
		dialer = impersonate.DirectDialer(timeout)
	}
	return impersonate.NewClient(dialer, identity, timeout), identity, nil
}

func (a *App) graphClientForProfile(profileID string) (*graphapi.Client, error) {
	client := &graphapi.Client{}
	if id := strings.TrimSpace(os.Getenv("PERSONAL_PILOT_MS_CLIENT_ID")); id != "" {
		client.ClientID = id
	}
	if sec := strings.TrimSpace(os.Getenv("PERSONAL_PILOT_MS_CLIENT_SECRET")); sec != "" {
		client.ClientSecret = sec
	}
	httpClient, identity, err := a.httpClientForProfile(profileID, 45*time.Second)
	if err != nil {
		return nil, err
	}
	client.HTTP = httpClient
	client.UserAgent = identity.FullUA
	return client, nil
}

// ProfileRotateFingerprintSeed rotates humanize seed after challenge feedback.
func (a *App) ProfileRotateFingerprintSeed(profileID string) (string, error) {
	if a == nil || a.browserMgr == nil {
		return "", fmt.Errorf("browser manager unavailable")
	}
	a.browserMgr.Mutex.Lock()
	defer a.browserMgr.Mutex.Unlock()
	profile := a.browserMgr.Profiles[profileID]
	if profile == nil {
		return "", fmt.Errorf("profile not found")
	}
	seed := generateUUID()
	profile.HumanizeSeed = seed
	profile.PersonaID = ""
	_, _ = browser.MaterializeRuntimeArgs(profile, nil, nil)
	if err := a.browserMgr.SaveProfiles(); err != nil {
		return "", err
	}
	meta := map[string]string{"profileId": profileID, "newSeed": seed, "personaId": profile.PersonaID}
	raw, _ := json.Marshal(meta)
	_, _ = a.db.GetConn().Exec(`
		INSERT INTO asymmetric_challenges (id, profile_id, site, challenge_type, payload, created_at)
		VALUES (?, ?, ?, ?, ?, ?)`,
		"rot-"+generateUUID(), profileID, "internal", "fingerprint_seed_rotated", string(raw), time.Now().UTC().Format(time.RFC3339),
	)
	return seed, nil
}
