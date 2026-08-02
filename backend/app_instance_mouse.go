package backend

import (
	"strings"

	"personal-pilot/backend/internal/browser"
	"personal-pilot/backend/internal/logger"
)

func (a *App) browserSettingsSnapshot() *browser.Settings {
	if a == nil {
		return nil
	}
	s := a.GetBrowserSettings()
	return &s
}

const showMousePointerLaunchArg = "--personal-pilot-show-mouse-pointer"

func profileWantsMousePointer(profile *BrowserProfile, settings *browser.Settings) bool {
	if profile == nil {
		return settings != nil && settings.ShowMousePointerDefault
	}
	for _, arg := range append(append([]string{}, profile.LaunchArgs...), profile.FingerprintArgs...) {
		if strings.EqualFold(strings.TrimSpace(arg), showMousePointerLaunchArg) {
			return true
		}
	}
	return settings != nil && settings.ShowMousePointerDefault
}

func (a *App) maybeShowMousePointerAsync(profileID string, debugPort int) {
	if a == nil || !profileWantsMousePointer(a.getProfileSnapshot(profileID), a.browserSettingsSnapshot()) {
		return
	}
	if err := a.WorkbenchShowMousePointer(profileID); err != nil {
		logger.New("Browser").Warn("auto mouse pointer overlay failed",
			logger.F("profile_id", profileID),
			logger.F("debug_port", debugPort),
			logger.F("error", err.Error()),
		)
	}
}

func (a *App) getProfileSnapshot(profileID string) *BrowserProfile {
	if a == nil || a.browserMgr == nil {
		return nil
	}
	a.browserMgr.Mutex.Lock()
	defer a.browserMgr.Mutex.Unlock()
	profile := a.browserMgr.Profiles[profileID]
	if profile == nil {
		return nil
	}
	copied := *profile
	return &copied
}
