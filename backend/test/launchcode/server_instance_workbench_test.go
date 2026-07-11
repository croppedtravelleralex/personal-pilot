package launchcode_test

import (
	"bytes"
	"encoding/json"
	"errors"
	"net/http"
	"net/http/httptest"
	"testing"
	"time"

	"personal-pilot/backend/internal/browser"
	"personal-pilot/backend/internal/launchcode"
)

type instanceWorkbenchStarter struct {
	profiles       map[string]*browser.Profile
	stopErr        error
	stopped        []string
	navigated      []string
	refreshed      []string
	screenshotted  []string
	activated      []string
	arranged       []string
	arrangeLayout  string
	workbenchErr   error
	screenshotData string
}

func newInstanceWorkbenchStarter() *instanceWorkbenchStarter {
	return &instanceWorkbenchStarter{
		profiles:       make(map[string]*browser.Profile),
		screenshotData: "data:image/jpeg;base64,ZmFrZQ==",
	}
}

func (s *instanceWorkbenchStarter) addProfile(profile *browser.Profile) {
	s.profiles[profile.ProfileId] = profile
}

func (s *instanceWorkbenchStarter) StartInstance(profileId string) (*browser.Profile, error) {
	profile, ok := s.profiles[profileId]
	if !ok {
		return nil, errors.New("profile not found")
	}
	profile.Running = true
	return profile, nil
}

func (s *instanceWorkbenchStarter) StopInstance(profileId string) (*browser.Profile, error) {
	if s.stopErr != nil {
		return nil, s.stopErr
	}
	profile, ok := s.profiles[profileId]
	if !ok {
		return nil, errors.New("profile not found")
	}
	profile.Running = false
	profile.DebugReady = false
	s.stopped = append(s.stopped, profileId)
	return profile, nil
}

func (s *instanceWorkbenchStarter) WorkbenchNavigateProfile(profileId string, rawURL string) error {
	if s.workbenchErr != nil {
		return s.workbenchErr
	}
	s.navigated = append(s.navigated, profileId+" "+rawURL)
	return nil
}

func (s *instanceWorkbenchStarter) WorkbenchRefreshProfile(profileId string) error {
	if s.workbenchErr != nil {
		return s.workbenchErr
	}
	s.refreshed = append(s.refreshed, profileId)
	return nil
}

func (s *instanceWorkbenchStarter) WorkbenchCaptureScreenshot(profileId string) (string, error) {
	if s.workbenchErr != nil {
		return "", s.workbenchErr
	}
	s.screenshotted = append(s.screenshotted, profileId)
	return s.screenshotData, nil
}

func (s *instanceWorkbenchStarter) WorkbenchCaptureFullReport(profileId string) (*launchcode.WorkbenchFullReport, error) {
	if s.workbenchErr != nil {
		return nil, s.workbenchErr
	}
	return &launchcode.WorkbenchFullReport{
		ProfileID:      profileId,
		URL:            "https://example.test",
		Title:          "Example",
		HTML:           "<html><body>Example</body></html>",
		Text:           "Example",
		Tabs:           []browser.Tab{},
		Cookies:        []map[string]interface{}{},
		LocalStorage:   map[string]string{},
		SessionStorage: map[string]string{},
		Failures:       []launchcode.WorkbenchReportFailure{},
		CapturedAt:     time.Now().UTC().Format(time.RFC3339Nano),
		Source:         "test",
	}, nil
}

func (s *instanceWorkbenchStarter) WorkbenchFingerprintProfile(profileId string) (*browser.FingerprintSnapshot, error) {
	if s.workbenchErr != nil {
		return nil, s.workbenchErr
	}
	return &browser.FingerprintSnapshot{
		UserAgent:           "test-agent",
		Platform:            "Win32",
		HardwareConcurrency: 8,
		Timezone:            "Asia/Shanghai",
		Language:            "zh-CN",
	}, nil
}

func (s *instanceWorkbenchStarter) WorkbenchFingerprintHealthProfile(profileId string) (*browser.FingerprintHealthProfile, error) {
	if s.workbenchErr != nil {
		return nil, s.workbenchErr
	}
	return browser.NewFingerprintHealthProfile(profileId, nil, &browser.FingerprintSnapshot{
		UserAgent:           "test-agent Chrome",
		Platform:            "Win32",
		HardwareConcurrency: 8,
		ScreenWidth:         1920,
		ScreenHeight:        1080,
		AvailWidth:          1920,
		AvailHeight:         1040,
		Timezone:            "Asia/Shanghai",
		Language:            "zh-CN",
		CanvasHash:          "abc123",
		WebGLVendor:         "Google Inc.",
		WebGLRenderer:       "ANGLE",
		FontHash:            "10.00,20.00",
	}, time.Now()), nil
}

