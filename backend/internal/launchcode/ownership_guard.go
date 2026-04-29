package launchcode

import (
	"fmt"

	"ant-chrome/backend/internal/browser"
)

func (s *LaunchServer) validateActiveCDPOwnership(profileID string, debugPort int, audit *browser.LaunchAuditSnapshot) error {
	if audit == nil {
		return fmt.Errorf("CDP ownership rejected: launch audit snapshot is missing for profile %s", profileID)
	}

	expected := browser.LaunchAuditExpectation{
		ProfileID: profileID,
		DebugPort: debugPort,
	}
	if err := browser.ValidateLaunchAuditSnapshot(audit, expected); err != nil {
		return err
	}

	if s.browserMgr == nil {
		return nil
	}
	s.browserMgr.Mutex.Lock()
	profile := s.browserMgr.Profiles[profileID]
	var snapshot *browser.Profile
	if profile != nil {
		copied := *profile
		snapshot = &copied
	}
	s.browserMgr.Mutex.Unlock()
	if snapshot == nil {
		return fmt.Errorf("CDP ownership rejected: active profile %s is not found", profileID)
	}
	return s.browserMgr.ValidateProfileLaunchAudit(snapshot)
}
