package backend

import (
	"strings"
	"testing"

	"personal-pilot/backend/internal/browser"
	"personal-pilot/backend/internal/config"
	"personal-pilot/backend/internal/launchcode"
)

func TestWorkbenchRejectsScriptWithoutBypassTag(t *testing.T) {
	t.Parallel()

	app := NewApp(t.TempDir())
	app.config = config.DefaultConfig()
	app.browserMgr = browser.NewManager(app.config, app.appRoot)
	app.browserMgr.Profiles["profile-script"] = &BrowserProfile{
		ProfileId:      "profile-script",
		ProfileName:    "Script Gate",
		Running:        true,
		DebugReady:     true,
		InjectionReady: true,
		DebugPort:      9222,
		Pid:            1001,
		LaunchAudit: &browser.LaunchAuditSnapshot{
			ProfileID: "profile-script",
			DebugPort: 9222,
			PID:       1001,
		},
	}

	err := app.validateWorkbenchScriptPolicy(app.browserMgr.Profiles["profile-script"], []launchcode.ActionRequest{
		{Type: "script", Script: "document.title"},
	})
	if err == nil {
		t.Fatal("expected script action to be rejected")
	}
	if !strings.Contains(err.Error(), "forbidden") {
		t.Fatalf("unexpected error: %v", err)
	}
}

func TestWorkbenchAllowsScriptWithBypassTag(t *testing.T) {
	t.Parallel()

	profile := &BrowserProfile{
		ProfileId: "profile-bypass",
		Tags:      []string{"allow-script-bypass"},
	}
	app := NewApp(t.TempDir())
	if err := app.validateWorkbenchScriptPolicy(profile, []launchcode.ActionRequest{
		{Type: "script", Script: "1+1"},
	}); err != nil {
		t.Fatalf("expected bypass tag to allow script: %v", err)
	}
}

func TestWorkbenchRejectsBeforeInjectionReady(t *testing.T) {
	t.Parallel()

	err := profileInjectionNotReadyError(&BrowserProfile{
		ProfileName:    "No Inject",
		InjectionReady: false,
	})
	if err == nil {
		t.Fatal("expected injection gate to block workbench")
	}
	if !strings.Contains(err.Error(), "环境注入未就绪") {
		t.Fatalf("unexpected error: %v", err)
	}
}
