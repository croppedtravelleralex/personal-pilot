package backend

import (
	"encoding/json"
	"fmt"
	"strings"
	"sync"
	"time"

	"personal-pilot/backend/internal/behavior"
	"personal-pilot/backend/internal/behavior/humanize"
	"personal-pilot/backend/internal/browser"
	"personal-pilot/backend/internal/launchcode"
	"personal-pilot/backend/internal/logger"
)

// humanizationLevelMap maps string level names to humanize.HumanizationLevel.
var humanizationLevelMap = map[string]humanize.HumanizationLevel{
	"high":   humanize.LevelHigh,
	"medium": humanize.LevelMedium,
	"low":    humanize.LevelMinimal,
	"none":   humanize.LevelNone,
}

// StartInstance 实现 launchcode.BrowserStarter 接口
func (a *App) StartInstance(profileId string) (*browser.Profile, error) {
	return a.BrowserInstanceStart(profileId)
}

// StartInstanceWithParams 实现 launchcode.BrowserStarterWithParams 接口
func (a *App) StartInstanceWithParams(profileId string, params launchcode.LaunchRequestParams) (*browser.Profile, error) {
	return a.BrowserInstanceStartWithParams(profileId, params.LaunchArgs, params.StartURLs, params.SkipDefaultStartURLs)
}

// StopInstance implements launchcode.BrowserStopper for the local HTTP API.
func (a *App) StopInstance(profileId string) (*browser.Profile, error) {
	return a.BrowserInstanceStop(profileId)
}

// GetInstanceStatus implements launchcode.InstanceStater.
func (a *App) GetInstanceStatus(profileId string) (*browser.Profile, error) {
	if a.browserMgr == nil {
		return nil, fmt.Errorf("browser manager not available")
	}
	a.browserMgr.Mutex.Lock()
	profile, ok := a.browserMgr.Profiles[profileId]
	a.browserMgr.Mutex.Unlock()
	if !ok {
		return nil, fmt.Errorf("profile not found")
	}
	// Return a copy of the profile status
	profileCopy := *profile
	return &profileCopy, nil
}

// CreateProfile implements launchcode.InstanceOperator.
func (a *App) CreateProfile(input browser.ProfileInput) (*browser.Profile, error) {
	return a.browserMgr.Create(input)
}

// UpdateProfile implements launchcode.InstanceOperator.
func (a *App) UpdateProfile(profileID string, input browser.ProfileInput) (*browser.Profile, error) {
	return a.browserMgr.Update(profileID, input)
}

// DeleteProfile implements launchcode.InstanceOperator.
func (a *App) DeleteProfile(profileID string) error {
	return a.browserMgr.Delete(profileID)
}

// CopyProfile implements launchcode.InstanceOperator.
func (a *App) CopyProfile(profileId string, newName string) (*browser.Profile, error) {
	return a.browserMgr.Copy(profileId, newName)
}

func (a *App) WorkbenchNavigateProfile(profileId string, rawURL string) error {
	return a.WorkbenchNavigateProfileOnTab(profileId, rawURL, "")
}

// WorkbenchNavigateProfileOnTab navigates a specific tab when tabID is non-empty.
func (a *App) WorkbenchNavigateProfileOnTab(profileId string, rawURL string, tabID string) error {
	results, err := a.WorkbenchExecuteActions(profileId, []launchcode.ActionRequest{{
		Type:              "navigate",
		URL:               rawURL,
		PostWaitMs:        2500,
		HumanizationLevel: "high",
		TabID:             strings.TrimSpace(tabID),
	}})
	if err != nil {
		return err
	}
	if len(results) == 0 {
		return fmt.Errorf("navigate returned no results")
	}
	if !results[0].OK {
		if results[0].Error != "" {
			return fmt.Errorf("%s", results[0].Error)
		}
		return fmt.Errorf("navigate failed")
	}
	return nil
}

func (a *App) WorkbenchRefreshProfile(profileId string) error {
	return a.SynchronizerRefreshProfile(profileId)
}

func (a *App) WorkbenchCaptureScreenshot(profileId string) (string, error) {
	return a.SynchronizerCaptureScreenshot(profileId)
}

func (a *App) WorkbenchFingerprintProfile(profileId string) (*browser.FingerprintSnapshot, error) {
	profile, err := a.runningProfileForWorkbench(profileId)
	if err != nil {
		return nil, err
	}
	return browser.ExtractFingerprint(profile.DebugPort)
}

func (a *App) WorkbenchFingerprintHealthProfile(profileId string) (*browser.FingerprintHealthProfile, error) {
	profile, err := a.runningProfileForWorkbench(profileId)
	if err != nil {
		return nil, err
	}
	fingerprint, err := browser.ExtractFingerprint(profile.DebugPort)
	if err != nil {
		return nil, err
	}
	return browser.NewFingerprintHealthProfile(profile.ProfileId, profile.FingerprintArgs, fingerprint, time.Now()), nil
}

