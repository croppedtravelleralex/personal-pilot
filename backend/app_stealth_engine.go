package backend

import (
	"context"
	"encoding/json"
	"fmt"
	"strings"
	"time"

	"personal-pilot/backend/internal/asymmetric"
	"personal-pilot/backend/internal/behavior"
	"personal-pilot/backend/internal/browser"
	"personal-pilot/backend/internal/detection"
	"personal-pilot/backend/internal/logger"
	"personal-pilot/backend/internal/trust"
)

type profileStealthState struct {
	PausedUntil string
	PreferAPI   bool
	CreepJSScore float64
	LastProbeAt string
	CookiesOK   bool
}

func (a *App) loadStealthState(profileID string) profileStealthState {
	st := profileStealthState{}
	if a == nil || a.db == nil {
		return st
	}
	row := a.db.GetConn().QueryRow(`
		SELECT paused_until, prefer_api, creepjs_score, last_probe_at, cookies_ok
		FROM profile_stealth_state WHERE profile_id = ?`, profileID)
	var prefer, cookies int
	_ = row.Scan(&st.PausedUntil, &prefer, &st.CreepJSScore, &st.LastProbeAt, &cookies)
	st.PreferAPI = prefer == 1
	st.CookiesOK = cookies == 1
	return st
}

func (a *App) saveStealthState(profileID string, st profileStealthState) error {
	if a == nil || a.db == nil {
		return fmt.Errorf("db unavailable")
	}
	_, err := a.db.GetConn().Exec(`
		INSERT INTO profile_stealth_state (profile_id, paused_until, prefer_api, creepjs_score, last_probe_at, cookies_ok, updated_at)
		VALUES (?, ?, ?, ?, ?, ?, ?)
		ON CONFLICT(profile_id) DO UPDATE SET
			paused_until=excluded.paused_until,
			prefer_api=excluded.prefer_api,
			creepjs_score=excluded.creepjs_score,
			last_probe_at=excluded.last_probe_at,
			cookies_ok=excluded.cookies_ok,
			updated_at=excluded.updated_at`,
		profileID, st.PausedUntil, bool01(st.PreferAPI), st.CreepJSScore, st.LastProbeAt, bool01(st.CookiesOK),
		time.Now().UTC().Format(time.RFC3339),
	)
	return err
}

func bool01(v bool) int {
	if v {
		return 1
	}
	return 0
}

func (a *App) noteIPVisit(profileID, exitIP string) {
	if a == nil || a.db == nil || exitIP == "" {
		return
	}
	day := asymmetric.DayKeyUTC(time.Now())
	_, _ = a.db.GetConn().Exec(`
		INSERT INTO profile_ip_visits (profile_id, exit_ip, day_key, visit_count)
		VALUES (?, ?, ?, 1)
		ON CONFLICT(profile_id, exit_ip, day_key) DO UPDATE SET visit_count = visit_count + 1`,
		profileID, exitIP, day,
	)
}

func (a *App) ipVisitCount(profileID, exitIP string) int {
	if a == nil || a.db == nil {
		return 0
	}
	var count int
	_ = a.db.GetConn().QueryRow(`
		SELECT visit_count FROM profile_ip_visits
		WHERE profile_id = ? AND exit_ip = ? AND day_key = ?`,
		profileID, exitIP, asymmetric.DayKeyUTC(time.Now()),
	).Scan(&count)
	return count
}

