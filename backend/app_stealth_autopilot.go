package backend

import (
	"context"
	"encoding/json"
	"fmt"
	"os"
	"strings"
	"time"

	"personal-pilot/backend/internal/asymmetric"
	"personal-pilot/backend/internal/browser"
	"personal-pilot/backend/internal/logger"
	"personal-pilot/backend/internal/trust"
)

const stealthAutopilotTag = "stealth-autopilot"
const auto99Tag = "auto-99"

// StealthAutopilotOptions configures zero-human 99+ pipeline.
type StealthAutopilotOptions struct {
	MaxIterations        int  `json:"maxIterations"`
	StartBrowser         bool `json:"startBrowser"`
	ImportEnvToken       bool `json:"importEnvToken"`
	HarvestSession       bool `json:"harvestSession"`
	BindResidentialProxy bool `json:"bindResidentialProxy"`
	PersistRuntime       bool `json:"persistRuntime"`
	RunProbes            bool `json:"runProbes"`
	ProbeTimeoutSec      int  `json:"probeTimeoutSec"`
}

func defaultStealthAutopilotOptions() StealthAutopilotOptions {
	return StealthAutopilotOptions{
		MaxIterations:        3,
		StartBrowser:         true,
		ImportEnvToken:       true,
		HarvestSession:       true,
		BindResidentialProxy: true,
		PersistRuntime:       true,
		RunProbes:            true,
		ProbeTimeoutSec:      120,
	}
}

// AsymmetricAutoReach99Plus runs the full zero-human stealth bootstrap loop.
func (a *App) AsymmetricAutoReach99Plus(profileID string, optsJSON string) (map[string]interface{}, error) {
	opts := defaultStealthAutopilotOptions()
	if strings.TrimSpace(optsJSON) != "" {
		_ = json.Unmarshal([]byte(optsJSON), &opts)
	}
	if opts.MaxIterations <= 0 {
		opts.MaxIterations = 3
	}
	report := asymmetric.AutopilotReport{ProfileID: profileID}
	for iter := 1; iter <= opts.MaxIterations; iter++ {
		report.Iterations = iter
		if opts.BindResidentialProxy {
			report.Steps = append(report.Steps, a.autopilotBindResidentialProxy(profileID))
		}
		if opts.PersistRuntime {
			report.Steps = append(report.Steps, a.autopilotPersistRuntimeHardening(profileID))
		}
		if opts.ImportEnvToken {
			report.Steps = append(report.Steps, a.autopilotImportEnvRefreshToken(profileID))
		}
		if opts.StartBrowser {
			step, err := a.autopilotEnsureBrowserRunning(profileID, time.Duration(opts.ProbeTimeoutSec)*time.Second)
			report.Steps = append(report.Steps, step)
			if err != nil && step.Status == "failed" {
				report.Blocked = true
				report.BlockReason = err.Error()
				break
			}
		}
		if opts.HarvestSession {
			report.Steps = append(report.Steps, a.autopilotHarvestBrowserTrust(profileID))
		}
		report.Steps = append(report.Steps, a.autopilotRefreshGraphToken(profileID))
		if opts.RunProbes {
			if _, err := a.WorkbenchRunStealthProbeSuite(profileID); err != nil {
				report.Steps = append(report.Steps, asymmetric.AutopilotStep{
					ID: asymmetric.StepRunProbes, Status: "failed", Detail: err.Error(),
				})
			} else {
				report.Steps = append(report.Steps, asymmetric.AutopilotStep{
					ID: asymmetric.StepRunProbes, Status: "ok", Changed: true,
				})
			}
		}
		if fb, err := a.AsymmetricApplyFeedbackAuto(profileID); err == nil {
			report.Steps = append(report.Steps, asymmetric.AutopilotStep{
				ID: asymmetric.StepApplyFeedback, Status: "ok", Detail: fmt.Sprint(fb),
			})
		}
		v2, err := a.AsymmetricStealthReportV2(profileID)
		if err != nil {
			return nil, err
		}
		if score, ok := v2["stealthScore"].(float64); ok {
			report.StealthScore = score
		}
		if grade, ok := v2["grade"].(string); ok {
			report.Grade = grade
		}
		if target, ok := v2["target99Plus"].(bool); ok {
			report.Target99Plus = target
		}
		report.Gaps = a.AsymmetricBootstrapGaps(profileID)
		if report.Target99Plus {
			break
		}
	}
	out, _ := json.Marshal(report)
	var m map[string]interface{}
	_ = json.Unmarshal(out, &m)
	return m, nil
}