func (s *instanceWorkbenchStarter) WorkbenchActivateProfile(profileId string) error {
	if s.workbenchErr != nil {
		return s.workbenchErr
	}
	s.activated = append(s.activated, profileId)
	return nil
}

func (s *instanceWorkbenchStarter) WorkbenchClickElement(_ string, _ string) error {
	if s.workbenchErr != nil {
		return s.workbenchErr
	}
	return nil
}

func (s *instanceWorkbenchStarter) WorkbenchTypeText(_ string, _ string, _ string) error {
	if s.workbenchErr != nil {
		return s.workbenchErr
	}
	return nil
}

func (s *instanceWorkbenchStarter) WorkbenchScrollPage(_ string, _ uint32) error {
	if s.workbenchErr != nil {
		return s.workbenchErr
	}
	return nil
}

func (s *instanceWorkbenchStarter) WorkbenchShowMousePointer(_ string) error {
	return s.workbenchErr
}

func (s *instanceWorkbenchStarter) WorkbenchHideMousePointer(_ string) error {
	return s.workbenchErr
}

func (s *instanceWorkbenchStarter) WorkbenchExecuteActions(profileID string, actions []launchcode.ActionRequest) ([]launchcode.ActionResult, error) {
	if s.workbenchErr != nil {
		return nil, s.workbenchErr
	}
	results := make([]launchcode.ActionResult, 0, len(actions))
	for _, action := range actions {
		result := launchcode.ActionResult{Type: action.Type, OK: true}
		switch action.Type {
		case "navigate":
			if err := s.WorkbenchNavigateProfile(profileID, action.URL); err != nil {
				return nil, err
			}
			result.PageURL = action.URL
			result.PageTitle = "Test Page"
		case "refresh":
			if err := s.WorkbenchRefreshProfile(profileID); err != nil {
				return nil, err
			}
		case "screenshot":
			value, err := s.WorkbenchCaptureScreenshot(profileID)
			if err != nil {
				return nil, err
			}
			result.Value = value
		}
		results = append(results, result)
	}
	return results, nil
}

func (s *instanceWorkbenchStarter) WorkbenchArrangeProfiles(profileIds []string, layout string) ([]launchcode.WorkbenchWindowPlacement, error) {
	if s.workbenchErr != nil {
		return nil, s.workbenchErr
	}
	s.arranged = append(s.arranged, profileIds...)
	s.arrangeLayout = layout
	placements := make([]launchcode.WorkbenchWindowPlacement, 0, len(profileIds))
	for i, profileID := range profileIds {
		placements = append(placements, launchcode.WorkbenchWindowPlacement{
			ProfileID: profileID,
			Pid:       9000 + i,
			Found:     true,
			X:         i * 100,
			Y:         0,
			Width:     100,
			Height:    100,
		})
	}
	return placements, nil
}