func (a *App) IdentityReportProfile(profileId string) (*browser.IdentityStrengthReport, error) {
	profile, err := a.runningProfileForWorkbench(profileId)
	if err != nil {
		return nil, err
	}
	fingerprint, err := browser.ExtractFingerprint(profile.DebugPort)
	if err != nil {
		return nil, err
	}
	launchAuditError := ""
	if a.browserMgr != nil {
		if err := a.browserMgr.ValidateProfileLaunchAudit(profile); err != nil {
			launchAuditError = err.Error()
		}
	}
	return browser.NewIdentityStrengthReport(profile, fingerprint, time.Now(), browser.IdentityReportContext{
		LaunchAuditError: launchAuditError,
	}), nil
}

func (a *App) WorkbenchActivateProfile(profileId string) error {
	return a.SynchronizerActivateProfile(profileId)
}

func (a *App) WorkbenchClickElement(profileID string, selector string) error {
	return a.WorkbenchClickElementOnTab(profileID, selector, "")
}

// WorkbenchClickElementOnTab clicks on a specific tab target when tabID is set.
func (a *App) WorkbenchClickElementOnTab(profileID string, selector string, tabID string) error {
	profile, err := a.runningProfileForWorkbench(profileID)
	if err != nil {
		return err
	}
	executor, err := connectCDPExecutorForTarget(profile.DebugPort, tabID)
	if err != nil {
		return err
	}
	defer executor.Close()
	return executor.ExecuteHumanizedClick(selector)
}

func (a *App) WorkbenchTypeText(profileID string, selector string, text string) error {
	return a.WorkbenchTypeTextOnTab(profileID, selector, "", text)
}

// WorkbenchTypeTextOnTab types into a specific tab target when tabID is set.
func (a *App) WorkbenchTypeTextOnTab(profileID string, selector string, tabID string, text string) error {
	profile, err := a.runningProfileForWorkbench(profileID)
	if err != nil {
		return err
	}
	executor, err := connectCDPExecutorForTarget(profile.DebugPort, tabID)
	if err != nil {
		return err
	}
	defer executor.Close()
	return executor.ExecuteHumanizedType(selector, text)
}

func (a *App) WorkbenchScrollPage(profileID string, distance uint32) error {
	return a.WorkbenchScrollPageOnTab(profileID, distance, "")
}

// WorkbenchScrollPageOnTab scrolls a specific tab target when tabID is set.
func (a *App) WorkbenchScrollPageOnTab(profileID string, distance uint32, tabID string) error {
	profile, err := a.runningProfileForWorkbench(profileID)
	if err != nil {
		return err
	}
	executor, err := connectCDPExecutorForTarget(profile.DebugPort, tabID)
	if err != nil {
		return err
	}
	defer executor.Close()
	return executor.ExecuteHumanizedScroll(distance)
}

func (a *App) WorkbenchCaptureFullReport(profileID string) (*launchcode.WorkbenchFullReport, error) {
	profile, err := a.runningProfileForWorkbench(profileID)
	if err != nil {
		return nil, err
	}
	executor, err := connectCDPExecutor(profile.DebugPort)
	if err != nil {
		return nil, err
	}
	defer executor.Close()

	now := time.Now()
	report := &launchcode.WorkbenchFullReport{
		ProfileID:      profileID,
		Tabs:           []browser.Tab{},
		Cookies:        []map[string]interface{}{},
		LocalStorage:   map[string]string{},
		SessionStorage: map[string]string{},
		Failures:       []launchcode.WorkbenchReportFailure{},
		CapturedAt:     now.UTC().Format(time.RFC3339Nano),
		Source:         "local-cdp",
	}
	addFailure := func(step string, err error) {
		if err == nil {
			return
		}
		report.Failures = append(report.Failures, launchcode.WorkbenchReportFailure{
			Step:  step,
			Error: err.Error(),
		})
	}

	url, err := executor.GetPageURL()
	if err != nil {
		return nil, fmt.Errorf("get page url: %w", err)
	}
	report.URL = url
	title, err := executor.GetPageTitle()
	if err != nil {
		return nil, fmt.Errorf("get page title: %w", err)
	}
	report.Title = title
	html, err := executor.EvaluateJS("document.documentElement ? document.documentElement.outerHTML : ''")
	if err != nil {
		return nil, fmt.Errorf("get page html: %w", err)
	}
	report.HTML = html
	text, err := executor.EvaluateJS("document.body ? document.body.innerText : ''")
	if err != nil {
		return nil, fmt.Errorf("get page text: %w", err)
	}
	report.Text = text

	if screenshot, err := executor.CaptureScreenshot(); err != nil {
		addFailure("screenshot", err)
	} else {
		report.Screenshot = screenshot
	}
	if tabs, err := a.WorkbenchListTabs(profileID); err != nil {
		addFailure("tabs", err)
	} else if tabs != nil {
		report.Tabs = tabs
	}
	if cookies, err := a.WorkbenchGetCookies(profileID); err != nil {
		addFailure("cookies", err)
	} else if cookies != nil {
		report.Cookies = cookies
	}
	if localStorage, err := a.WorkbenchGetLocalStorage(profileID); err != nil {
		addFailure("localStorage", err)
	} else if localStorage != nil {
		report.LocalStorage = localStorage
	}
	if sessionStorage, err := a.WorkbenchGetSessionStorage(profileID); err != nil {
		addFailure("sessionStorage", err)
	} else if sessionStorage != nil {
		report.SessionStorage = sessionStorage
	}
	if fingerprint, err := a.WorkbenchFingerprintProfile(profileID); err != nil {
		addFailure("fingerprint", err)
	} else {
		report.Fingerprint = fingerprint
	}
	if health, err := a.WorkbenchFingerprintHealthProfile(profileID); err != nil {
		addFailure("fingerprintHealth", err)
	} else {
		report.FingerprintHealth = health
	}
	if identity, err := a.IdentityReportProfile(profileID); err != nil {
		addFailure("identityReport", err)
	} else {
		report.IdentityReport = identity
	}

	return report, nil
}

