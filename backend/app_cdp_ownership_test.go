package backend

import (
	"strings"
	"testing"

	"ant-chrome/backend/internal/browser"
	"ant-chrome/backend/internal/config"
)

func TestWorkbenchRejectsReachableDebugPortWithoutLaunchAudit(t *testing.T) {
	t.Parallel()

	ln := mustListenLoopback(t)
	defer ln.Close()

	app := NewApp(t.TempDir())
	app.config = config.DefaultConfig()
	app.browserMgr = browser.NewManager(app.config, app.appRoot)
	app.browserMgr.Profiles["profile-missing-audit"] = &BrowserProfile{
		ProfileId:   "profile-missing-audit",
		ProfileName: "Missing Audit",
		Running:     true,
		DebugReady:  true,
		DebugPort:   listenerPort(t, ln),
		Pid:         4101,
	}

	_, err := app.runningProfileForWorkbench("profile-missing-audit")
	if err == nil {
		t.Fatal("expected reachable debug port without launch audit to be rejected")
	}
	if !strings.Contains(err.Error(), "CDP ownership rejected") ||
		!strings.Contains(err.Error(), "launch audit snapshot") {
		t.Fatalf("unexpected error: %v", err)
	}
}

func TestWorkbenchRejectsReachableDebugPortWithAuditProfileMismatch(t *testing.T) {
	t.Parallel()

	ln := mustListenLoopback(t)
	defer ln.Close()

	port := listenerPort(t, ln)
	app := NewApp(t.TempDir())
	app.config = config.DefaultConfig()
	app.browserMgr = browser.NewManager(app.config, app.appRoot)
	app.browserMgr.Profiles["profile-a"] = &BrowserProfile{
		ProfileId:   "profile-a",
		ProfileName: "Profile A",
		Running:     true,
		DebugReady:  true,
		DebugPort:   port,
		Pid:         4102,
		LaunchAudit: &browser.LaunchAuditSnapshot{
			BrowserExe:           `C:\test\chrome.exe`,
			ProfileID:            "profile-b",
			CanonicalUserDataDir: `C:\test\profiles\profile-a`,
			ProxyHash:            "proxy-hash",
			FingerprintArgsHash:  "fingerprint-hash",
			LaunchArgsHash:       "launch-args-hash",
			DebugPort:            port,
			PID:                  4102,
			Timestamp:            "2026-04-29T00:00:00Z",
			AppMode:              "test",
		},
	}

	_, err := app.runningProfileForWorkbench("profile-a")
	if err == nil {
		t.Fatal("expected profileId mismatch in launch audit to be rejected")
	}
	if !strings.Contains(err.Error(), "profileId mismatch") {
		t.Fatalf("unexpected error: %v", err)
	}
}