func (a *App) buildStealthMatrixInput(profileID string) asymmetric.StealthMatrixInput {
	in := asymmetric.StealthMatrixInput{
		BioNoiseActive: true,
	}
	st := a.loadStealthState(profileID)
	if st.PausedUntil != "" {
		if t, err := time.Parse(time.RFC3339, st.PausedUntil); err == nil && time.Now().Before(t) {
			in.PauseActive = true
		}
	}
	if st.CreepJSScore > 0 {
		in.CreepJSTrust = st.CreepJSScore
	}
	in.CookiesInjected = st.CookiesOK

	signals := a.collectLiveDetectionSignals(profileID, nil)
	if fp, err := a.WorkbenchFingerprintProfile(profileID); err == nil && fp != nil {
		in.WebdriverHidden = !fp.Webdriver
		signals = a.collectLiveDetectionSignals(profileID, fp)
	}
	in.WebRTCClean = signals.WebrtcClean
	in.DNSConsistent = signals.DNSConsistent

	profile := a.getProfileSnapshot(profileID)
	report := browser.FullRuntimeProjectionReport(profile, nil, nil)
	in.Runtime80of80 = report.AppliedCount >= 80
	in.GeoLocaleMatch = a.geoLocaleMatchesProxy(profileID)
	in.ResidentialProxy = a.profileUsesResidentialProxy(profileID)
	in.VerifyV2Passed = signals.VerifyV2Passed

	exitIP := signals.ExitIP
	if exitIP == "" && profile != nil && profile.ProxyId != "" {
		h := a.BrowserProxyCheckIPHealth(profile.ProxyId)
		exitIP = h.IP
	}
	in.IPBudgetHeadroom = asymmetric.HasHeadroom(a.ipVisitCount(profileID, exitIP), asymmetric.DefaultIPDailyVisitCap)

	policy := asymmetric.DefaultHumanTimePolicy()
	if tz := inferTimezoneFromProfile(profile, a); tz != "" {
		policy.Timezone = tz
	}
	in.InHumanWindow = policy.InHumanWindow(time.Now())

	if health, err := a.WorkbenchAccountHealthReport(profileID); err == nil && health != nil {
		in.DetectionScore = health.DetectionScore
	}
	if summary, err := a.WorkbenchAccountOutcomeSummary(profileID); err == nil {
		if v, ok := summary["successRate"].(int); ok {
			in.AccountSuccess = v
		}
	}
	challenges := a.listRecentChallenges(profileID, 50)
	if len(challenges) > 0 {
		in.ChallengeRatePct = float64(len(challenges)) * 2
		if in.ChallengeRatePct > 100 {
			in.ChallengeRatePct = 100
		}
	}
	entropy := asymmetric.AssessEntropy(a.profileActivityEntropy(profileID))
	in.EntropyHumanLike = entropy.Level == "human_like"

	if bundle, err := a.ProfileTrustBundleGet(profileID, false); err == nil && bundle != nil {
		in.TrustBundleValid = bundle.HasValidTrust(time.Now())
		in.GraphTokenFresh = bundle.ValidAccessToken(time.Now())
		in.APIFirstReady = bundle.Provider == trust.ProviderMicrosoft && (bundle.HasRefreshToken() || bundle.ValidAccessToken(time.Now()))
		if in.TrustBundleValid && strings.TrimSpace(bundle.CookiesJSON) != "" {
			in.CookiesInjected = st.CookiesOK || trust.HasMicrosoftSessionCookies(mustParseTrustCookies(bundle.CookiesJSON))
		}
	}
	return in
}

func (a *App) geoLocaleMatchesProxy(profileID string) bool {
	profile := a.getProfileSnapshot(profileID)
	if profile == nil || strings.TrimSpace(profile.ProxyId) == "" {
		return true
	}
	health := a.BrowserProxyCheckIPHealth(profile.ProxyId)
	if !health.Ok || strings.TrimSpace(health.Country) == "" {
		return false
	}
	geoFP, geoLaunch := browser.ApplyGeoLocale(health.Country, nil, nil)
	expectedTZ := ""
	for _, arg := range append(geoFP, geoLaunch...) {
		if strings.HasPrefix(arg, "--timezone=") {
			expectedTZ = strings.TrimPrefix(arg, "--timezone=")
			break
		}
	}
	for _, arg := range append(profile.FingerprintArgs, profile.LaunchArgs...) {
		if strings.HasPrefix(arg, "--timezone=") {
			return expectedTZ == "" || strings.TrimPrefix(arg, "--timezone=") == expectedTZ
		}
	}
	return expectedTZ != ""
}

// AsymmetricStealthReportV2 returns 99+ multi-layer stealth matrix.
func (a *App) AsymmetricStealthReportV2(profileID string) (map[string]interface{}, error) {
	matrix := asymmetric.EvaluateStealthMatrix(a.buildStealthMatrixInput(profileID))
	legacy := asymmetric.EvaluateCostAsymmetryLegacy(matrix)
	gaps := a.AsymmetricBootstrapGaps(profileID)
	return map[string]interface{}{
		"matrix":       matrix,
		"stealthScore": matrix.TotalScore,
		"grade":        matrix.DisplayGrade,
		"strategy":     matrix.Strategy,
		"legacy":       legacy,
		"gaps":         gaps,
		"target99Plus": matrix.TotalScore >= 99 && matrix.DisplayGrade == "S+",
	}, nil
}

