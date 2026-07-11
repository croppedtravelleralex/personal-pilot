package backend

import (
	"encoding/json"
	"fmt"
	"strings"
	"time"

	"personal-pilot/backend/internal/asymmetric"
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
		"allowed":  allowed,
		"inWindow": inWindow,
		"timezone": policy.Timezone,
		"feedback": feedback,
		"preferAPI": st.PreferAPI,
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
		SELECT updated_at FROM profile_trust_bundles WHERE profile_id = ?`,
		profileID, profileID, profileID)
	if err != nil {
		return 0
	}
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
	return asymmetric.ActivityEntropyInput{HourlyCounts: counts}.ShannonEntropyH()
}

func (a *App) profileUsesResidentialProxy(profileID string) bool {
	profile := a.getProfileSnapshot(profileID)
	if profile == nil || strings.TrimSpace(profile.ProxyId) == "" {
		return false
	}
	health := a.BrowserProxyCheckIPHealth(profile.ProxyId)
	return health.IsResidential && health.Ok
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
