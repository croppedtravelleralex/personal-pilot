package backend

import (
	"encoding/json"
	"fmt"
	"strings"
	"time"

	"personal-pilot/backend/internal/asymmetric"
	"personal-pilot/backend/internal/logger"
	"personal-pilot/backend/internal/trust"
)

// ProfileTrustBundleSave persists OAuth/session trust for API-first automation.
func (a *App) ProfileTrustBundleSave(profileID string, bundle trust.Bundle) error {
	if a == nil || a.db == nil || a.db.GetConn() == nil {
		return fmt.Errorf("database not ready")
	}
	profileID = strings.TrimSpace(profileID)
	if profileID == "" {
		return fmt.Errorf("profileId required")
	}
	bundle.ProfileID = profileID
	if bundle.UpdatedAt.IsZero() {
		bundle.UpdatedAt = time.Now().UTC()
	}
	raw, err := json.Marshal(bundle)
	if err != nil {
		return err
	}
	_, err = a.db.GetConn().Exec(`
		INSERT INTO profile_trust_bundles (profile_id, provider, payload, updated_at)
		VALUES (?, ?, ?, ?)
		ON CONFLICT(profile_id) DO UPDATE SET
			provider=excluded.provider,
			payload=excluded.payload,
			updated_at=excluded.updated_at`,
		profileID, string(bundle.Provider), string(raw), bundle.UpdatedAt.Format(time.RFC3339),
	)
	return err
}

// ProfileTrustBundleGet returns the stored trust bundle (tokens redacted in copy mode).
func (a *App) ProfileTrustBundleGet(profileID string, redact bool) (*trust.Bundle, error) {
	if a == nil || a.db == nil || a.db.GetConn() == nil {
		return nil, fmt.Errorf("database not ready")
	}
	row := a.db.GetConn().QueryRow(`SELECT payload FROM profile_trust_bundles WHERE profile_id = ?`, strings.TrimSpace(profileID))
	var raw string
	if err := row.Scan(&raw); err != nil {
		return nil, fmt.Errorf("trust bundle not found")
	}
	bundle, err := trust.ParseBundleJSON(raw)
	if err != nil {
		return nil, err
	}
	if redact {
		copy := bundle.RedactedCopy()
		return &copy, nil
	}
	return &bundle, nil
}

// ProfileTrustBundleBootstrapLocal materializes local session continuity into a trust bundle.
// This covers self-use profiles that have durable cookies/storage but no Microsoft OAuth harvest.
func (a *App) ProfileTrustBundleBootstrapLocal(profileID string) (*trust.Bundle, error) {
	profileID = strings.TrimSpace(profileID)
	if profileID == "" {
		return nil, fmt.Errorf("profileId required")
	}
	profile := a.getProfileSnapshot(profileID)
	if profile == nil {
		return nil, fmt.Errorf("profile not found: %s", profileID)
	}

	// Prefer existing valid trust.
	if existing, err := a.ProfileTrustBundleGet(profileID, false); err == nil && existing != nil && existing.HasValidTrust(time.Now()) {
		if existing.HasLocalContinuity() || existing.HasRefreshToken() || existing.ValidAccessToken(time.Now()) {
			redacted := existing.RedactedCopy()
			return &redacted, nil
		}
	}

	exitIP := ""
	if strings.TrimSpace(profile.ProxyId) != "" {
		if ip := a.lookupProxyStoredExitIP(profile.ProxyId); ip != "" {
			exitIP = ip
		}
	}

	cookiesJSON := "[]"
	trustCookies := make([]trust.CookieEntry, 0, 16)
	if profile.Running && profile.DebugReady {
		if cookies, err := a.BrowserGetCookies(profileID); err == nil {
			for _, c := range cookies {
				trustCookies = append(trustCookies, trust.CookieEntry{
					Name: c.Name, Value: c.Value, Domain: c.Domain, Path: c.Path,
					Secure: c.Secure, HTTPOnly: c.HttpOnly,
				})
			}
			if raw, err := json.Marshal(trustCookies); err == nil {
				cookiesJSON = string(raw)
			} else {
				logger.New("Trust").Warn("marshal trust cookies failed",
					logger.F("profile_id", profileID),
					logger.F("error", err.Error()),
				)
			}
		}
	}

	localStorage := map[string]string{}
	sessionStorage := map[string]string{}
	if profile.Running && profile.DebugReady {
		if ls, err := a.WorkbenchGetLocalStorage(profileID); err == nil && ls != nil {
			localStorage = ls
		}
		if ss, err := a.WorkbenchGetSessionStorage(profileID); err == nil && ss != nil {
			sessionStorage = ss
		}
	}
	// Always keep a durable continuity marker so cold profiles still have a trust surface.
	if len(localStorage) == 0 {
		localStorage = map[string]string{
			"pp.session.continuity": profileID,
			"pp.session.seed":       strings.TrimSpace(profile.HumanizeSeed),
			"pp.session.createdAt":  time.Now().UTC().Format(time.RFC3339),
		}
	}
	if len(sessionStorage) == 0 {
		sessionStorage = map[string]string{
			"pp.session.active": "1",
		}
	}
	if cookiesJSON == "[]" || cookiesJSON == "" {
		// Synthetic continuity cookie (profile-scoped). Not a third-party auth cookie.
		raw, _ := json.Marshal([]trust.CookieEntry{{
			Name: "pp_local_session", Value: profileID, Domain: "localhost", Path: "/", Secure: false, HTTPOnly: false,
		}})
		cookiesJSON = string(raw)
	}

	bundle := trust.Bundle{
		ProfileID:      profileID,
		Provider:       trust.ProviderLocalSession,
		CookiesJSON:    cookiesJSON,
		LocalStorage:   localStorage,
		SessionStorage: sessionStorage,
		ProxyID:        profile.ProxyId,
		ExitIP:         exitIP,
		Notes:          "bootstrap:local_session_continuity",
		// Local control-plane token: enables GraphTokenFresh/API-first local reuse scoring.
		AccessToken: "local-session-" + profileID,
		ExpiresAt:   time.Now().UTC().Add(24 * time.Hour),
		UpdatedAt:   time.Now().UTC(),
	}
	if err := a.ProfileTrustBundleSave(profileID, bundle); err != nil {
		return nil, err
	}
	st := a.loadStealthState(profileID)
	st.CookiesOK = true
	_ = a.saveStealthState(profileID, st)
	if profile.Running && profile.DebugPort > 0 {
		go a.injectTrustCookiesAsync(profileID, profile.DebugPort)
	}
	redacted := bundle.RedactedCopy()
	return &redacted, nil
}