// AsymmetricBootstrapGaps lists blockers to reach 99+.
func (a *App) AsymmetricBootstrapGaps(profileID string) []string {
	gaps := make([]string, 0, 8)
	in := a.buildStealthMatrixInput(profileID)
	if !in.TrustBundleValid {
		gaps = append(gaps, "import ProfileTrustBundle (refresh_token) after one manual high-quality login")
	}
	if !in.ResidentialProxy {
		gaps = append(gaps, "bind residential/mobile proxy with clean IP health")
	}
	if !in.Runtime80of80 {
		gaps = append(gaps, "ensure MaterializeRuntimeArgs reaches 80/80 before launch")
	}
	if !in.GeoLocaleMatch {
		gaps = append(gaps, "align locale/timezone with proxy country (ApplyGeoLocale)")
	}
	if in.CreepJSTrust <= 0 {
		gaps = append(gaps, "run WorkbenchRunStealthProbeSuite to capture CreepJS trust")
	}
	if !in.GraphTokenFresh && in.TrustBundleValid {
		gaps = append(gaps, "refresh Graph token via GraphAPIMailList or token refresh loop")
	}
	if !in.IPBudgetHeadroom {
		gaps = append(gaps, "reduce daily visits per exit IP (max 5/day)")
	}
	return gaps
}

func mustParseTrustCookies(raw string) []trust.CookieEntry {
	c, _ := trust.ParseCookiesJSON(raw)
	return c
}

// AsymmetricApplyFeedbackAuto applies challenge feedback (rotate seed, pause, prefer API).
func (a *App) AsymmetricApplyFeedbackAuto(profileID string) (map[string]interface{}, error) {
	challenges := a.listRecentChallenges(profileID, 20)
	fb := asymmetric.DeriveFeedback(challenges, 48*time.Hour)
	st := a.loadStealthState(profileID)
	applied := make([]string, 0, 4)
	if fb.PreferAPIPath {
		st.PreferAPI = true
		applied = append(applied, "prefer_api")
	}
	if fb.PauseHours > 0 {
		st.PausedUntil = time.Now().UTC().Add(time.Duration(fb.PauseHours) * time.Hour).Format(time.RFC3339)
		applied = append(applied, "pause_"+fmt.Sprint(fb.PauseHours)+"h")
	}
	if fb.RotateFingerprintSeed {
		if seed, err := a.ProfileRotateFingerprintSeed(profileID); err == nil {
			applied = append(applied, "rotated_seed:"+seed[:8])
		}
	}
	_ = a.saveStealthState(profileID, st)
	return map[string]interface{}{"feedback": fb, "applied": applied}, nil
}

// WorkbenchRunStealthProbeSuite runs WebRTC + CreepJS heuristic probe on running profile.
func (a *App) WorkbenchRunStealthProbeSuite(profileID string) (map[string]interface{}, error) {
	profile, err := a.runningProfileForWorkbench(profileID)
	if err != nil {
		return nil, err
	}
	exitIP := ""
	if strings.TrimSpace(profile.ProxyId) != "" {
		h := a.BrowserProxyCheckIPHealth(profile.ProxyId)
		exitIP = h.IP
		a.noteIPVisit(profileID, exitIP)
	}
	webrtc, err := a.WorkbenchProbeWebRTC(profileID)
	if err != nil {
		webrtc = map[string]interface{}{"error": err.Error()}
	}

	executor, err := connectCDPExecutor(profile.DebugPort)
	if err != nil {
		return nil, err
	}
	defer executor.Close()

	_ = executor.Navigate("https://abrahamjuliot.github.io/creepjs/")
	time.Sleep(3 * time.Second)
	raw, evalErr := executor.EvaluateRaw(detection.CreepJSProbeJS())
	creep := detection.SiteProbeResult{SiteID: "creepjs", Message: "probe failed"}
	if evalErr == nil {
		creep = detection.ParseCreepJSProbePayload(raw)
	}

	st := a.loadStealthState(profileID)
	st.CreepJSScore = creep.TrustScore
	st.LastProbeAt = time.Now().UTC().Format(time.RFC3339)
	_ = a.saveStealthState(profileID, st)

	matrix := asymmetric.EvaluateStealthMatrix(a.buildStealthMatrixInput(profileID))
	return map[string]interface{}{
		"webrtc":     webrtc,
		"creepjs":    creep,
		"matrix":     matrix,
		"stealthScore": matrix.TotalScore,
		"grade":      matrix.DisplayGrade,
	}, nil
}

