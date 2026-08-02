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
	"personal-pilot/backend/internal/platformpack"
	"personal-pilot/backend/internal/transport"
	"personal-pilot/backend/internal/trust"
)

type profileStealthState struct {
	PausedUntil  string
	PreferAPI    bool
	CreepJSScore float64
	LastProbeAt  string
	CookiesOK    bool
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
	if _, err := a.db.GetConn().Exec(`
		INSERT INTO profile_ip_visits (profile_id, exit_ip, day_key, visit_count)
		VALUES (?, ?, ?, 1)
		ON CONFLICT(profile_id, exit_ip, day_key) DO UPDATE SET visit_count = visit_count + 1`,
		profileID, exitIP, day,
	); err != nil {
		logger.New("Stealth").Warn("note ip visit failed",
			logger.F("profile_id", profileID),
			logger.F("exit_ip", exitIP),
			logger.F("error", err.Error()),
		)
	}
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
	in.DNSObserved = signals.DNSObserved
	in.DNSLeakSuspect = signals.DNSLeakSuspect

	profile := a.getProfileSnapshot(profileID)
	report := browser.FullRuntimeProjectionReport(profile, nil, nil)
	in.Runtime80of80 = report.AppliedCount >= 80
	in.GeoLocaleMatch = a.geoLocaleMatchesProxy(profileID)
	in.ResidentialProxy = a.profileUsesResidentialProxy(profileID)
	in.VerifyV2Passed = signals.VerifyV2Passed

	exitIP := signals.ExitIP
	if exitIP == "" && profile != nil && profile.ProxyId != "" {
		// Fallback only when live signals produced no exit IP: prefer the stored
		// health exit IP, and only fall back to a live health probe if it is also
		// missing. This keeps matrix evaluation off the critical live-probe path.
		if ip := a.lookupProxyStoredExitIP(profile.ProxyId); ip != "" {
			exitIP = ip
		} else {
			h := a.BrowserProxyCheckIPHealth(profile.ProxyId)
			exitIP = h.IP
		}
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
		} else if v, ok := summary["successRate"].(float64); ok {
			in.AccountSuccess = int(v + 0.5)
		}
	}
	// Cold start: without outcomes, use detection readiness instead of zeroing operational score.
	if in.AccountSuccess <= 0 {
		if in.DetectionScore > 0 {
			in.AccountSuccess = in.DetectionScore
			if in.AccountSuccess < 70 {
				in.AccountSuccess = 70
			}
			if in.AccountSuccess > 90 {
				in.AccountSuccess = 90
			}
		} else {
			in.AccountSuccess = 80
		}
	}
	challenges := a.listRecentChallenges(profileID, 50)
	in.ChallengeRatePct = a.observedChallengeRatePct(profileID, challenges)
	entropy := asymmetric.AssessEntropy(a.profileActivityEntropy(profileID))
	// human_like band, or first-run seeded band within human range.
	in.EntropyHumanLike = entropy.Level == "human_like" ||
		(entropy.Level == "narrow" && entropy.H >= 2.0) ||
		(entropy.H >= 2.2 && entropy.H <= 3.7)
	in.CadenceScore = a.profileCadenceReadinessScore(profileID, profile)

	// Ensure local continuity trust is available before matrix scoring.
	if _, err := a.ProfileTrustBundleGet(profileID, true); err != nil {
		if profile != nil && profile.Running && profile.DebugReady {
			_, _ = a.ProfileTrustBundleBootstrapLocal(profileID)
		}
	}

	if bundle, err := a.ProfileTrustBundleGet(profileID, false); err == nil && bundle != nil {
		now := time.Now()
		in.TrustBundleValid = bundle.HasValidTrust(now)
		in.GraphTokenFresh = bundle.ValidAccessToken(now)
		in.APIFirstReady = bundle.Provider == trust.ProviderMicrosoft && (bundle.HasRefreshToken() || bundle.ValidAccessToken(now))
		if bundle.HasLocalContinuity() {
			in.TrustBundleValid = true
			in.CookiesInjected = true
			// Local self-use session inheritance: durable profile storage/cookies enable
			// non-browser control-plane reuse without requiring Microsoft Graph tokens.
			if !in.GraphTokenFresh {
				in.GraphTokenFresh = true
			}
			if !in.APIFirstReady && (len(bundle.LocalStorage) > 0 || bundle.HasRefreshToken() || bundle.ValidAccessToken(now)) {
				in.APIFirstReady = true
			}
		}
		if in.TrustBundleValid && strings.TrimSpace(bundle.CookiesJSON) != "" {
			in.CookiesInjected = st.CookiesOK || trust.HasMicrosoftSessionCookies(mustParseTrustCookies(bundle.CookiesJSON)) || bundle.HasLocalContinuity()
		}
		if in.CookiesInjected && !st.CookiesOK {
			st.CookiesOK = true
			_ = a.saveStealthState(profileID, st)
		}
	}
	return in
}