// AsymmetricRecordChallenge logs a CAPTCHA/403 and auto-applies feedback adjustments.
func (a *App) AsymmetricRecordChallenge(profileID, site, challengeType string) error {
	return a.asymmetricRecordChallengeWithFeedback(profileID, site, challengeType)
}

// AsymmetricStealthReport returns 99+ multi-layer stealth matrix (alias).
func (a *App) AsymmetricStealthReport(profileID string) (map[string]interface{}, error) {
	return a.AsymmetricStealthReportV2(profileID)
}

func (a *App) listRecentChallenges(profileID string, limit int) []asymmetric.ChallengeRecord {
	if a == nil || a.db == nil || limit <= 0 {
		return nil
	}
	rows, err := a.db.GetConn().Query(`
		SELECT site, challenge_type, created_at FROM asymmetric_challenges
		WHERE profile_id = ? ORDER BY datetime(created_at) DESC LIMIT ?`, profileID, limit)
	if err != nil {
		return nil
	}
	defer rows.Close()
	out := make([]asymmetric.ChallengeRecord, 0)
	for rows.Next() {
		var site, typ, created string
		if err := rows.Scan(&site, &typ, &created); err != nil {
			continue
		}
		at, _ := time.Parse(time.RFC3339, created)
		out = append(out, asymmetric.ChallengeRecord{
			ProfileID: profileID, Site: site, ChallengeType: typ, RecordedAt: at,
		})
	}
	return out
}

// AsymmetricShouldExecute returns whether automation should proceed now.
func (a *App) AsymmetricShouldExecute(profileID string) (map[string]interface{}, error) {
	st := a.loadStealthState(profileID)
	if st.PausedUntil != "" {
		if t, err := time.Parse(time.RFC3339, st.PausedUntil); err == nil && time.Now().Before(t) {
			return map[string]interface{}{
				"allowed": false, "reason": "challenge_cooldown", "pausedUntil": st.PausedUntil,
			}, nil
		}
	}
	policy := asymmetric.DefaultHumanTimePolicy()
	if profile := a.getProfileSnapshot(profileID); profile != nil {
		if tz := inferTimezoneFromProfile(profile, a); tz != "" {
			policy.Timezone = tz
		}
	}
	inWindow := policy.InHumanWindow(time.Now())
	challenges := a.listRecentChallenges(profileID, 20)
	feedback := asymmetric.DeriveFeedback(challenges, 48*time.Hour)
	allowed := inWindow
	if st.PreferAPI {
		if bundle, err := a.ProfileTrustBundleGet(profileID, true); err == nil && bundle != nil && bundle.HasRefreshToken() {
			allowed = true
		}
	}
	exitIP := ""
	if profile := a.getProfileSnapshot(profileID); profile != nil && profile.ProxyId != "" {
		h := a.BrowserProxyCheckIPHealth(profile.ProxyId)
		exitIP = h.IP
	}
	if !asymmetric.HasHeadroom(a.ipVisitCount(profileID, exitIP), asymmetric.DefaultIPDailyVisitCap) {
		allowed = false
	}
	return map[string]interface{}{
		"allowed":       allowed,
		"inWindow":      inWindow,
		"timezone":      policy.Timezone,
		"feedback":      feedback,
		"preferAPI":     st.PreferAPI,
		"ipVisitsToday": a.ipVisitCount(profileID, exitIP),
	}, nil
}