func (a *App) runStealthAutopilotAsync(profileID string) {
	if a == nil {
		return
	}
	time.Sleep(5 * time.Second) // allow env injection + CDP settle
	_, err := a.AsymmetricAutoReach99Plus(profileID, "")
	if err != nil {
		logger.New("StealthAutopilot").Warn("auto 99+ pipeline failed",
			logger.F("profile_id", profileID), logger.F("error", err.Error()))
		return
	}
	logger.New("StealthAutopilot").Info("auto 99+ pipeline finished", logger.F("profile_id", profileID))
}

func profileWantsStealthAutopilot(profile *BrowserProfile) bool {
	if profile == nil {
		return false
	}
	for _, tag := range profile.Tags {
		t := strings.ToLower(strings.TrimSpace(tag))
		if t == stealthAutopilotTag || t == auto99Tag {
			return true
		}
	}
	return false
}

func (a *App) autopilotBindResidentialProxy(profileID string) asymmetric.AutopilotStep {
	step := asymmetric.AutopilotStep{ID: asymmetric.StepBindResidential, Status: "skipped"}
	if a.profileUsesResidentialProxy(profileID) {
		step.Detail = "already residential"
		return step
	}
	bestID, detail := a.pickBestResidentialProxyID()
	if bestID == "" {
		step.Status = "failed"
		step.Detail = detail
		return step
	}
	if err := a.autopilotUpdateProfileProxy(profileID, bestID); err != nil {
		step.Status = "failed"
		step.Detail = err.Error()
		return step
	}
	step.Status = "ok"
	step.Changed = true
	step.Detail = "bound proxy " + bestID
	return step
}

func (a *App) pickBestResidentialProxyID() (string, string) {
	proxies := a.BrowserProxyList()
	bestID := ""
	bestScore := int64(1<<62 - 1)
	for _, p := range proxies {
		if strings.TrimSpace(p.ProxyId) == "" {
			continue
		}
		health := a.BrowserProxyCheckIPHealth(p.ProxyId)
		if !health.Ok || !health.IsResidential {
			continue
		}
		score := health.FraudScore
		if score < bestScore {
			bestScore = score
			bestID = p.ProxyId
		}
	}
	if bestID == "" {
		return "", "no residential proxy with clean IP health in pool"
	}
	return bestID, ""
}

func (a *App) autopilotPersistRuntimeHardening(profileID string) asymmetric.AutopilotStep {
	step := asymmetric.AutopilotStep{ID: asymmetric.StepPersistRuntime, Status: "skipped"}
	profile := a.getProfileSnapshot(profileID)
	if profile == nil {
		step.Status = "failed"
		step.Detail = "profile not found"
		return step
	}
	fp, launch := browser.MaterializeRuntimeArgs(profile, a.config.Browser.DefaultFingerprintArgs, a.config.Browser.DefaultLaunchArgs)
	if strings.TrimSpace(profile.ProxyId) != "" {
		if health := a.BrowserProxyCheckIPHealth(profile.ProxyId); health.Ok && health.Country != "" {
			fp, launch = browser.ApplyGeoLocale(health.Country, fp, launch)
		}
	}
	fp, launch = browser.ApplyStrictAuthPreset(profile, fp, launch)
	report := browser.FullRuntimeProjectionReport(&browser.Profile{
		FingerprintArgs: fp, LaunchArgs: launch,
	}, a.config.Browser.DefaultFingerprintArgs, a.config.Browser.DefaultLaunchArgs)
	if err := a.autopilotUpdateProfileArgs(profileID, profile, fp, launch); err != nil {
		step.Status = "failed"
		step.Detail = err.Error()
		return step
	}
	step.Status = "ok"
	step.Changed = true
	step.Detail = fmt.Sprintf("materialized %d/80 runtime controls", report.AppliedCount)
	return step
}