func (a *App) WorkbenchShowMousePointer(profileID string) error {
	profile, err := a.runningProfileForWorkbench(profileID)
	if err != nil {
		return err
	}
	executor, err := connectCDPExecutor(profile.DebugPort)
	if err != nil {
		return err
	}
	defer executor.Close()
	return executor.ShowMousePointerOverlay()
}

func (a *App) WorkbenchHideMousePointer(profileID string) error {
	profile, err := a.runningProfileForWorkbench(profileID)
	if err != nil {
		return err
	}
	executor, err := connectCDPExecutor(profile.DebugPort)
	if err != nil {
		return err
	}
	defer executor.Close()
	return executor.HideMousePointerOverlay()
}

func (a *App) WorkbenchArrangeProfiles(profileIds []string, layout string) ([]launchcode.WorkbenchWindowPlacement, error) {
	placements, err := a.SynchronizerArrangeProfiles(profileIds, layout)
	result := make([]launchcode.WorkbenchWindowPlacement, 0, len(placements))
	for _, placement := range placements {
		result = append(result, launchcode.WorkbenchWindowPlacement{
			ProfileID:   placement.ProfileID,
			ProfileName: placement.ProfileName,
			Pid:         placement.Pid,
			Found:       placement.Found,
			X:           placement.X,
			Y:           placement.Y,
			Width:       placement.Width,
			Height:      placement.Height,
			Error:       placement.Error,
		})
	}
	return result, err
}

// BrowserProfileGetCode 获取实例的 LaunchCode（Wails 绑定）
func (a *App) BrowserProfileGetCode(profileId string) (string, error) {
	if a.launchCodeSvc == nil {
		return "", nil
	}
	return a.launchCodeSvc.EnsureCode(profileId)
}

// BrowserProfileRegenerateCode 重新生成实例的 LaunchCode（Wails 绑定）
func (a *App) BrowserProfileRegenerateCode(profileId string) (string, error) {
	if a.launchCodeSvc == nil {
		return "", nil
	}
	return a.launchCodeSvc.RegenerateCode(profileId)
}

// BrowserProfileSetCode 自定义设置实例 LaunchCode（Wails 绑定）
func (a *App) BrowserProfileSetCode(profileId string, code string) (string, error) {
	if a.launchCodeSvc == nil {
		return "", nil
	}
	return a.launchCodeSvc.SetCode(profileId, code)
}

// BrowserInstanceStartByCode 通过 LaunchCode 启动实例（Wails 绑定）
func (a *App) BrowserInstanceStartByCode(code string) (*browser.Profile, error) {
	if a.launchCodeSvc == nil {
		return nil, fmt.Errorf("launch code service not initialized")
	}
	profileId, err := a.launchCodeSvc.Resolve(code)
	if err != nil {
		return nil, err
	}
	return a.BrowserInstanceStart(profileId)
}

// ─── Cookie Operations ──────────────────────────────────────────────────────────

func (a *App) WorkbenchGetCookies(profileID string) ([]map[string]interface{}, error) {
	profile, err := a.runningProfileForWorkbench(profileID)
	if err != nil {
		return nil, err
	}
	conn, err := behavior.ConnectPageCDP(profile.DebugPort)
	if err != nil {
		return nil, fmt.Errorf("connect CDP: %w", err)
	}
	defer conn.Close()

	raw, err := behavior.ExecuteCDP(conn, "Network.getAllCookies", nil)
	if err != nil {
		return nil, fmt.Errorf("get cookies: %w", err)
	}

	var resp struct {
		Cookies []map[string]interface{} `json:"cookies"`
	}
	if err := json.Unmarshal(raw, &resp); err != nil {
		return nil, fmt.Errorf("parse cookies: %w", err)
	}
	return resp.Cookies, nil
}

func (a *App) WorkbenchSetCookie(profileID string, cookie map[string]interface{}) error {
	profile, err := a.runningProfileForWorkbench(profileID)
	if err != nil {
		return err
	}
	conn, err := behavior.ConnectPageCDP(profile.DebugPort)
	if err != nil {
		return fmt.Errorf("connect CDP: %w", err)
	}
	defer conn.Close()

	_, err = behavior.ExecuteCDP(conn, "Network.setCookie", cookie)
	return err
}

func (a *App) WorkbenchClearCookies(profileID string) error {
	profile, err := a.runningProfileForWorkbench(profileID)
	if err != nil {
		return err
	}
	conn, err := behavior.ConnectPageCDP(profile.DebugPort)
	if err != nil {
		return fmt.Errorf("connect CDP: %w", err)
	}
	defer conn.Close()

	_, err = behavior.ExecuteCDP(conn, "Network.clearBrowserCookies", nil)
	return err
}

// ─── Tab Operations ───────────────────────────────────────────────────────────

