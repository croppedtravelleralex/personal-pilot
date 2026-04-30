package backend

import (
	"fmt"
	"personal-pilot/backend/internal/browser"
	"personal-pilot/backend/internal/launchcode"
	"time"
)

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

// 确保编译器检查 App 实现了 BrowserStarter 接口
var _ launchcode.BrowserStarter = (*App)(nil)
var _ launchcode.BrowserStarterWithParams = (*App)(nil)
var _ launchcode.BrowserStopper = (*App)(nil)
var _ launchcode.WorkbenchOperator = (*App)(nil)