func (a *App) autopilotImportEnvRefreshToken(profileID string) asymmetric.AutopilotStep {
	step := asymmetric.AutopilotStep{ID: asymmetric.StepImportEnvToken, Status: "skipped"}
	if bundle, err := a.ProfileTrustBundleGet(profileID, true); err == nil && bundle != nil && bundle.HasRefreshToken() {
		step.Detail = "trust bundle already has refresh_token"
		return step
	}
	tok := autopilotEnvRefreshToken(profileID)
	if tok == "" {
		step.Detail = "no PERSONAL_PILOT_MS_REFRESH_TOKEN env"
		return step
	}
	bundle := trust.Bundle{
		ProfileID:    profileID,
		Provider:     trust.ProviderMicrosoft,
		RefreshToken: tok,
		UpdatedAt:    time.Now().UTC(),
		Notes:        "autopilot:env_refresh_token",
	}
	client, err := a.graphClientForProfile(profileID)
	if err != nil {
		step.Status = "failed"
		step.Detail = err.Error()
		return step
	}
	ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
	defer cancel()
	if refreshed, err := client.RefreshAccessToken(ctx, tok); err == nil {
		refreshed.ProfileID = profileID
		refreshed.Provider = trust.ProviderMicrosoft
		refreshed.Notes = bundle.Notes
		bundle = refreshed
	}
	if err := a.ProfileTrustBundleSave(profileID, bundle); err != nil {
		step.Status = "failed"
		step.Detail = err.Error()
		return step
	}
	step.Status = "ok"
	step.Changed = true
	step.Detail = "imported refresh_token from environment"
	return step
}

func autopilotEnvRefreshToken(profileID string) string {
	safe := strings.ToUpper(strings.NewReplacer("-", "_", " ", "_").Replace(strings.TrimSpace(profileID)))
	if v := strings.TrimSpace(os.Getenv("PERSONAL_PILOT_MS_REFRESH_TOKEN_" + safe)); v != "" {
		return v
	}
	return strings.TrimSpace(os.Getenv("PERSONAL_PILOT_MS_REFRESH_TOKEN"))
}

func (a *App) autopilotEnsureBrowserRunning(profileID string, timeout time.Duration) (asymmetric.AutopilotStep, error) {
	step := asymmetric.AutopilotStep{ID: asymmetric.StepEnsureRunning, Status: "ok"}
	if _, err := a.runningProfileForWorkbench(profileID); err == nil {
		step.Detail = "already running"
		return step, nil
	}
	if _, err := a.BrowserInstanceStart(profileID); err != nil {
		step.Status = "failed"
		step.Detail = err.Error()
		return step, err
	}
	port, err := a.waitProfileDebugReady(profileID, timeout)
	if err != nil {
		step.Status = "failed"
		step.Detail = err.Error()
		return step, err
	}
	time.Sleep(4 * time.Second) // env injection + cookie inject window
	step.Changed = true
	step.Detail = fmt.Sprintf("started debugPort=%d", port)
	return step, nil
}

func (a *App) waitProfileDebugReady(profileID string, timeout time.Duration) (int, error) {
	if timeout <= 0 {
		timeout = 90 * time.Second
	}
	deadline := time.Now().Add(timeout)
	for time.Now().Before(deadline) {
		st, err := a.BrowserInstanceStatus(profileID)
		if err == nil && st != nil && st.Running && st.DebugPort > 0 && st.DebugReady {
			return st.DebugPort, nil
		}
		time.Sleep(400 * time.Millisecond)
	}
	return 0, fmt.Errorf("browser debug port not ready within %s", timeout)
}

func (a *App) autopilotHarvestBrowserTrust(profileID string) asymmetric.AutopilotStep {
	step := asymmetric.AutopilotStep{ID: asymmetric.StepHarvestTrust, Status: "skipped"}
	profile, err := a.runningProfileForWorkbench(profileID)
	if err != nil {
		step.Status = "failed"
		step.Detail = err.Error()
		return step
	}
	exitIP := ""
	if profile.ProxyId != "" {
		exitIP = a.BrowserProxyCheckIPHealth(profile.ProxyId).IP
	}
	cookies, err := a.BrowserGetCookies(profileID)
	if err != nil {
		step.Status = "failed"
		step.Detail = err.Error()
		return step
	}
	trustCookies := make([]trust.CookieEntry, 0, len(cookies))
	for _, c := range cookies {
		trustCookies = append(trustCookies, trust.CookieEntry{
			Name: c.Name, Value: c.Value, Domain: c.Domain, Path: c.Path,
			Secure: c.Secure, HTTPOnly: c.HttpOnly,
		})
	}
	harvest := trust.TokenHarvest{Source: "cookies_only"}
	executor, err := connectCDPExecutor(profile.DebugPort)
	if err == nil {
		_ = executor.Navigate("https://outlook.live.com/mail/")
		time.Sleep(4 * time.Second)
		if raw, evalErr := executor.EvaluateRaw(trust.MicrosoftSessionHarvestJS); evalErr == nil {
			harvest = trust.ParseTokenHarvestPayload(raw)
			harvest.Source = "outlook_msal_storage"
		}
		_ = executor.Close()
	}
	if len(trustCookies) == 0 && harvest.RefreshToken == "" && harvest.AccessToken == "" {
		step.Detail = "no harvestable session in browser"
		return step
	}
	bundle := trust.BundleFromHarvest(profileID, trust.ProviderMicrosoft, trustCookies, harvest, exitIP, profile.ProxyId)
	if ls, err := a.WorkbenchGetLocalStorage(profileID); err == nil && len(ls) > 0 {
		bundle.LocalStorage = ls
	}
	if ss, err := a.WorkbenchGetSessionStorage(profileID); err == nil && len(ss) > 0 {
		bundle.SessionStorage = ss
	}
	if existing, err := a.ProfileTrustBundleGet(profileID, false); err == nil && existing != nil {
		if bundle.RefreshToken == "" {
			bundle.RefreshToken = existing.RefreshToken
		}
		if bundle.AccessToken == "" {
			bundle.AccessToken = existing.AccessToken
			bundle.ExpiresAt = existing.ExpiresAt
		}
		if bundle.CookiesJSON == "" || bundle.CookiesJSON == "null" {
			bundle.CookiesJSON = existing.CookiesJSON
		}
	}
	if err := a.ProfileTrustBundleSave(profileID, bundle); err != nil {
		step.Status = "failed"
		step.Detail = err.Error()
		return step
	}
	go a.injectTrustCookiesAsync(profileID, profile.DebugPort)
	step.Status = "ok"
	step.Changed = true
	step.Detail = fmt.Sprintf("cookies=%d refresh=%t access=%t", len(trustCookies), bundle.HasRefreshToken(), bundle.ValidAccessToken(time.Now()))
	return step
}