func (a *App) WorkbenchListTabs(profileID string) ([]browser.Tab, error) {
	profile, err := a.runningProfileForWorkbench(profileID)
	if err != nil {
		return nil, err
	}

	cdpTargets, err := behavior.ListCDPTargets(profile.DebugPort)
	if err != nil {
		return nil, fmt.Errorf("list targets: %w", err)
	}

	tabs := make([]browser.Tab, 0, len(cdpTargets))
	for _, t := range cdpTargets {
		tabs = append(tabs, browser.Tab{
			TabId:  t.ID,
			Title:  t.Title,
			Url:    t.URL,
			Active: t.Active,
		})
	}
	return tabs, nil
}

func (a *App) WorkbenchSwitchTab(profileID string, tabID string) error {
	profile, err := a.runningProfileForWorkbench(profileID)
	if err != nil {
		return err
	}
	conn, err := behavior.ConnectPageCDP(profile.DebugPort)
	if err != nil {
		return fmt.Errorf("connect CDP: %w", err)
	}
	defer conn.Close()

	_, err = behavior.ExecuteCDP(conn, "Target.activateTarget", map[string]interface{}{
		"targetId": tabID,
	})
	if err != nil {
		return err
	}
	resetCDPExecutorsForPort(profile.DebugPort)
	return nil
}

func (a *App) WorkbenchCloseTab(profileID string, tabID string) error {
	profile, err := a.runningProfileForWorkbench(profileID)
	if err != nil {
		return err
	}
	conn, err := behavior.ConnectPageCDP(profile.DebugPort)
	if err != nil {
		return fmt.Errorf("connect CDP: %w", err)
	}
	defer conn.Close()

	_, err = behavior.ExecuteCDP(conn, "Target.closeTarget", map[string]interface{}{
		"targetId": tabID,
	})
	return err
}

func (a *App) WorkbenchNewTab(profileID string, url string) (string, error) {
	profile, err := a.runningProfileForWorkbench(profileID)
	if err != nil {
		return "", err
	}
	conn, err := behavior.ConnectPageCDP(profile.DebugPort)
	if err != nil {
		return "", fmt.Errorf("connect CDP: %w", err)
	}
	defer conn.Close()

	params := map[string]interface{}{"url": "about:blank"}
	if url != "" {
		params["url"] = url
	}
	raw, err := behavior.ExecuteCDP(conn, "Target.createTarget", params)
	if err != nil {
		return "", fmt.Errorf("create target: %w", err)
	}

	var resp struct {
		TargetID string `json:"targetId"`
	}
	if err := json.Unmarshal(raw, &resp); err != nil {
		return "", fmt.Errorf("parse create target: %w", err)
	}
	return resp.TargetID, nil
}

// ─── Storage Operations ──────────────────────────────────────────────────────

func (a *App) WorkbenchGetLocalStorage(profileID string) (map[string]string, error) {
	return a.workbenchGetWebStorage(profileID, "localStorage")
}

func (a *App) WorkbenchSetLocalStorage(profileID string, items map[string]string) error {
	return a.workbenchSetWebStorage(profileID, "localStorage", items)
}

func (a *App) WorkbenchGetSessionStorage(profileID string) (map[string]string, error) {
	return a.workbenchGetWebStorage(profileID, "sessionStorage")
}

func (a *App) WorkbenchSetSessionStorage(profileID string, items map[string]string) error {
	return a.workbenchSetWebStorage(profileID, "sessionStorage", items)
}

func (a *App) workbenchGetWebStorage(profileID string, storageName string) (map[string]string, error) {
	storageName = strings.TrimSpace(storageName)
	if storageName != "localStorage" && storageName != "sessionStorage" {
		return nil, fmt.Errorf("unsupported storage %q", storageName)
	}
	profile, err := a.runningProfileForWorkbench(profileID)
	if err != nil {
		return nil, err
	}
	conn, err := behavior.ConnectPageCDP(profile.DebugPort)
	if err != nil {
		return nil, fmt.Errorf("connect CDP: %w", err)
	}
	defer conn.Close()

	expression := fmt.Sprintf(`JSON.stringify(Object.assign({}, window.%s || {}))`, storageName)
	raw, err := behavior.ExecuteCDP(conn, "Runtime.evaluate", map[string]interface{}{
		"expression":    expression,
		"returnByValue": true,
	})
	if err != nil {
		return nil, fmt.Errorf("get %s: %w", storageName, err)
	}

	value := runtimeEvaluateStringValue(raw)
	if strings.TrimSpace(value) == "" {
		return map[string]string{}, nil
	}
	var items map[string]string
	if err := json.Unmarshal([]byte(value), &items); err != nil {
		return nil, fmt.Errorf("parse %s: %w", storageName, err)
	}
	if items == nil {
		items = map[string]string{}
	}
	return items, nil
}