// Extended WorkbenchOperator methods
func (s *instanceWorkbenchStarter) IdentityReportProfile(_ string) (*browser.IdentityStrengthReport, error) {
	return &browser.IdentityStrengthReport{Score: 90, Level: "strong"}, nil
}
func (s *instanceWorkbenchStarter) WorkbenchGetCookies(_ string) ([]map[string]interface{}, error) {
	return nil, nil
}
func (s *instanceWorkbenchStarter) WorkbenchSetCookie(_ string, _ map[string]interface{}) error {
	return nil
}
func (s *instanceWorkbenchStarter) WorkbenchClearCookies(_ string) error { return nil }
func (s *instanceWorkbenchStarter) WorkbenchListTabs(_ string) ([]browser.Tab, error) {
	return []browser.Tab{}, nil
}
func (s *instanceWorkbenchStarter) WorkbenchSwitchTab(_ string, _ string) error { return nil }
func (s *instanceWorkbenchStarter) WorkbenchCloseTab(_ string, _ string) error  { return nil }
func (s *instanceWorkbenchStarter) WorkbenchNewTab(_ string, _ string) (string, error) {
	return "new-tab-id", nil
}
func (s *instanceWorkbenchStarter) WorkbenchGetLocalStorage(_ string) (map[string]string, error) {
	return map[string]string{}, nil
}
func (s *instanceWorkbenchStarter) WorkbenchSetLocalStorage(_ string, _ map[string]string) error {
	return nil
}
func (s *instanceWorkbenchStarter) WorkbenchGetSessionStorage(_ string) (map[string]string, error) {
	return map[string]string{}, nil
}
func (s *instanceWorkbenchStarter) WorkbenchSetSessionStorage(_ string, _ map[string]string) error {
	return nil
}
func (s *instanceWorkbenchStarter) WorkbenchBehaviorStart(_ string, _ string) error   { return nil }
func (s *instanceWorkbenchStarter) WorkbenchBehaviorStop(_ string) error              { return nil }
func (s *instanceWorkbenchStarter) WorkbenchBehaviorConfig(_ string, _ float64) error { return nil }
func (s *instanceWorkbenchStarter) WorkbenchNurtureStart(_ string, _ string) error    { return nil }
func (s *instanceWorkbenchStarter) WorkbenchNurtureStop(_ string) error               { return nil }
func (s *instanceWorkbenchStarter) WorkbenchCheckProxy(_ string) (*browser.FingerprintHealthProfile, error) {
	return nil, nil
}
func (s *instanceWorkbenchStarter) WorkbenchProxySpeedtest(_ string) (map[string]interface{}, error) {
	return map[string]interface{}{}, nil
}

func TestInstanceStopAPI(t *testing.T) {
	t.Run("success", func(t *testing.T) {
		starter := newInstanceWorkbenchStarter()
		starter.addProfile(&browser.Profile{ProfileId: "profile-1", ProfileName: "one", Running: true, DebugReady: true, DebugPort: 9333})
		handler := buildTestHandler(newInMemoryService(), starter)

		req := httptest.NewRequest(http.MethodPost, "/api/instances/stop", bytes.NewBufferString(`{"profileId":"profile-1"}`))
		req.Header.Set("Content-Type", "application/json")
		w := httptest.NewRecorder()
		handler.ServeHTTP(w, req)

		if w.Code != http.StatusOK {
			t.Fatalf("expected 200, got %d body=%s", w.Code, w.Body.String())
		}
		if len(starter.stopped) != 1 || starter.stopped[0] != "profile-1" {
			t.Fatalf("stop not called: %+v", starter.stopped)
		}
		var resp struct {
			OK        bool   `json:"ok"`
			Stopped   bool   `json:"stopped"`
			ProfileID string `json:"profileId"`
		}
		if err := json.NewDecoder(w.Body).Decode(&resp); err != nil {
			t.Fatalf("decode response: %v", err)
		}
		if !resp.OK || !resp.Stopped || resp.ProfileID != "profile-1" {
			t.Fatalf("bad response: %+v", resp)
		}
	})

	t.Run("missing-profile-id", func(t *testing.T) {
		handler := buildTestHandler(newInMemoryService(), newInstanceWorkbenchStarter())
		req := httptest.NewRequest(http.MethodPost, "/api/instances/stop", bytes.NewBufferString(`{"profileId":""}`))
		w := httptest.NewRecorder()
		handler.ServeHTTP(w, req)
		if w.Code != http.StatusBadRequest {
			t.Fatalf("expected 400, got %d", w.Code)
		}
	})

	t.Run("stopper-unavailable", func(t *testing.T) {
		handler := buildTestHandler(newInMemoryService(), newMockStarterWithParams())
		req := httptest.NewRequest(http.MethodPost, "/api/instances/stop", bytes.NewBufferString(`{"profileId":"profile-1"}`))
		w := httptest.NewRecorder()
		handler.ServeHTTP(w, req)
		if w.Code != http.StatusServiceUnavailable {
			t.Fatalf("expected 503, got %d", w.Code)
		}
	})

	t.Run("backend-error", func(t *testing.T) {
		starter := newInstanceWorkbenchStarter()
		starter.stopErr = errors.New("stop failed")
		handler := buildTestHandler(newInMemoryService(), starter)
		req := httptest.NewRequest(http.MethodPost, "/api/instances/stop", bytes.NewBufferString(`{"profileId":"profile-1"}`))
		w := httptest.NewRecorder()
		handler.ServeHTTP(w, req)
		if w.Code != http.StatusInternalServerError {
			t.Fatalf("expected 500, got %d", w.Code)
		}
	})
}