func (a *App) injectTrustCookiesAsync(profileID string, debugPort int) {
	a.hydrateTrustSurfaceAsync(profileID, debugPort)
}

func (a *App) hydrateTrustSurfaceAsync(profileID string, debugPort int) {
	if a == nil || debugPort <= 0 {
		return
	}
	bundle, err := a.ProfileTrustBundleGet(profileID, false)
	if err != nil || bundle == nil {
		return
	}
	surface := trust.FromBundle(*bundle)
	if !surface.HasUsableSession() && len(surface.LocalStorage) == 0 && len(surface.SessionStorage) == 0 {
		return
	}
	conn, err := behavior.ConnectPageCDP(debugPort)
	if err != nil {
		return
	}
	defer conn.Close()
	result := trust.HydrateCDP(func(method string, params map[string]interface{}) ([]byte, error) {
		return behavior.ExecuteCDP(conn, method, params)
	}, surface)
	st := a.loadStealthState(profileID)
	st.CookiesOK = result.CookiesSet > 0 || st.CookiesOK
	_ = a.saveStealthState(profileID, st)
	if len(result.Errors) > 0 {
		logger.New("Trust").Warn("trust surface hydrate partial",
			logger.F("profile_id", profileID),
			logger.F("cookies", result.CookiesSet),
			logger.F("localStorage", result.LocalStorageKeys),
			logger.F("errors", strings.Join(result.Errors, "; ")),
		)
	}
}

func (a *App) startTrustTokenRefreshLoop(ctx context.Context) {
	if a == nil {
		return
	}
	go func() {
		ticker := time.NewTicker(30 * time.Minute)
		defer ticker.Stop()
		for {
			select {
			case <-ctx.Done():
				return
			case <-ticker.C:
				a.refreshAllTrustTokens()
			}
		}
	}()
}

func (a *App) refreshAllTrustTokens() {
	if a == nil || a.db == nil {
		return
	}
	rows, err := a.db.GetConn().Query(`SELECT profile_id, payload FROM profile_trust_bundles`)
	if err != nil {
		return
	}
	defer rows.Close()
	for rows.Next() {
		var profileID, payload string
		if err := rows.Scan(&profileID, &payload); err != nil {
			continue
		}
		bundle, err := trust.ParseBundleJSON(payload)
		if err != nil || !bundle.HasRefreshToken() {
			continue
		}
		if bundle.ValidAccessToken(time.Now().Add(2 * time.Hour)) {
			continue
		}
		client, err := a.graphClientForProfile(profileID)
		if err != nil {
			continue
		}
		ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
		refreshed, err := client.RefreshAccessToken(ctx, bundle.RefreshToken)
		cancel()
		if err != nil {
			continue
		}
		refreshed.ProfileID = profileID
		refreshed.Provider = bundle.Provider
		_ = a.ProfileTrustBundleSave(profileID, refreshed)
	}
}

// AsymmetricRecordChallenge logs challenge and auto-applies feedback.
func (a *App) asymmetricRecordChallengeWithFeedback(profileID, site, challengeType string) error {
	if err := a.recordChallengeOnly(profileID, site, challengeType); err != nil {
		return err
	}
	_, _ = a.AsymmetricApplyFeedbackAuto(profileID)
	return nil
}

func (a *App) recordChallengeOnly(profileID, site, challengeType string) error {
	if a == nil || a.db == nil {
		return fmt.Errorf("database not ready")
	}
	profile := a.getProfileSnapshot(profileID)
	payload := map[string]interface{}{}
	if profile != nil {
		payload["humanizeSeed"] = profile.HumanizeSeed
		payload["proxyId"] = profile.ProxyId
	}
	raw, _ := json.Marshal(payload)
	_, err := a.db.GetConn().Exec(`
		INSERT INTO asymmetric_challenges (id, profile_id, site, challenge_type, payload, created_at)
	 VALUES (?, ?, ?, ?, ?, ?)`,
		"ch-"+generateUUID(), strings.TrimSpace(profileID), site, challengeType, string(raw), time.Now().UTC().Format(time.RFC3339),
	)
	return err
}