func (a *App) workbenchSetWebStorage(profileID string, storageName string, items map[string]string) error {
	storageName = strings.TrimSpace(storageName)
	if storageName != "localStorage" && storageName != "sessionStorage" {
		return fmt.Errorf("unsupported storage %q", storageName)
	}
	profile, err := a.runningProfileForWorkbench(profileID)
	if err != nil {
		return err
	}
	conn, err := behavior.ConnectPageCDP(profile.DebugPort)
	if err != nil {
		return fmt.Errorf("connect CDP: %w", err)
	}
	defer conn.Close()

	payload, err := json.Marshal(items)
	if err != nil {
		return fmt.Errorf("marshal %s items: %w", storageName, err)
	}
	js := fmt.Sprintf(`(function(items){
		var store = window.%s;
		if (!store) return false;
		Object.keys(items || {}).forEach(function(key) {
			store.setItem(String(key), String(items[key]));
		});
		return true;
	})(%s)`, storageName, string(payload))
	raw, err := behavior.ExecuteCDP(conn, "Runtime.evaluate", map[string]interface{}{
		"expression":    js,
		"returnByValue": true,
	})
	if err != nil {
		return fmt.Errorf("set %s: %w", storageName, err)
	}
	if strings.TrimSpace(runtimeEvaluateStringValue(raw)) == "false" {
		return fmt.Errorf("%s is not available", storageName)
	}
	return nil
}

func runtimeEvaluateStringValue(raw json.RawMessage) string {
	var direct struct {
		Value interface{} `json:"value"`
	}
	if err := json.Unmarshal(raw, &direct); err == nil && direct.Value != nil {
		return fmt.Sprint(direct.Value)
	}
	var nested struct {
		Result struct {
			Value interface{} `json:"value"`
		} `json:"result"`
	}
	if err := json.Unmarshal(raw, &nested); err == nil && nested.Result.Value != nil {
		return fmt.Sprint(nested.Result.Value)
	}
	return ""
}

// ─── Behavior Engine Operations ──────────────────────────────────────────────

func (a *App) WorkbenchBehaviorStart(profileID string, presetID string) error {
	profile, err := a.runningProfileForWorkbench(profileID)
	if err != nil {
		return err
	}

	preset := behavior.GetPreset(presetID)
	if preset == nil {
		preset = behavior.GetPreset("office-worker")
	}
	if preset == nil {
		presets := behavior.BuiltinPresets()
		if len(presets) > 0 {
			preset = &presets[0]
		}
	}
	if preset == nil {
		return fmt.Errorf("no behavior preset available")
	}

	a.behaviorEnginesMu.Lock()
	defer a.behaviorEnginesMu.Unlock()

	if existing, ok := a.behaviorEngines[profileID]; ok {
		existing.Stop()
	}

	engine := behavior.NewEngine(profile.DebugPort, preset)
	ctx := a.ctx
	if ctx == nil {
		ctx = nil
	}
	if err := engine.Start(ctx); err != nil {
		return fmt.Errorf("start behavior engine: %w", err)
	}
	if a.behaviorEngines == nil {
		a.behaviorEngines = make(map[string]*behavior.Engine)
	}
	a.behaviorEngines[profileID] = engine
	return nil
}

func (a *App) WorkbenchBehaviorStop(profileID string) error {
	a.behaviorEnginesMu.Lock()
	defer a.behaviorEnginesMu.Unlock()

	engine, ok := a.behaviorEngines[profileID]
	if !ok {
		return fmt.Errorf("no active behavior engine for profile %s", profileID)
	}
	engine.Stop()
	delete(a.behaviorEngines, profileID)
	return nil
}

func (a *App) WorkbenchBehaviorConfig(profileID string, intensity float64) error {
	// Currently intensity is only read at engine creation time.
	// For dynamic adjustment, we would need to update the running engine's profile.
	// For now, restarting with different intensity requires stopping and starting again.
	return nil
}

// ─── Nurture Operations ──────────────────────────────────────────────────────

func (a *App) WorkbenchNurtureStart(profileID string, behaviorPreset string) error {
	profile, err := a.runningProfileForWorkbench(profileID)
	if err != nil {
		return err
	}

	conn, err := behavior.ConnectPageCDP(profile.DebugPort)
	if err != nil {
		return fmt.Errorf("connect CDP: %w", err)
	}

	// Start nurture in a goroutine
	go func() {
		defer conn.Close()
		executor := behavior.NewCDPExecutor(conn, humanize.DefaultConfig())
		if executor == nil {
			return
		}
		_ = executor.SimulateNaturalBrowsing(60 * time.Second)
	}()

	return nil
}

func (a *App) WorkbenchNurtureStop(profileID string) error {
	// Nurture is fire-and-forget for now; it runs for a fixed duration.
	return nil
}

// ─── Proxy Operations ─────────────────────────────────────────────────────────

func (a *App) WorkbenchCheckProxy(profileID string) (*browser.FingerprintHealthProfile, error) {
	profile, err := a.runningProfileForWorkbench(profileID)
	if err != nil {
		return nil, err
	}
	// Use the fingerprint health as a proxy check (it already checks if expected
	// fingerprint args including proxy settings are being applied correctly).
	fingerprint, err := browser.ExtractFingerprint(profile.DebugPort)
	if err != nil {
		return nil, fmt.Errorf("extract fingerprint: %w", err)
	}

	return browser.NewFingerprintHealthProfile(profile.ProfileId, profile.FingerprintArgs, fingerprint, time.Now()), nil
}