func (a *App) autopilotRefreshGraphToken(profileID string) asymmetric.AutopilotStep {
	step := asymmetric.AutopilotStep{ID: asymmetric.StepRefreshGraph, Status: "skipped"}
	bundle, err := a.ProfileTrustBundleGet(profileID, false)
	if err != nil || bundle == nil || !bundle.HasRefreshToken() {
		step.Detail = "no refresh_token to refresh"
		return step
	}
	if bundle.ValidAccessToken(time.Now().Add(15 * time.Minute)) {
		step.Detail = "access token still fresh"
		return step
	}
	client, err := a.graphClientForProfile(profileID)
	if err != nil {
		step.Status = "failed"
		step.Detail = err.Error()
		return step
	}
	ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
	defer cancel()
	refreshed, err := client.RefreshAccessToken(ctx, bundle.RefreshToken)
	if err != nil {
		step.Status = "failed"
		step.Detail = err.Error()
		return step
	}
	refreshed.ProfileID = profileID
	refreshed.Provider = bundle.Provider
	refreshed.CookiesJSON = bundle.CookiesJSON
	refreshed.ProxyID = bundle.ProxyID
	refreshed.ExitIP = bundle.ExitIP
	_ = a.ProfileTrustBundleSave(profileID, refreshed)
	step.Status = "ok"
	step.Changed = true
	step.Detail = "graph access token refreshed"
	return step
}

func (a *App) autopilotUpdateProfileProxy(profileID, proxyID string) error {
	profile := a.getProfileSnapshot(profileID)
	if profile == nil {
		return fmt.Errorf("profile not found")
	}
	input := profileToInput(profile)
	input.ProxyId = proxyID
	_, err := a.BrowserProfileUpdate(profileID, input)
	return err
}

func (a *App) autopilotUpdateProfileArgs(profileID string, profile *BrowserProfile, fp, launch []string) error {
	input := profileToInput(profile)
	input.FingerprintArgs = append([]string{}, fp...)
	input.LaunchArgs = append([]string{}, launch...)
	_, err := a.BrowserProfileUpdate(profileID, input)
	return err
}

func profileToInput(profile *BrowserProfile) BrowserProfileInput {
	if profile == nil {
		return BrowserProfileInput{}
	}
	return BrowserProfileInput{
		ProfileName:          profile.ProfileName,
		UserDataDir:          profile.UserDataDir,
		CoreId:               profile.CoreId,
		FingerprintArgs:      append([]string{}, profile.FingerprintArgs...),
		PreferencesOverrides: profile.PreferencesOverrides,
		ProxyId:              profile.ProxyId,
		ProxyConfig:          profile.ProxyConfig,
		LaunchArgs:           append([]string{}, profile.LaunchArgs...),
		Tags:                 append([]string{}, profile.Tags...),
		Keywords:             append([]string{}, profile.Keywords...),
		GroupId:              profile.GroupId,
		BehaviorProfileID:    profile.BehaviorProfileID,
		HumanizeSeed:         profile.HumanizeSeed,
	}
}