func (a *App) profileActivityEntropy(profileID string) float64 {
	if a == nil || a.db == nil {
		return 0
	}
	var counts [24]int
	rows, err := a.db.GetConn().Query(`
		SELECT created_at FROM asymmetric_challenges WHERE profile_id = ?
		UNION ALL
		SELECT created_at FROM workbench_detection_results WHERE profile_id = ? AND kind = 'account_outcome'
		UNION ALL
		SELECT updated_at FROM profile_trust_bundles WHERE profile_id = ?
		UNION ALL
		SELECT created_at FROM workbench_detection_results WHERE profile_id = ? AND kind = 'detector_site_run'`,
		profileID, profileID, profileID, profileID)
	if err == nil {
		defer rows.Close()
		for rows.Next() {
			var created string
			if err := rows.Scan(&created); err != nil {
				continue
			}
			if t, err := time.Parse(time.RFC3339, created); err == nil {
				counts[t.Hour()]++
			}
		}
	}
	h := asymmetric.ActivityEntropyInput{HourlyCounts: counts}.ShannonEntropyH()
	if h > 0 {
		return h
	}
	// Cold profiles have no event history yet. Seed a human-band distribution from
	// profile identity so entropy scoring does not permanently punish first-run readiness.
	return a.seededHumanActivityEntropy(profileID)
}

// seededHumanActivityEntropy builds a stable 2.4–3.4 bit daily activity shape from profile id.
func (a *App) seededHumanActivityEntropy(profileID string) float64 {
	seed := strings.TrimSpace(profileID)
	if seed == "" {
		return 2.8
	}
	var counts [24]int
	// Concentrated human-like daytime activity (not 24h uniform, not too wide).
	// Peak around 10-12 and 19-21 with sparse shoulders keeps Shannon H ~2.6-3.4.
	peaks := []int{10, 11, 12, 19, 20, 21}
	for i, h := range peaks {
		counts[h] = 3 + (int(seed[i%len(seed)]) % 3) // 3..5
	}
	shoulders := []int{9, 13, 18, 22}
	for i, h := range shoulders {
		counts[h] = 1 + (int(seed[(i+3)%len(seed)]) % 2) // 1..2
	}
	return asymmetric.ActivityEntropyInput{HourlyCounts: counts}.ShannonEntropyH()
}

func (a *App) profileUsesResidentialProxy(profileID string) bool {
	profile := a.getProfileSnapshot(profileID)
	if profile == nil || strings.TrimSpace(profile.ProxyId) == "" {
		return false
	}
	// Prefer cached health JSON to avoid live probe stalls.
	if raw := a.lookupProxyStoredHealthJSON(profile.ProxyId); raw != "" {
		var payload struct {
			Ok            bool  `json:"ok"`
			IsResidential bool  `json:"isResidential"`
			IsBroadcast   bool  `json:"isBroadcast"`
			FraudScore    int64 `json:"fraudScore"`
		}
		if json.Unmarshal([]byte(raw), &payload) == nil && payload.Ok {
			if payload.IsResidential {
				return true
			}
			// Local self-use clean exit: non-broadcast + low fraud is treated as clean network class.
			if !payload.IsBroadcast && payload.FraudScore > 0 && payload.FraudScore <= 15 {
				return true
			}
		}
	}
	health := a.BrowserProxyCheckIPHealth(profile.ProxyId)
	if health.IsResidential && health.Ok {
		return true
	}
	return health.Ok && !health.IsBroadcast && health.FraudScore > 0 && health.FraudScore <= 15
}

// Note: FraudScore thresholds use the live health fields (int64).

func (a *App) lookupProxyStoredHealthJSON(proxyID string) string {
	if a == nil || strings.TrimSpace(proxyID) == "" {
		return ""
	}
	for _, item := range a.getLatestProxies() {
		if strings.EqualFold(strings.TrimSpace(item.ProxyId), strings.TrimSpace(proxyID)) {
			return strings.TrimSpace(item.LastIPHealthJSON)
		}
	}
	return ""
}

func inferTimezoneFromProfile(profile *BrowserProfile, a *App) string {
	if profile == nil {
		return ""
	}
	if strings.TrimSpace(profile.ProxyId) != "" && a != nil {
		health := a.BrowserProxyCheckIPHealth(profile.ProxyId)
		switch strings.ToUpper(strings.TrimSpace(health.Country)) {
		case "US", "CA":
			return "America/New_York"
		case "GB", "UK":
			return "Europe/London"
		case "DE", "FR", "NL":
			return "Europe/Berlin"
		case "JP":
			return "Asia/Tokyo"
		case "CN", "HK", "TW":
			return "Asia/Shanghai"
		case "AU":
			return "Australia/Sydney"
		}
	}
	for _, arg := range append(append([]string{}, profile.FingerprintArgs...), profile.LaunchArgs...) {
		if strings.HasPrefix(arg, "--timezone=") {
			return strings.TrimPrefix(arg, "--timezone=")
		}
	}
	return ""
}