func (a *App) WorkbenchProxySpeedtest(profileID string) (map[string]interface{}, error) {
	profile, err := a.runningProfileForWorkbench(profileID)
	if err != nil {
		return nil, err
	}

	result := map[string]interface{}{
		"profileId":   profileID,
		"proxyConfig": profile.ProxyConfig,
		"tested":      false,
	}

	// If there's no proxy configured, skip test
	if strings.TrimSpace(profile.ProxyConfig) == "" &&
		strings.TrimSpace(profile.ProxyId) == "" {
		result["reason"] = "no proxy configured"
		return result, nil
	}

	// Try to use the proxy speed testing infrastructure
	proxyConfig := strings.TrimSpace(profile.ProxyConfig)
	if proxyConfig == "" && profile.ProxyId != "" && a.browserMgr != nil {
		if cfg, ok := a.browserMgr.GetProxyConfigById(profile.ProxyId); ok {
			proxyConfig = cfg
		}
	}

	result["config"] = proxyConfig
	result["tested"] = false
	result["reason"] = "speed test requires external proxy managers (xray/singbox)"
	return result, nil
}

// GetLaunchServerInfo 返回 LaunchServer 的当前监听信息（Wails 绑定）
func (a *App) GetLaunchServerInfo() map[string]interface{} {
	preferredPort := 0
	authRequested := false
	authConfigured := false
	authEnabled := false
	authHeader := launchcode.DefaultAPIKeyHeader
	if a.config != nil {
		preferredPort = a.config.LaunchServer.Port
		authRequested = a.config.LaunchServer.Auth.Enabled
		authConfigured = a.config.LaunchServer.Auth.APIKey != ""
		if header := a.config.LaunchServer.Auth.Header; header != "" {
			authHeader = header
		}
	}

	actualPort := 0
	if a.launchServer != nil {
		actualPort = a.launchServer.Port()
		authRequested = a.launchServer.APIAuthRequested()
		authConfigured = a.launchServer.APIAuthConfigured()
		authEnabled = a.launchServer.APIAuthEnabled()
		authHeader = a.launchServer.APIAuthHeader()
	}

	info := map[string]interface{}{
		"host":          "127.0.0.1",
		"preferredPort": preferredPort,
		"port":          actualPort,
		"ready":         actualPort > 0,
		"apiAuth": map[string]interface{}{
			"requested":  authRequested,
			"configured": authConfigured,
			"enabled":    authEnabled,
			"header":     authHeader,
		},
	}
	if actualPort > 0 {
		info["baseUrl"] = fmt.Sprintf("http://127.0.0.1:%d", actualPort)
		info["cdpUrl"] = fmt.Sprintf("http://127.0.0.1:%d", actualPort)
		if a.launchServer != nil {
			info["activeDebugPort"] = a.launchServer.ActiveDebugPort()
		}
	} else {
		info["baseUrl"] = ""
		info["cdpUrl"] = ""
		info["activeDebugPort"] = 0
	}
	return info
}

// ─── CDP Executor Connection Pool ──────────────────────────────────────────────

type cdpExecutorEntry struct {
	executor   *behavior.CDPExecutor
	refCount   int
	mu         sync.Mutex
	lastUsedAt time.Time
}

var (
	cdpExecutorPool    sync.Map
	cdpPoolCleanupOnce sync.Once
)

// cdpPoolEntryTimeout defines how long an idle entry (refCount==0) is kept.
const cdpPoolEntryTimeout = 30 * time.Second

// parseHumanizationLevel parses a string level name into a HumanizationLevel.
func parseHumanizationLevel(level string) humanize.HumanizationLevel {
	if level == "" {
		return humanize.LevelHigh
	}
	if v, ok := humanizationLevelMap[strings.ToLower(strings.TrimSpace(level))]; ok {
		return v
	}
	return humanize.LevelHigh
}

func (a *App) validateWorkbenchScriptPolicy(profile *BrowserProfile, actions []launchcode.ActionRequest) error {
	if profileHasTag(profile, "allow-script-bypass") {
		return nil
	}
	for _, action := range actions {
		if !strings.EqualFold(strings.TrimSpace(action.Type), "script") {
			continue
		}
		level := parseHumanizationLevel(action.HumanizationLevel)
		if len(actions) > 0 && action.HumanizationLevel == "" {
			level = parseHumanizationLevel(actions[0].HumanizationLevel)
		}
		if level <= humanize.LevelHigh {
			return fmt.Errorf("script actions forbidden (tier≤T3): use humanized type/click; add profile tag allow-script-bypass to override")
		}
	}
	return nil
}

// startCDPPoolCleanup starts a background goroutine that periodically removes
// stale entries from the connection pool.
func startCDPPoolCleanup() {
	cdpPoolCleanupOnce.Do(func() {
		go func() {
			ticker := time.NewTicker(15 * time.Second)
			defer ticker.Stop()
			for range ticker.C {
				cdpExecutorPool.Range(func(key, value interface{}) bool {
					entry := value.(*cdpExecutorEntry)
					entry.mu.Lock()
					if entry.refCount <= 0 && time.Since(entry.lastUsedAt) > cdpPoolEntryTimeout {
						if entry.executor != nil {
							_ = entry.executor.Close()
						}
						cdpExecutorPool.Delete(key)
					}
					entry.mu.Unlock()
					return true
				})
			}
		}()
	})
}

func cdpPoolKey(debugPort int, tabID string) string {
	tabID = strings.TrimSpace(tabID)
	if tabID == "" {
		return fmt.Sprintf("%d", debugPort)
	}
	return fmt.Sprintf("%d:%s", debugPort, tabID)
}