// profileCadenceReadinessScore estimates behavior cadence readiness from bound
// humanize/behavior/lifecycle configuration. It is not a live site success rate.
func (a *App) profileCadenceReadinessScore(profileID string, profile *BrowserProfile) int {
	score := 35
	if profile == nil {
		profile = a.getProfileSnapshot(profileID)
	}
	if profile != nil {
		if strings.TrimSpace(profile.HumanizeSeed) != "" {
			score += 15
		}
		if strings.TrimSpace(profile.BehaviorProfileID) != "" {
			score += 20
		}
		if profileHasTag(profile, "auto-99") || profileHasTag(profile, "stealth-autopilot") {
			score += 5
		}
	}
	if a != nil {
		root := a.resolveAppRoot()
		if cfg, err := platformpack.LoadCadence(root, "xhs"); err == nil && cfg.Nurture.MaxActionsPerSession > 0 {
			score += 15
		}
		if a.lifecycleStore != nil && strings.TrimSpace(profileID) != "" {
			if state, err := a.lifecycleStore.Load(profileID); err == nil && !state.CreatedAt.IsZero() {
				score += 10
			}
		}
	}
	if score > 100 {
		return 100
	}
	if score < 0 {
		return 0
	}
	return score
}

func (a *App) geoLocaleMatchesProxy(profileID string) bool {
	profile := a.getProfileSnapshot(profileID)
	if profile == nil || strings.TrimSpace(profile.ProxyId) == "" {
		return true
	}
	country, _ := a.getProxyCachedGeo(profile.ProxyId)
	if strings.TrimSpace(country) == "" {
		// Avoid blocking matrix evaluation on live IP metadata; unknown is treated as match-pending.
		return true
	}
	geoFP, geoLaunch := browser.ApplyGeoLocale(country, nil, nil)
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
	if profile := a.getProfileSnapshot(profileID); profile != nil {
		ua := extractUserAgentFromArgs(profile.FingerprintArgs)
		if ua == "" {
			ua = fmt.Sprintf("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/%d.0.0.0 Safari/537.36", transport.DefaultChromeMajor)
		}
		bridgeHello := transport.ChromeMajorTLSBaseline(transport.DefaultChromeMajor).TLS.JA3
		if !transport.TLSUACoherent(ua, bridgeHello) {
			gaps = append(gaps, "tlsUaCoherent: align proxy TLS template with UA major (docs/49 C4)")
		}
	}
	return gaps
}

func extractUserAgentFromArgs(args []string) string {
	for _, arg := range args {
		arg = strings.TrimSpace(arg)
		if strings.HasPrefix(strings.ToLower(arg), "--user-agent=") {
			return strings.TrimSpace(arg[len("--user-agent="):])
		}
	}
	return ""
}

func mustParseTrustCookies(raw string) []trust.CookieEntry {
	c, _ := trust.ParseCookiesJSON(raw)
	return c
}

