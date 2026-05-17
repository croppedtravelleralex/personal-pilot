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
	return a.SynchronizerNavigateProfile(profileId, rawURL)
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
	profile, err := a.runningProfileForWorkbench(profileID)
	if err != nil {
		return err
	}
	executor, err := connectCDPExecutor(profile.DebugPort)
	if err != nil {
		return err
	}
	defer executor.Close()
	return executor.ExecuteHumanizedClick(selector)
}

func (a *App) WorkbenchTypeText(profileID string, selector string, text string) error {
	profile, err := a.runningProfileForWorkbench(profileID)
	if err != nil {
		return err
	}
	executor, err := connectCDPExecutor(profile.DebugPort)
	if err != nil {
		return err
	}
	defer executor.Close()
	return executor.ExecuteHumanizedType(selector, text)
}

func (a *App) WorkbenchScrollPage(profileID string, distance uint32) error {
	profile, err := a.runningProfileForWorkbench(profileID)
	if err != nil {
		return err
	}
	executor, err := connectCDPExecutor(profile.DebugPort)
	if err != nil {
		return err
	}
	defer executor.Close()
	return executor.ExecuteHumanizedScroll(distance)
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
	return err
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
	profile, err := a.runningProfileForWorkbench(profileID)
	if err != nil {
		return nil, err
	}
	conn, err := behavior.ConnectPageCDP(profile.DebugPort)
	if err != nil {
		return nil, fmt.Errorf("connect CDP: %w", err)
	}
	defer conn.Close()

	raw, err := behavior.ExecuteCDP(conn, "Runtime.evaluate", map[string]interface{}{
		"expression":    "JSON.stringify(window.localStorage)",
		"returnByValue": true,
	})
	if err != nil {
		return nil, fmt.Errorf("get localStorage: %w", err)
	}

	var resp struct {
		Result struct {
			Value string `json:"value"`
		} `json:"result"`
	}
	if err := json.Unmarshal(raw, &resp); err != nil || resp.Result.Value == "" {
		return map[string]string{}, nil
	}

	var items map[string]string
	if err := json.Unmarshal([]byte(resp.Result.Value), &items); err != nil {
		return nil, fmt.Errorf("parse localStorage: %w", err)
	}
	return items, nil
}