func resetCDPExecutorsForPort(debugPort int) {
	prefix := fmt.Sprintf("%d", debugPort)
	cdpExecutorPool.Range(func(key, value interface{}) bool {
		keyStr, _ := key.(string)
		if keyStr == prefix || strings.HasPrefix(keyStr, prefix+":") {
			entry := value.(*cdpExecutorEntry)
			entry.mu.Lock()
			if entry.executor != nil {
				_ = entry.executor.Close()
			}
			entry.mu.Unlock()
			cdpExecutorPool.Delete(key)
		}
		return true
	})
}

func acquireCDPExecutor(debugPort int, tabID string, humanizationLevel string, pid int, humanizeSeed string) (*behavior.CDPExecutor, error) {
	key := cdpPoolKey(debugPort, tabID)
	entryI, _ := cdpExecutorPool.LoadOrStore(key, &cdpExecutorEntry{})
	entry := entryI.(*cdpExecutorEntry)

	entry.mu.Lock()
	defer entry.mu.Unlock()

	level := parseHumanizationLevel(humanizationLevel)
	if entry.executor == nil || !entry.executor.IsConnected() {
		ws, err := behavior.ConnectPageCDPForTarget(debugPort, tabID)
		if err != nil {
			return nil, fmt.Errorf("connect CDP on port %d: %w", debugPort, err)
		}
		cfg := humanize.ConfigForLevel(level)
		entry.executor = behavior.NewCDPExecutor(ws, cfg)
		bindOSClickFallback(entry.executor, pid, humanizeSeed)
		if err := entry.executor.EnableMinimalSession(); err != nil {
			_ = entry.executor.Close()
			entry.executor = nil
			return nil, fmt.Errorf("cdp minimal session on port %d: %w", debugPort, err)
		}
	} else {
		currentLevel := entry.executor.HumanizationLevel()
		if currentLevel != level {
			cfg := humanize.ConfigForLevel(level)
			entry.executor.UpdateHumanizationConfig(cfg)
		}
	}
	entry.refCount++
	entry.lastUsedAt = time.Now()
	return entry.executor, nil
}

func releaseCDPExecutor(debugPort int, tabID string) {
	key := cdpPoolKey(debugPort, tabID)
	entryI, ok := cdpExecutorPool.Load(key)
	if !ok {
		return
	}
	entry := entryI.(*cdpExecutorEntry)

	entry.mu.Lock()
	defer entry.mu.Unlock()

	entry.refCount--
	entry.lastUsedAt = time.Now()
	if entry.refCount <= 0 {
		if entry.executor != nil {
			_ = entry.executor.Close()
		}
		cdpExecutorPool.Delete(key)
	}
}

// cdpPoolEntryForTest returns the entry for the given port, for testing only.
func cdpPoolEntryForTest(debugPort int) *cdpExecutorEntry {
	entryI, ok := cdpExecutorPool.Load(cdpPoolKey(debugPort, ""))
	if !ok {
		return nil
	}
	return entryI.(*cdpExecutorEntry)
}

// WorkbenchExecuteActions 执行批量动作序列，通过连接池共享 CDP 连接。
func (a *App) WorkbenchExecuteActions(profileID string, actions []launchcode.ActionRequest) (results []launchcode.ActionResult, err error) {
	profile, err := a.runningProfileForWorkbench(profileID)
	if err != nil {
		return nil, err
	}
	if err := a.validateWorkbenchScriptPolicy(profile, actions); err != nil {
		return nil, err
	}
	log := logger.New("WorkbenchActions")

	startCDPPoolCleanup()

	defaultLevel := ""
	if len(actions) > 0 {
		defaultLevel = actions[0].HumanizationLevel
	}

	var currentTabID string
	var executor *behavior.CDPExecutor

	defer func() {
		if r := recover(); r != nil {
			log.Error("panic in WorkbenchExecuteActions", logger.F("recover", fmt.Sprintf("%v", r)))
			if results == nil {
				results = make([]launchcode.ActionResult, 0)
			}
			err = fmt.Errorf("internal panic: %v", r)
		}
		if executor != nil {
			releaseCDPExecutor(profile.DebugPort, currentTabID)
		}
	}()

	results = make([]launchcode.ActionResult, 0, len(actions))
	for _, action := range actions {
		tabID := strings.TrimSpace(action.TabID)
		level := strings.TrimSpace(action.HumanizationLevel)
		if level == "" {
			level = defaultLevel
		}
		if executor != nil && tabID != currentTabID {
			releaseCDPExecutor(profile.DebugPort, currentTabID)
			executor = nil
		}
		if executor == nil {
			ex, acquireErr := acquireCDPExecutor(profile.DebugPort, tabID, level, profile.Pid, profile.HumanizeSeed)
			if acquireErr != nil {
				return results, acquireErr
			}
			executor = ex
			currentTabID = tabID
		}

		result := launchcode.ActionResult{Type: action.Type}

		// pre-wait
		if action.PreWaitMs > 0 {
			time.Sleep(time.Duration(action.PreWaitMs) * time.Millisecond)
		}

		// wait-for-selector
		if action.WaitForSelector != "" {
			timeout := time.Duration(action.WaitTimeoutMs) * time.Millisecond
			if timeout <= 0 {
				timeout = 5 * time.Second
			}
			if err := executor.WaitForSelector(action.WaitForSelector, timeout); err != nil {
				result.OK = false
				result.Error = err.Error()
				result.ErrorCode = string(launchcode.ErrSelectorTimeout)
				results = append(results, result)
				continue
			}
		}

		// pre-wait for selector using the selector itself
		if action.Type != "wait" && action.Selector != "" && action.WaitTimeoutMs > 0 {
			timeout := time.Duration(action.WaitTimeoutMs) * time.Millisecond
			_ = executor.WaitForSelector(action.Selector, timeout)
		}

		// dispatch by type
		if err := a.dispatchAction(executor, profile, &action, &result); err != nil {
			result.OK = false
			result.Error = err.Error()
			result.ErrorCode = string(launchcode.ErrActionFailed)
		} else {
			result.OK = true
		}

		// collect page info on success
		if result.OK {
			if url, err := executor.GetPageURL(); err == nil {
				result.PageURL = url
			}
			if title, err := executor.GetPageTitle(); err == nil {
				result.PageTitle = title
			}
			if browser.IsNavigationErrorPage(result.PageURL) {
				result.OK = false
				result.Error = "navigation landed on browser error page: " + result.PageURL
				result.ErrorCode = "navigation_error_page"
			}
		}

		// post-wait
		if action.PostWaitMs > 0 {
			time.Sleep(time.Duration(action.PostWaitMs) * time.Millisecond)
		}

		results = append(results, result)
		if !result.OK {
			log.Warn("action failed", logger.F("type", action.Type), logger.F("error", result.Error))
		}
	}

	// Recording integration: store action batch if recording is active
	if a != nil {
		_ = a.StoreActionBatch(profileID, actions, results)
	}

	return results, nil
}