func TestWorkbenchHTTPAPI(t *testing.T) {
	t.Run("navigate-refresh-screenshot-arrange", func(t *testing.T) {
		starter := newInstanceWorkbenchStarter()
		handler := buildTestHandler(newInMemoryService(), starter)

		postJSON(t, handler, "/api/workbench/navigate", `{"profileId":"profile-1","url":"https://example.com"}`, http.StatusOK)
		postJSON(t, handler, "/api/workbench/refresh", `{"profileId":"profile-1"}`, http.StatusOK)
		wScreenshot := postJSON(t, handler, "/api/workbench/screenshot", `{"profileId":"profile-1"}`, http.StatusOK)
		wHealth := postJSON(t, handler, "/api/workbench/fingerprint-health", `{"profileId":"profile-1"}`, http.StatusOK)
		wArrange := postJSON(t, handler, "/api/workbench/arrange", `{"profileIds":["profile-1","profile-2"],"layout":"grid"}`, http.StatusOK)

		if len(starter.navigated) != 1 || starter.navigated[0] != "profile-1 https://example.com" {
			t.Fatalf("navigate not called: %+v", starter.navigated)
		}
		if len(starter.refreshed) != 1 || len(starter.screenshotted) != 1 {
			t.Fatalf("refresh/screenshot not called: refreshed=%+v screenshots=%+v", starter.refreshed, starter.screenshotted)
		}
		if len(starter.arranged) != 2 || starter.arrangeLayout != "grid" {
			t.Fatalf("arrange not called: ids=%+v layout=%s", starter.arranged, starter.arrangeLayout)
		}

		var screenshotResp struct {
			OK         bool   `json:"ok"`
			Screenshot string `json:"screenshot"`
		}
		if err := json.NewDecoder(wScreenshot.Body).Decode(&screenshotResp); err != nil {
			t.Fatalf("decode screenshot: %v", err)
		}
		if !screenshotResp.OK || screenshotResp.Screenshot == "" {
			t.Fatalf("bad screenshot response: %+v", screenshotResp)
		}

		var healthResp struct {
			OK          bool                         `json:"ok"`
			Score       int                          `json:"score"`
			Level       string                       `json:"level"`
			Source      string                       `json:"source"`
			Fingerprint *browser.FingerprintSnapshot `json:"fingerprint"`
		}
		if err := json.NewDecoder(wHealth.Body).Decode(&healthResp); err != nil {
			t.Fatalf("decode fingerprint health: %v", err)
		}
		if !healthResp.OK || healthResp.Score <= 0 || healthResp.Level == "" || healthResp.Source != browser.FingerprintHealthSourceLocalCDP || healthResp.Fingerprint == nil {
			t.Fatalf("bad fingerprint health response: %+v", healthResp)
		}

		var arrangeResp struct {
			OK         bool                                  `json:"ok"`
			Placements []launchcode.WorkbenchWindowPlacement `json:"placements"`
		}
		if err := json.NewDecoder(wArrange.Body).Decode(&arrangeResp); err != nil {
			t.Fatalf("decode arrange: %v", err)
		}
		if !arrangeResp.OK || len(arrangeResp.Placements) != 2 {
			t.Fatalf("bad arrange response: %+v", arrangeResp)
		}
	})

	t.Run("workbench-unavailable", func(t *testing.T) {
		handler := buildTestHandler(newInMemoryService(), newMockStarterWithParams())
		postJSON(t, handler, "/api/workbench/refresh", `{"profileId":"profile-1"}`, http.StatusServiceUnavailable)
	})
}

func postJSON(t *testing.T, handler http.Handler, path string, body string, wantStatus int) *httptest.ResponseRecorder {
	t.Helper()
	req := httptest.NewRequest(http.MethodPost, path, bytes.NewBufferString(body))
	req.Header.Set("Content-Type", "application/json")
	w := httptest.NewRecorder()
	handler.ServeHTTP(w, req)
	if w.Code != wantStatus {
		t.Fatalf("%s expected %d, got %d body=%s", path, wantStatus, w.Code, w.Body.String())
	}
	return w
}