func (a *App) WorkbenchSetLocalStorage(profileID string, items map[string]string) error {
	profile, err := a.runningProfileForWorkbench(profileID)
	if err != nil {
		return err
	}
	conn, err := behavior.ConnectPageCDP(profile.DebugPort)
	if err != nil {
		return fmt.Errorf("connect CDP: %w", err)
	}
	defer conn.Close()

	for key, value := range items {
		escapedKey := strings.ReplaceAll(key, `\`, `\\`)
		escapedKey = strings.ReplaceAll(escapedKey, `"`, `\"`)
		escapedVal := strings.ReplaceAll(value, `\`, `\\`)
		escapedVal = strings.ReplaceAll(escapedVal, `"`, `\"`)
		js := fmt.Sprintf(`localStorage.setItem("%s","%s")`, escapedKey, escapedVal)
		_, err := behavior.ExecuteCDP(conn, "Runtime.evaluate", map[string]interface{}{
			"expression":    js,
			"returnByValue": true,
		})
		if err != nil {
			return fmt.Errorf("set localStorage %s: %w", key, err)
		}
	}
	return nil
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
		"profileId":     profileID,
		"proxyConfig":   profile.ProxyConfig,
		"tested":        false,
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
	cdpExecutorPool     sync.Map
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

func acquireCDPExecutor(debugPort int, humanizationLevel string) (*behavior.CDPExecutor, error) {
	entryI, _ := cdpExecutorPool.LoadOrStore(debugPort, &cdpExecutorEntry{})
	entry := entryI.(*cdpExecutorEntry)

	entry.mu.Lock()
	defer entry.mu.Unlock()

	level := parseHumanizationLevel(humanizationLevel)
	if entry.executor == nil || !entry.executor.IsConnected() {
		ws, err := behavior.ConnectPageCDP(debugPort)
		if err != nil {
			return nil, fmt.Errorf("connect CDP on port %d: %w", debugPort, err)
		}
		cfg := humanize.ConfigForLevel(level)
		entry.executor = behavior.NewCDPExecutor(ws, cfg)
	} else {
		// Update middleware config if level differs
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

func releaseCDPExecutor(debugPort int) {
	entryI, ok := cdpExecutorPool.Load(debugPort)
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
		cdpExecutorPool.Delete(debugPort)
	}
}

// cdpPoolEntryForTest returns the entry for the given port, for testing only.
func cdpPoolEntryForTest(debugPort int) *cdpExecutorEntry {
	entryI, ok := cdpExecutorPool.Load(debugPort)
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
	log := logger.New("WorkbenchActions")

	// Start pool cleanup on first use
	startCDPPoolCleanup()

	// Extract humanization level from the first action (all actions share one executor)
	humanizationLevel := ""
	if len(actions) > 0 {
		humanizationLevel = actions[0].HumanizationLevel
	}

	executor, err := acquireCDPExecutor(profile.DebugPort, humanizationLevel)
	if err != nil {
		return nil, err
	}

	// Leak protection: always release on exit, even on panic
	defer func() {
		if r := recover(); r != nil {
			log.Error("panic in WorkbenchExecuteActions", logger.F("recover", fmt.Sprintf("%v", r)))
			if results == nil {
				results = make([]launchcode.ActionResult, 0)
			}
			err = fmt.Errorf("internal panic: %v", r)
		}
		releaseCDPExecutor(profile.DebugPort)
	}()

	results = make([]launchcode.ActionResult, 0, len(actions))
	for _, action := range actions {
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
		if err := a.dispatchAction(executor, &action, &result); err != nil {
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

func (a *App) dispatchAction(executor *behavior.CDPExecutor, action *launchcode.ActionRequest, result *launchcode.ActionResult) error {
	if action == nil {
		return fmt.Errorf("nil action")
	}

	switch strings.ToLower(strings.TrimSpace(action.Type)) {
	case "click":
		if action.Selector == "" {
			return fmt.Errorf("selector is required for click")
		}
		if action.Frame != "" {
			return executor.ExecuteHumanizedClickInFrame(action.Frame, action.Selector)
		}
		return executor.ExecuteHumanizedClick(action.Selector)

	case "type", "text":
		if action.Selector == "" {
			return fmt.Errorf("selector is required for type")
		}
		clearFirst := true
		if action.ClearFirst != nil {
			clearFirst = *action.ClearFirst
		}
		if action.Frame != "" {
			return executor.ExecuteHumanizedTypeInFrame(action.Frame, action.Selector, action.Text, clearFirst, action.SubmitOnEnter)
		}
		return executor.ExecuteHumanizedTypeEx(action.Selector, action.Text, clearFirst, action.SubmitOnEnter)

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
		if action.Selector == "" {
			return fmt.Errorf("selector is required for double-click")
		}
		if action.Frame != "" {
			return executor.DoubleClickElementInFrame(action.Frame, action.Selector)
		}
		return executor.DoubleClickElement(action.Selector)

	case "right-click":
		if action.Selector == "" {
			return fmt.Errorf("selector is required for right-click")
		}
		if action.Frame != "" {
			return executor.RightClickElementInFrame(action.Frame, action.Selector)
		}
		return executor.RightClickElement(action.Selector)

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
		if action.Selector == "" {
			return fmt.Errorf("selector is required for click-offset")
		}
		offsetX := 0
		offsetY := 0
		if action.OffsetX != nil {
			offsetX = *action.OffsetX
		}
		if action.OffsetY != nil {
			offsetY = *action.OffsetY
		}
		if action.Frame != "" {
			return executor.ClickWithOffsetInFrame(action.Frame, action.Selector, offsetX, offsetY)
		}
		return executor.ClickWithOffset(action.Selector, offsetX, offsetY)

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