func (a *App) dispatchAction(executor *behavior.CDPExecutor, profile *BrowserProfile, action *launchcode.ActionRequest, result *launchcode.ActionResult) error {
	if action == nil {
		return fmt.Errorf("nil action")
	}

	switch strings.ToLower(strings.TrimSpace(action.Type)) {
	case "click":
		return a.dispatchClickAction(executor, profile, action, result)

	case "type", "text":
		return a.dispatchTypeAction(executor, profile, action, result)

	case "scroll":
		distance := action.Distance
		if distance == 0 {
			distance = 500
		}
		direction := strings.ToLower(strings.TrimSpace(action.Direction))
		if direction == "" {
			direction = "down"
		}
		return executor.ExecuteHumanizedScrollDir(distance, direction)

	case "hover":
		if action.Selector == "" {
			return fmt.Errorf("selector is required for hover")
		}
		if action.Frame != "" {
			return executor.HoverElementInFrame(action.Frame, action.Selector)
		}
		return executor.HoverElement(action.Selector)

	case "double-click":
		return a.dispatchDoubleClickAction(executor, profile, action, result)

	case "right-click":
		return a.dispatchRightClickAction(executor, profile, action, result)

	case "screenshot":
		val, err := executor.CaptureScreenshot()
		if err != nil {
			return err
		}
		result.Value = val
		return nil

	case "wait":
		duration := time.Duration(action.Distance) * time.Millisecond
		if duration <= 0 {
			duration = 1 * time.Second
		}
		return executor.Wait(duration)

	case "navigate":
		if action.URL == "" {
			return fmt.Errorf("url is required for navigate")
		}
		return executor.Navigate(action.URL)

	case "script":
		if action.Script == "" {
			return fmt.Errorf("script is required for script action")
		}
		val, err := executor.EvaluateJS(action.Script)
		if err != nil {
			return err
		}
		result.Value = val
		return nil

	case "click-offset":
		return a.dispatchClickOffsetAction(executor, profile, action, result)

	case "wait-for-selector":
		if action.Selector == "" {
			return fmt.Errorf("selector is required for wait-for-selector")
		}
		timeout := time.Duration(action.WaitTimeoutMs) * time.Millisecond
		if timeout <= 0 {
			timeout = 5 * time.Second
		}
		if action.Frame != "" {
			return executor.WaitForSelectorInFrame(action.Frame, action.Selector, timeout)
		}
		return executor.WaitForSelector(action.Selector, timeout)

	case "wait-for-visible":
		if action.Selector == "" {
			return fmt.Errorf("selector is required for wait-for-visible")
		}
		timeout := time.Duration(action.WaitTimeoutMs) * time.Millisecond
		if timeout <= 0 {
			timeout = 5 * time.Second
		}
		if action.Frame != "" {
			return executor.WaitForSelectorVisibleInFrame(action.Frame, action.Selector, timeout)
		}
		return executor.WaitForSelectorVisible(action.Selector, timeout)

	case "get-info":
		url, _ := executor.GetPageURL()
		title, _ := executor.GetPageTitle()
		result.PageURL = url
		result.PageTitle = title
		return nil

	default:
		return fmt.Errorf("unknown action type: %s", action.Type)
	}
}

// 确保编译器检查 App 实现了 BrowserStarter 接口
var _ launchcode.BrowserStarter = (*App)(nil)
var _ launchcode.BrowserStarterWithParams = (*App)(nil)
var _ launchcode.BrowserStopper = (*App)(nil)
var _ launchcode.WorkbenchOperator = (*App)(nil)