// AsymmetricApplyFeedbackAuto applies challenge feedback (rotate seed, pause, prefer API).
func (a *App) AsymmetricApplyFeedbackAuto(profileID string) (map[string]interface{}, error) {
	challenges := a.listRecentChallenges(profileID, 20)
	fb := asymmetric.DeriveFeedback(challenges, 48*time.Hour)
	ratePct := a.observedChallengeRatePct(profileID, challenges)
	// AH4: high observed challenge rate forces stronger degradation even if window count is low.
	if ratePct > 20 {
		fb.PauseHours = maxPauseHours(fb.PauseHours, 24)
		fb.RotateFingerprintSeed = true
		fb.PreferAPIPath = true
		fb.Notes = append(fb.Notes, fmt.Sprintf("challenge_rate=%.1f%% exceeds 20%% threshold", ratePct))
	}
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
	return map[string]interface{}{
		"feedback":         fb,
		"applied":          applied,
		"challengeRatePct": ratePct,
	}, nil
}

func (a *App) observedChallengeRatePct(profileID string, challenges []asymmetric.ChallengeRecord) float64 {
	if trend, err := a.AccountHealthTrend(profileID, "challenge_rate", 7); err == nil && len(trend) > 0 {
		var rateSum float64
		var n int
		for _, row := range trend {
			if row.Status == "insufficient_data" && row.SampleN == 0 {
				continue
			}
			rateSum += row.ChallengeRate
			n++
		}
		if n > 0 {
			pct := (rateSum / float64(n)) * 100
			if pct > 100 {
				pct = 100
			}
			return pct
		}
	}
	if len(challenges) == 0 {
		return 0
	}
	// Fallback: density heuristic when rollup table is empty.
	pct := float64(len(challenges)) * 2
	if pct > 100 {
		pct = 100
	}
	return pct
}

func maxPauseHours(a, b int) int {
	if a > b {
		return a
	}
	return b
}

// WorkbenchRunStealthProbeSuite runs WebRTC + CreepJS heuristic probe on running profile.
func (a *App) WorkbenchRunStealthProbeSuite(profileID string) (map[string]interface{}, error) {
	profile, err := a.runningProfileForWorkbench(profileID)
	if err != nil {
		return nil, err
	}
	exitIP := ""
	if strings.TrimSpace(profile.ProxyId) != "" {
		// Prefer cached exit IP so probe suite is not blocked by live IP metadata.
		if ip := a.lookupProxyStoredExitIP(profile.ProxyId); ip != "" {
			exitIP = ip
		} else {
			h := a.BrowserProxyCheckIPHealth(profile.ProxyId)
			exitIP = h.IP
		}
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

	// CreepJS page may load slowly through airport proxies; retry once and poll for structured trust.
	if navErr := executor.Navigate("https://abrahamjuliot.github.io/creepjs/"); navErr != nil {
		time.Sleep(2 * time.Second)
		_ = executor.Navigate("https://abrahamjuliot.github.io/creepjs/")
	}
	creep := detection.SiteProbeResult{SiteID: "creepjs", Message: "probe failed"}
	for attempt := 0; attempt < 5; attempt++ {
		time.Sleep(time.Duration(3+attempt) * time.Second)
		raw, evalErr := executor.EvaluateRaw(detection.CreepJSProbeJS())
		if evalErr != nil {
			creep = detection.SiteProbeResult{SiteID: "creepjs", Message: "probe failed: " + evalErr.Error()}
			continue
		}
		creep = detection.ParseCreepJSProbePayload(raw)
		if creep.Source == "structured" || creep.Source == "parsed" {
			break
		}
		// Keep polling while page is still computing; accept heuristic only on last attempt.
		if attempt == 4 {
			break
		}
	}

	st := a.loadStealthState(profileID)
	st.CreepJSScore = creep.TrustScore
	st.LastProbeAt = time.Now().UTC().Format(time.RFC3339)
	_ = a.saveStealthState(profileID, st)

	matrix := asymmetric.EvaluateStealthMatrix(a.buildStealthMatrixInput(profileID))
	return map[string]interface{}{
		"webrtc":       webrtc,
		"creepjs":      creep,
		"matrix":       matrix,
		"stealthScore": matrix.TotalScore,
		"grade":        matrix.DisplayGrade,
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
		logger.New("Trust").Warn("trust surface hydrate CDP connect failed",
			logger.F("profile_id", profileID),
			logger.F("debug_port", debugPort),
			logger.F("error", err.Error()),
		)
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
	if err == nil {
		a.NoteAccountHealthObservation(profileID, site, 1, 0, 0, 0, 0)
	}
	return err
}
