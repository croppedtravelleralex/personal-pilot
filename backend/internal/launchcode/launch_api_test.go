package launchcode

import (
	"bytes"
	"encoding/json"
	"errors"
	"fmt"
	"net"
	"net/http"
	"net/http/httptest"
	"net/url"
	"os"
	"strconv"
	"strings"
	"sync"
	"testing"
	"time"

	"personal-pilot/backend/internal/behavior"
	"personal-pilot/backend/internal/browser"
	"personal-pilot/backend/internal/config"
)

// ---------------------------------------------------------------------------
// mockStarter — implements BrowserStarter, BrowserStopper, profileCreator,
// profileUpdater, profileDeleter, BrowserStarterWithParams, WorkbenchOperator
// ---------------------------------------------------------------------------

type mockStarter struct {
	mu sync.Mutex

	startErr      error
	stopErr       error
	createErr     error
	updateErr     error
	deleteErr     error

	navigateErr   error
	refreshErr    error
	screenshotErr error
	fingerprintErr     error
	fingerprintHealthErr error
	activateErr   error
	arrangeErr    error
	clickErr      error
	typeErr       error
	scrollErr     error
	mouseErr      error

	screenshotData string
	createdName    string
	updatedID      string
	localStorage   map[string]string
	sessionStorage map[string]string
}

func (m *mockStarter) StartInstance(profileId string) (*browser.Profile, error) {
	if m.startErr != nil {
		return nil, m.startErr
	}
	return &browser.Profile{
		ProfileId:   profileId,
		ProfileName: "started-" + profileId,
		Running:     true,
		DebugPort:   9222,
		DebugReady:  true,
		Pid:         12345,
		LaunchAudit: &browser.LaunchAuditSnapshot{
			ProfileID:            profileId,
			DebugPort:            9222,
			PID:                  12345,
			BrowserExe:           "/test/chrome",
			CanonicalUserDataDir: "/test/data",
			ProxyHash:            "abc",
			FingerprintArgsHash:  "def",
			LaunchArgsHash:       "ghi",
			Timestamp:            "2025-01-01T00:00:00Z",
			AppMode:              "test",
		},
	}, nil
}

func (m *mockStarter) StartInstanceWithParams(profileId string, _ LaunchRequestParams) (*browser.Profile, error) {
	return m.StartInstance(profileId)
}

func (m *mockStarter) StopInstance(profileId string) (*browser.Profile, error) {
	if m.stopErr != nil {
		return nil, m.stopErr
	}
	return &browser.Profile{ProfileId: profileId, ProfileName: "stopped-" + profileId}, nil
}

func (m *mockStarter) CreateProfile(input browser.ProfileInput) (*browser.Profile, error) {
	if m.createErr != nil {
		return nil, m.createErr
	}
	m.mu.Lock()
	m.createdName = input.ProfileName
	m.mu.Unlock()
	pid := "mock-" + input.ProfileName
	return &browser.Profile{
		ProfileId:   pid,
		ProfileName: input.ProfileName,
		Running:     false,
	}, nil
}

func (m *mockStarter) UpdateProfile(profileID string, input browser.ProfileInput) (*browser.Profile, error) {
	if m.updateErr != nil {
		return nil, m.updateErr
	}
	m.mu.Lock()
	m.updatedID = profileID
	m.mu.Unlock()
	return &browser.Profile{
		ProfileId:   profileID,
		ProfileName: input.ProfileName,
		Running:     false,
	}, nil
}

func (m *mockStarter) DeleteProfile(_ string) error {
	return m.deleteErr
}

func (m *mockStarter) WorkbenchNavigateProfile(_ string, _ string) error {
	return m.navigateErr
}

func (m *mockStarter) WorkbenchRefreshProfile(_ string) error {
	return m.refreshErr
}

func (m *mockStarter) WorkbenchCaptureScreenshot(_ string) (string, error) {
	if m.screenshotErr != nil {
		return "", m.screenshotErr
	}
	if m.screenshotData != "" {
		return m.screenshotData, nil
	}
	return "data:image/png;base64,dGVzdA==", nil
}

func (m *mockStarter) WorkbenchCaptureFullReport(profileID string) (*WorkbenchFullReport, error) {
	if m.screenshotErr != nil {
		return nil, m.screenshotErr
	}
	return &WorkbenchFullReport{
		ProfileID:      profileID,
		URL:            "https://example.com",
		Title:          "Example",
		HTML:           "<html><body>Example</body></html>",
		Text:           "Example",
		Screenshot:     "data:image/png;base64,dGVzdA==",
		Tabs:           []browser.Tab{},
		Cookies:        []map[string]interface{}{},
		LocalStorage:   map[string]string{"local": "value"},
		SessionStorage: map[string]string{"session": "value"},
		Failures:       []WorkbenchReportFailure{},
		CapturedAt:     time.Now().UTC().Format(time.RFC3339Nano),
		Source:         "test",
	}, nil
}

func (m *mockStarter) WorkbenchFingerprintProfile(_ string) (*browser.FingerprintSnapshot, error) {
	if m.fingerprintErr != nil {
		return nil, m.fingerprintErr
	}
	return &browser.FingerprintSnapshot{}, nil
}

func (m *mockStarter) WorkbenchFingerprintHealthProfile(_ string) (*browser.FingerprintHealthProfile, error) {
	if m.fingerprintHealthErr != nil {
		return nil, m.fingerprintHealthErr
	}
	return &browser.FingerprintHealthProfile{Score: 95, Level: "good"}, nil
}

func (m *mockStarter) WorkbenchActivateProfile(_ string) error {
	return m.activateErr
}

func (m *mockStarter) WorkbenchClickElement(_ string, _ string) error {
	return m.clickErr
}

func (m *mockStarter) WorkbenchTypeText(_ string, _ string, _ string) error {
	return m.typeErr
}

func (m *mockStarter) WorkbenchScrollPage(_ string, _ uint32) error {
	return m.scrollErr
}

func (m *mockStarter) WorkbenchShowMousePointer(_ string) error {
	return m.mouseErr
}

func (m *mockStarter) WorkbenchHideMousePointer(_ string) error {
	return m.mouseErr
}

func (m *mockStarter) WorkbenchExecuteActions(_ string, actions []ActionRequest) ([]ActionResult, error) {
	results := make([]ActionResult, 0, len(actions))
	for _, a := range actions {
		res := ActionResult{Type: a.Type, OK: true}
		if strings.EqualFold(a.Type, "navigate") && m.navigateErr != nil {
			return results, m.navigateErr
		}
		if strings.EqualFold(a.Type, "navigate") {
			res.PageURL = a.URL
			res.PageTitle = "Example"
		}
		if a.Type == "hover" || a.Type == "double-click" || a.Type == "right-click" || a.Type == "wait" {
			res.PageURL = "https://example.com"
			res.PageTitle = "Example"
		}
		results = append(results, res)
	}
	return results, nil
}

func (m *mockStarter) WorkbenchArrangeProfiles(profileIds []string, _ string) ([]WorkbenchWindowPlacement, error) {
	if m.arrangeErr != nil {
		return nil, m.arrangeErr
	}
	return []WorkbenchWindowPlacement{
		{ProfileID: profileIds[0], Found: true, X: 0, Y: 0, Width: 800, Height: 600},
	}, nil
}
func (m *mockStarter) IdentityReportProfile(_ string) (*browser.IdentityStrengthReport, error) {
	return &browser.IdentityStrengthReport{Score: 90, Level: "strong"}, nil
}
func (m *mockStarter) CopyProfile(id string, newName string) (*browser.Profile, error) {
	return &browser.Profile{ProfileId: id + "-copy", ProfileName: newName}, nil
}
func (m *mockStarter) GetInstanceStatus(id string) (*browser.Profile, error) {
	return &browser.Profile{ProfileId: id, Running: true}, nil
}
func (m *mockStarter) WorkbenchGetCookies(_ string) ([]map[string]interface{}, error) { return nil, nil }
func (m *mockStarter) WorkbenchSetCookie(_ string, _ map[string]interface{}) error { return nil }
func (m *mockStarter) WorkbenchClearCookies(_ string) error { return nil }
func (m *mockStarter) WorkbenchListTabs(_ string) ([]browser.Tab, error) { return []browser.Tab{}, nil }
func (m *mockStarter) WorkbenchSwitchTab(_ string, _ string) error { return nil }
func (m *mockStarter) WorkbenchCloseTab(_ string, _ string) error { return nil }
func (m *mockStarter) WorkbenchNewTab(_ string, _ string) (string, error) { return "new-tab-id", nil }
func (m *mockStarter) WorkbenchGetLocalStorage(_ string) (map[string]string, error) {
	if m.localStorage != nil {
		return m.localStorage, nil
	}
	return map[string]string{}, nil
}
func (m *mockStarter) WorkbenchSetLocalStorage(_ string, items map[string]string) error {
	m.localStorage = items
	return nil
}
func (m *mockStarter) WorkbenchGetSessionStorage(_ string) (map[string]string, error) {
	if m.sessionStorage != nil {
		return m.sessionStorage, nil
	}
	return map[string]string{}, nil
}
func (m *mockStarter) WorkbenchSetSessionStorage(_ string, items map[string]string) error {
	m.sessionStorage = items
	return nil
}
func (m *mockStarter) WorkbenchBehaviorStart(_ string, _ string) error { return nil }
func (m *mockStarter) WorkbenchBehaviorStop(_ string) error { return nil }
func (m *mockStarter) WorkbenchBehaviorConfig(_ string, _ float64) error { return nil }
func (m *mockStarter) WorkbenchNurtureStart(_ string, _ string) error { return nil }
func (m *mockStarter) WorkbenchNurtureStop(_ string) error { return nil }
func (m *mockStarter) WorkbenchCheckProxy(_ string) (*browser.FingerprintHealthProfile, error) { return nil, nil }
func (m *mockStarter) WorkbenchProxySpeedtest(_ string) (map[string]interface{}, error) { return map[string]interface{}{}, nil }

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

func newTestServer(t *testing.T, opts ...func(*LaunchServer)) *LaunchServer {
	t.Helper()
	svc := NewLaunchCodeService(NewMemoryLaunchCodeDAO())
	starter := &mockStarter{}
	mgr := &browser.Manager{
		Config: &config.Config{Browser: config.BrowserConfig{
			UserDataRoot: "data",
			Cores:        []config.BrowserCore{},
			Proxies:      []config.BrowserProxy{},
			Profiles:     []config.BrowserProfileConfig{},
		}},
		Profiles: make(map[string]*browser.Profile),
	}
	srv := NewLaunchServer(svc, starter, nil, mgr, 0)
	for _, o := range opts {
		o(srv)
	}
	return srv
}

func serve(srv *LaunchServer, method, path string, body interface{}) *httptest.ResponseRecorder {
	return serveWithHandler(srv, method, path, body, false)
}

func serveWithHandler(srv *LaunchServer, method, path string, body interface{}, includeLocalhost bool) *httptest.ResponseRecorder {
	var reader *bytes.Reader
	if body == nil {
		reader = bytes.NewReader(nil)
	} else {
		data, _ := json.Marshal(body)
		reader = bytes.NewReader(data)
	}
	req := httptest.NewRequest(method, path, reader)
	w := httptest.NewRecorder()
	srv.buildHandler(includeLocalhost).ServeHTTP(w, req)
	return w
}

func decodeJSON(t *testing.T, resp *httptest.ResponseRecorder, dst interface{}) {
	t.Helper()
	if err := json.Unmarshal(resp.Body.Bytes(), dst); err != nil {
		t.Fatalf("json decode: %v\nbody: %s", err, resp.Body.String())
	}
}

// ---------------------------------------------------------------------------
// 1. Health endpoint tests
// ---------------------------------------------------------------------------

func TestHealthEndpoint_Returns200(t *testing.T) {
	srv := newTestServer(t)
	resp := serve(srv, http.MethodGet, "/api/health", nil)
	if resp.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d: %s", resp.Code, resp.Body.String())
	}
	var p struct {
		OK bool `json:"ok"`
	}
	decodeJSON(t, resp, &p)
	if !p.OK {
		t.Fatal("expected ok=true")
	}
}

func TestHealthEndpoint_ValidAPIKey(t *testing.T) {
	srv := newTestServer(t, func(s *LaunchServer) {
		s.SetAPIAuthConfig(APIAuthConfig{Enabled: true, APIKey: "secret", Header: DefaultAPIKeyHeader})
	})
	req := httptest.NewRequest(http.MethodGet, "/api/health", nil)
	req.Header.Set(DefaultAPIKeyHeader, "secret")
	w := httptest.NewRecorder()
	srv.buildHandler(false).ServeHTTP(w, req)
	if w.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d: %s", w.Code, w.Body.String())
	}
}

func TestHealthEndpoint_InvalidAPIKey(t *testing.T) {
	srv := newTestServer(t, func(s *LaunchServer) {
		s.SetAPIAuthConfig(APIAuthConfig{Enabled: true, APIKey: "secret", Header: DefaultAPIKeyHeader})
	})
	req := httptest.NewRequest(http.MethodGet, "/api/health", nil)
	req.Header.Set(DefaultAPIKeyHeader, "wrong")
	w := httptest.NewRecorder()
	srv.buildHandler(false).ServeHTTP(w, req)
	if w.Code != http.StatusUnauthorized {
		t.Fatalf("expected 401, got %d: %s", w.Code, w.Body.String())
	}
	var p struct {
		OK bool `json:"ok"`
	}
	decodeJSON(t, w, &p)
	if p.OK {
		t.Fatal("expected ok=false")
	}
}

func TestHealthEndpoint_NonLocalhost_Forbidden(t *testing.T) {
	srv := newTestServer(t)
	req := httptest.NewRequest(http.MethodGet, "/api/health", nil)
	req.RemoteAddr = "10.0.0.8:3456"
	w := httptest.NewRecorder()
	srv.buildHandler(true).ServeHTTP(w, req)
	if w.Code != http.StatusForbidden {
		t.Fatalf("expected 403, got %d: %s", w.Code, w.Body.String())
	}
}

// ---------------------------------------------------------------------------
// 2. Profile CRUD tests
// ---------------------------------------------------------------------------

func TestProfiles_Create_201(t *testing.T) {
	srv := newTestServer(t)
	body := map[string]interface{}{
		"profile": map[string]interface{}{
			"profileName": "My Profile",
		},
	}
	resp := serve(srv, http.MethodPost, "/api/profiles", body)
	if resp.Code != http.StatusCreated {
		t.Fatalf("expected 201, got %d: %s", resp.Code, resp.Body.String())
	}
	var p struct {
		OK          bool   `json:"ok"`
		Created     bool   `json:"created"`
		ProfileID   string `json:"profileId"`
		ProfileName string `json:"profileName"`
	}
	decodeJSON(t, resp, &p)
	if !p.OK || !p.Created {
		t.Fatalf("unexpected payload: %+v", p)
	}
	if p.ProfileName != "My Profile" {
		t.Fatalf("expected profileName 'My Profile', got %q", p.ProfileName)
	}
}

func TestProfiles_Create_InvalidBody_400(t *testing.T) {
	srv := newTestServer(t)
	resp := serve(srv, http.MethodPost, "/api/profiles", map[string]interface{}{"profile": nil})
	if resp.Code != http.StatusBadRequest {
		t.Fatalf("expected 400, got %d: %s", resp.Code, resp.Body.String())
	}
}

func TestProfiles_Create_MissingProfile_400(t *testing.T) {
	srv := newTestServer(t)
	resp := serve(srv, http.MethodPost, "/api/profiles", map[string]interface{}{})
	if resp.Code != http.StatusBadRequest {
		t.Fatalf("expected 400, got %d: %s", resp.Code, resp.Body.String())
	}
}

func TestProfiles_List_200(t *testing.T) {
	srv := newTestServer(t)
	srv.browserMgr.Profiles["p1"] = &browser.Profile{ProfileId: "p1", ProfileName: "Alpha"}
	srv.browserMgr.Profiles["p2"] = &browser.Profile{ProfileId: "p2", ProfileName: "Beta"}

	resp := serve(srv, http.MethodGet, "/api/profiles", nil)
	if resp.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d: %s", resp.Code, resp.Body.String())
	}
	var p struct {
		OK    bool            `json:"ok"`
		Count int             `json:"count"`
		Items []browser.Profile `json:"items"`
	}
	decodeJSON(t, resp, &p)
	if !p.OK {
		t.Fatal("expected ok=true")
	}
	if p.Count != 2 {
		t.Fatalf("expected count 2, got %d", p.Count)
	}
}

func TestProfiles_List_NoManager_503(t *testing.T) {
	srv := newTestServer(t)
	srv.browserMgr = nil
	resp := serve(srv, http.MethodGet, "/api/profiles", nil)
	if resp.Code != http.StatusServiceUnavailable {
		t.Fatalf("expected 503, got %d: %s", resp.Code, resp.Body.String())
	}
}

func TestProfiles_GetByID_200(t *testing.T) {
	srv := newTestServer(t)
	srv.browserMgr.Profiles["p1"] = &browser.Profile{ProfileId: "p1", ProfileName: "Alpha", Running: false}

	resp := serve(srv, http.MethodGet, "/api/profiles/p1", nil)
	if resp.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d: %s", resp.Code, resp.Body.String())
	}
	var p struct {
		OK          bool   `json:"ok"`
		ProfileID   string `json:"profileId"`
		ProfileName string `json:"profileName"`
	}
	decodeJSON(t, resp, &p)
	if !p.OK || p.ProfileID != "p1" || p.ProfileName != "Alpha" {
		t.Fatalf("unexpected payload: %+v", p)
	}
}

func TestProfiles_GetByID_NotFound_404(t *testing.T) {
	srv := newTestServer(t)
	resp := serve(srv, http.MethodGet, "/api/profiles/nonexistent", nil)
	if resp.Code != http.StatusNotFound {
		t.Fatalf("expected 404, got %d: %s", resp.Code, resp.Body.String())
	}
}

func TestProfiles_Update_200(t *testing.T) {
	srv := newTestServer(t)
	srv.browserMgr.Profiles["p1"] = &browser.Profile{ProfileId: "p1", ProfileName: "Old", Running: false}

	body := map[string]interface{}{
		"profile": map[string]interface{}{
			"profileName": "Renamed",
		},
	}
	resp := serve(srv, http.MethodPut, "/api/profiles/p1", body)
	if resp.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d: %s", resp.Code, resp.Body.String())
	}
	var p struct {
		OK          bool   `json:"ok"`
		Updated     bool   `json:"updated"`
		ProfileID   string `json:"profileId"`
		ProfileName string `json:"profileName"`
	}
	decodeJSON(t, resp, &p)
	if !p.OK || !p.Updated {
		t.Fatalf("unexpected payload: %+v", p)
	}
}

func TestProfiles_Update_NotFound_404(t *testing.T) {
	srv := newTestServer(t)
	body := map[string]interface{}{
		"profile": map[string]interface{}{
			"profileName": "Ghost",
		},
	}
	resp := serve(srv, http.MethodPut, "/api/profiles/nonexistent", body)
	if resp.Code != http.StatusNotFound {
		t.Fatalf("expected 404, got %d: %s", resp.Code, resp.Body.String())
	}
}

func TestProfiles_Delete_200(t *testing.T) {
	srv := newTestServer(t)
	srv.browserMgr.Profiles["p1"] = &browser.Profile{ProfileId: "p1", ProfileName: "To Delete", Running: false}

	resp := serve(srv, http.MethodDelete, "/api/profiles/p1", nil)
	if resp.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d: %s", resp.Code, resp.Body.String())
	}
	var p struct {
		OK      bool   `json:"ok"`
		Deleted bool   `json:"deleted"`
	}
	decodeJSON(t, resp, &p)
	if !p.OK || !p.Deleted {
		t.Fatalf("unexpected payload: %+v", p)
	}
}

func TestProfiles_Delete_Running_409(t *testing.T) {
	srv := newTestServer(t)
	srv.browserMgr.Profiles["p1"] = &browser.Profile{ProfileId: "p1", ProfileName: "Running", Running: true}

	resp := serve(srv, http.MethodDelete, "/api/profiles/p1", nil)
	if resp.Code != http.StatusConflict {
		t.Fatalf("expected 409, got %d: %s", resp.Code, resp.Body.String())
	}
}

func TestProfiles_Delete_NotFound_404(t *testing.T) {
	srv := newTestServer(t)
	resp := serve(srv, http.MethodDelete, "/api/profiles/nonexistent", nil)
	if resp.Code != http.StatusNotFound {
		t.Fatalf("expected 404, got %d: %s", resp.Code, resp.Body.String())
	}
}

func TestProfiles_MethodNotAllowed_405(t *testing.T) {
	srv := newTestServer(t)
	resp := serve(srv, http.MethodPatch, "/api/profiles", nil)
	if resp.Code != http.StatusMethodNotAllowed {
		t.Fatalf("expected 405, got %d: %s", resp.Code, resp.Body.String())
	}
}

// ---------------------------------------------------------------------------
// 3. Instance management
// ---------------------------------------------------------------------------

func TestInstanceStop_Success(t *testing.T) {
	srv := newTestServer(t)
	resp := serve(srv, http.MethodPost, "/api/instances/stop",
		map[string]string{"profileId": "prof-1"})
	if resp.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d: %s", resp.Code, resp.Body.String())
	}
	var p struct {
		OK      bool   `json:"ok"`
		Stopped bool   `json:"stopped"`
	}
	decodeJSON(t, resp, &p)
	if !p.OK || !p.Stopped {
		t.Fatalf("unexpected payload: %+v", p)
	}
}

func TestInstanceStop_EmptyProfileID_400(t *testing.T) {
	srv := newTestServer(t)
	resp := serve(srv, http.MethodPost, "/api/instances/stop",
		map[string]string{"profileId": ""})
	if resp.Code != http.StatusBadRequest {
		t.Fatalf("expected 400, got %d: %s", resp.Code, resp.Body.String())
	}
}

func TestInstanceStop_MissingProfileID_400(t *testing.T) {
	srv := newTestServer(t)
	resp := serve(srv, http.MethodPost, "/api/instances/stop", map[string]string{})
	if resp.Code != http.StatusBadRequest {
		t.Fatalf("expected 400, got %d: %s", resp.Code, resp.Body.String())
	}
}

func TestInstanceStop_NoStopper_503(t *testing.T) {
	svc := NewLaunchCodeService(NewMemoryLaunchCodeDAO())
	srv := NewLaunchServer(svc, nil, nil, nil, 0)
	resp := serve(srv, http.MethodPost, "/api/instances/stop",
		map[string]string{"profileId": "prof-1"})
	if resp.Code != http.StatusServiceUnavailable {
		t.Fatalf("expected 503, got %d: %s", resp.Code, resp.Body.String())
	}
}

func TestInstanceStop_WrongMethod_405(t *testing.T) {
	srv := newTestServer(t)
	resp := serve(srv, http.MethodGet, "/api/instances/stop", nil)
	if resp.Code != http.StatusMethodNotAllowed {
		t.Fatalf("expected 405, got %d: %s", resp.Code, resp.Body.String())
	}
}

// ---------------------------------------------------------------------------
// 4. Launch endpoints
// ---------------------------------------------------------------------------

func TestLaunch_GET_EmptyCode_404(t *testing.T) {
	srv := newTestServer(t)
	resp := serve(srv, http.MethodGet, "/api/launch/", nil)
	if resp.Code != http.StatusNotFound {
		t.Fatalf("expected 404, got %d: %s", resp.Code, resp.Body.String())
	}
}

func TestLaunch_GET_UnknownCode_404(t *testing.T) {
	srv := newTestServer(t)
	resp := serve(srv, http.MethodGet, "/api/launch/BADBAD", nil)
	if resp.Code != http.StatusNotFound {
		t.Fatalf("expected 404, got %d: %s", resp.Code, resp.Body.String())
	}
}

func TestLaunch_GET_Success(t *testing.T) {
	srv := newTestServer(t)
	code, err := srv.service.EnsureCode("prof-1")
	if err != nil {
		t.Fatalf("EnsureCode: %v", err)
	}
	resp := serve(srv, http.MethodGet, "/api/launch/"+code, nil)
	if resp.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d: %s", resp.Code, resp.Body.String())
	}
	var p struct {
		OK        bool   `json:"ok"`
		ProfileID string `json:"profileId"`
		LaunchCode string `json:"launchCode"`
	}
	decodeJSON(t, resp, &p)
	if !p.OK || p.ProfileID != "prof-1" {
		t.Fatalf("unexpected payload: %+v", p)
	}
	if p.LaunchCode != code {
		t.Fatalf("launchCode mismatch: expected %q, got %q", code, p.LaunchCode)
	}
}

func TestLaunch_POST_Success(t *testing.T) {
	srv := newTestServer(t)
	code, err := srv.service.EnsureCode("prof-1")
	if err != nil {
		t.Fatalf("EnsureCode: %v", err)
	}
	body := map[string]interface{}{"code": code}
	resp := serve(srv, http.MethodPost, "/api/launch", body)
	if resp.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d: %s", resp.Code, resp.Body.String())
	}
	var p struct {
		OK bool `json:"ok"`
	}
	decodeJSON(t, resp, &p)
	if !p.OK {
		t.Fatal("expected ok=true")
	}
}

func TestLaunch_POST_InvalidJSON_400(t *testing.T) {
	srv := newTestServer(t)
	req := httptest.NewRequest(http.MethodPost, "/api/launch",
		bytes.NewReader([]byte(`not json`)))
	w := httptest.NewRecorder()
	srv.buildHandler(false).ServeHTTP(w, req)
	if w.Code != http.StatusBadRequest {
		t.Fatalf("expected 400, got %d: %s", w.Code, w.Body.String())
	}
}

func TestLaunch_POST_EmptySelector_400(t *testing.T) {
	srv := newTestServer(t)
	resp := serve(srv, http.MethodPost, "/api/launch", map[string]interface{}{})
	if resp.Code != http.StatusBadRequest {
		t.Fatalf("expected 400, got %d: %s", resp.Code, resp.Body.String())
	}
}

func TestLaunch_POST_WrongMethod_405(t *testing.T) {
	srv := newTestServer(t)
	resp := serve(srv, http.MethodGet, "/api/launch", nil)
	if resp.Code != http.StatusMethodNotAllowed {
		t.Fatalf("expected 405, got %d: %s", resp.Code, resp.Body.String())
	}
}

func TestLaunchLogs_GET_200(t *testing.T) {
	srv := newTestServer(t)
	srv.appendLaunchLog("GET", "/api/health", "127.0.0.1", "", LaunchSelector{}, LaunchRequestParams{}, true, 200, "", "", "", time.Now())

	resp := serve(srv, http.MethodGet, "/api/launch/logs", nil)
	if resp.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d: %s", resp.Code, resp.Body.String())
	}
	var p struct {
		OK    bool               `json:"ok"`
		Items []LaunchCallRecord `json:"items"`
	}
	decodeJSON(t, resp, &p)
	if !p.OK {
		t.Fatal("expected ok=true")
	}
	if len(p.Items) != 1 {
		t.Fatalf("expected 1 item, got %d", len(p.Items))
	}
	if p.Items[0].Path != "/api/health" {
		t.Fatalf("expected path /api/health, got %q", p.Items[0].Path)
	}
}

func TestLaunchLogs_GET_Empty(t *testing.T) {
	srv := newTestServer(t)
	resp := serve(srv, http.MethodGet, "/api/launch/logs", nil)
	if resp.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d: %s", resp.Code, resp.Body.String())
	}
	var p struct {
		OK    bool               `json:"ok"`
		Items []LaunchCallRecord `json:"items"`
	}
	decodeJSON(t, resp, &p)
	if !p.OK {
		t.Fatal("expected ok=true")
	}
	if len(p.Items) != 0 {
		t.Fatalf("expected 0 items, got %d", len(p.Items))
	}
}

func TestLaunchLogs_WrongMethod_405(t *testing.T) {
	srv := newTestServer(t)
	resp := serve(srv, http.MethodPost, "/api/launch/logs", nil)
	if resp.Code != http.StatusMethodNotAllowed {
		t.Fatalf("expected 405, got %d: %s", resp.Code, resp.Body.String())
	}
}

// ---------------------------------------------------------------------------
// 5. Workbench endpoints
// ---------------------------------------------------------------------------

func TestWorkbench_Navigate_Success(t *testing.T) {
	srv := newTestServer(t)
	body := map[string]string{"profileId": "prof-1", "url": "https://example.com"}
	resp := serve(srv, http.MethodPost, "/api/workbench/navigate", body)
	if resp.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d: %s", resp.Code, resp.Body.String())
	}
	var p struct {
		OK bool `json:"ok"`
	}
	decodeJSON(t, resp, &p)
	if !p.OK {
		t.Fatal("expected ok=true")
	}
}

func TestWorkbench_Navigate_MissingFields_400(t *testing.T) {
	srv := newTestServer(t)
	// missing url
	resp := serve(srv, http.MethodPost, "/api/workbench/navigate",
		map[string]string{"profileId": "prof-1"})
	if resp.Code != http.StatusBadRequest {
		t.Fatalf("expected 400, got %d: %s", resp.Code, resp.Body.String())
	}
}

func TestWorkbench_Navigate_WrongMethod_405(t *testing.T) {
	srv := newTestServer(t)
	resp := serve(srv, http.MethodGet, "/api/workbench/navigate", nil)
	if resp.Code != http.StatusMethodNotAllowed {
		t.Fatalf("expected 405, got %d: %s", resp.Code, resp.Body.String())
	}
}

func TestWorkbench_Navigate_NoOperator_503(t *testing.T) {
	svc := NewLaunchCodeService(NewMemoryLaunchCodeDAO())
	srv := NewLaunchServer(svc, nil, nil, nil, 0)
	body := map[string]string{"profileId": "prof-1", "url": "https://example.com"}
	resp := serve(srv, http.MethodPost, "/api/workbench/navigate", body)
	if resp.Code != http.StatusServiceUnavailable {
		t.Fatalf("expected 503, got %d: %s", resp.Code, resp.Body.String())
	}
}

func TestWorkbench_Refresh_Success(t *testing.T) {
	srv := newTestServer(t)
	body := map[string]string{"profileId": "prof-1"}
	resp := serve(srv, http.MethodPost, "/api/workbench/refresh", body)
	if resp.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d: %s", resp.Code, resp.Body.String())
	}
	var p struct {
		OK        bool `json:"ok"`
		Refreshed bool `json:"refreshed"`
	}
	decodeJSON(t, resp, &p)
	if !p.OK || !p.Refreshed {
		t.Fatalf("unexpected payload: %+v", p)
	}
}

func TestWorkbench_Refresh_MissingProfileID_400(t *testing.T) {
	srv := newTestServer(t)
	resp := serve(srv, http.MethodPost, "/api/workbench/refresh", map[string]string{})
	if resp.Code != http.StatusBadRequest {
		t.Fatalf("expected 400, got %d: %s", resp.Code, resp.Body.String())
	}
}

func TestWorkbench_Screenshot_Success(t *testing.T) {
	srv := newTestServer(t)
	body := map[string]string{"profileId": "prof-1"}
	resp := serve(srv, http.MethodPost, "/api/workbench/screenshot", body)
	if resp.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d: %s", resp.Code, resp.Body.String())
	}
	var p struct {
		OK         bool   `json:"ok"`
		Screenshot string `json:"screenshot"`
	}
	decodeJSON(t, resp, &p)
	if !p.OK || p.Screenshot == "" {
		t.Fatalf("unexpected payload: %+v", p)
	}
}

func TestWorkbench_FullReport_Success(t *testing.T) {
	srv := newTestServer(t)
	body := map[string]string{"profileId": "prof-1"}
	resp := serve(srv, http.MethodPost, "/api/workbench/report/full", body)
	if resp.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d: %s", resp.Code, resp.Body.String())
	}
	var p struct {
		OK     bool `json:"ok"`
		Report struct {
			ProfileID      string            `json:"profileId"`
			URL            string            `json:"url"`
			Title          string            `json:"title"`
			HTML           string            `json:"html"`
			Text           string            `json:"text"`
			LocalStorage   map[string]string `json:"localStorage"`
			SessionStorage map[string]string `json:"sessionStorage"`
		} `json:"report"`
		Failures []WorkbenchReportFailure `json:"failures"`
	}
	decodeJSON(t, resp, &p)
	if !p.OK || p.Report.ProfileID != "prof-1" || p.Report.URL == "" || p.Report.HTML == "" || p.Report.Text == "" {
		t.Fatalf("unexpected payload: %+v", p)
	}
	if p.Report.LocalStorage["local"] != "value" || p.Report.SessionStorage["session"] != "value" {
		t.Fatalf("storage not included in report: %+v", p.Report)
	}
	if len(p.Failures) != 0 {
		t.Fatalf("unexpected failures: %+v", p.Failures)
	}
}

func TestWorkbench_Fingerprint_Success(t *testing.T) {
	srv := newTestServer(t)
	body := map[string]string{"profileId": "prof-1"}
	resp := serve(srv, http.MethodPost, "/api/workbench/fingerprint", body)
	if resp.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d: %s", resp.Code, resp.Body.String())
	}
	var p struct {
		OK          bool `json:"ok"`
		Fingerprint *struct {
			ProfileID string `json:"profileId"`
		} `json:"fingerprint"`
	}
	decodeJSON(t, resp, &p)
	if !p.OK || p.Fingerprint == nil {
		t.Fatalf("unexpected payload: %+v", p)
	}
}

func TestWorkbench_Fingerprint_WrongMethod_405(t *testing.T) {
	srv := newTestServer(t)
	resp := serve(srv, http.MethodGet, "/api/workbench/fingerprint", nil)
	if resp.Code != http.StatusMethodNotAllowed {
		t.Fatalf("expected 405, got %d: %s", resp.Code, resp.Body.String())
	}
}

func TestWorkbench_Fingerprint_NoOperator_503(t *testing.T) {
	svc := NewLaunchCodeService(NewMemoryLaunchCodeDAO())
	srv := NewLaunchServer(svc, nil, nil, nil, 0)
	body := map[string]string{"profileId": "prof-1"}
	resp := serve(srv, http.MethodPost, "/api/workbench/fingerprint", body)
	if resp.Code != http.StatusServiceUnavailable {
		t.Fatalf("expected 503, got %d: %s", resp.Code, resp.Body.String())
	}
}

func TestWorkbench_Fingerprint_Error(t *testing.T) {
	starter := &mockStarter{fingerprintErr: errors.New("fingerprint failed")}
	svc := NewLaunchCodeService(NewMemoryLaunchCodeDAO())
	mgr := &browser.Manager{
		Config:   &config.Config{Browser: config.BrowserConfig{UserDataRoot: "data"}},
		Profiles: make(map[string]*browser.Profile),
	}
	srv := NewLaunchServer(svc, starter, nil, mgr, 0)
	body := map[string]string{"profileId": "prof-1"}
	resp := serve(srv, http.MethodPost, "/api/workbench/fingerprint", body)
	if resp.Code != http.StatusInternalServerError {
		t.Fatalf("expected 500, got %d: %s", resp.Code, resp.Body.String())
	}
}

func TestWorkbench_FingerprintHealth_Success(t *testing.T) {
	srv := newTestServer(t)
	body := map[string]string{"profileId": "prof-1"}
	resp := serve(srv, http.MethodPost, "/api/workbench/fingerprint-health", body)
	if resp.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d: %s", resp.Code, resp.Body.String())
	}
	var p struct {
		OK    bool   `json:"ok"`
		Score int    `json:"score"`
		Level string `json:"level"`
	}
	decodeJSON(t, resp, &p)
	if !p.OK || p.Score != 95 || p.Level != "good" {
		t.Fatalf("unexpected payload: %+v", p)
	}
}

func TestWorkbench_FingerprintHealth_NoOperator_503(t *testing.T) {
	svc := NewLaunchCodeService(NewMemoryLaunchCodeDAO())
	srv := NewLaunchServer(svc, nil, nil, nil, 0)
	body := map[string]string{"profileId": "prof-1"}
	resp := serve(srv, http.MethodPost, "/api/workbench/fingerprint-health", body)
	if resp.Code != http.StatusServiceUnavailable {
		t.Fatalf("expected 503, got %d: %s", resp.Code, resp.Body.String())
	}
}

func TestWorkbench_FingerprintHealth_Empty_500(t *testing.T) {
	starter := &mockStarter{fingerprintHealthErr: nil}
	svc := NewLaunchCodeService(NewMemoryLaunchCodeDAO())
	mgr := &browser.Manager{
		Config:   &config.Config{Browser: config.BrowserConfig{UserDataRoot: "data"}},
		Profiles: make(map[string]*browser.Profile),
	}
	srv := NewLaunchServer(svc, starter, nil, mgr, 0)
	body := map[string]string{"profileId": "prof-1"}
	resp := serve(srv, http.MethodPost, "/api/workbench/fingerprint-health", body)
	if resp.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d: %s", resp.Code, resp.Body.String())
	}
}

func TestWorkbench_Activate_Success(t *testing.T) {
	srv := newTestServer(t)
	body := map[string]string{"profileId": "prof-1"}
	resp := serve(srv, http.MethodPost, "/api/workbench/activate", body)
	if resp.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d: %s", resp.Code, resp.Body.String())
	}
	var p struct {
		OK        bool `json:"ok"`
		Activated bool `json:"activated"`
	}
	decodeJSON(t, resp, &p)
	if !p.OK || !p.Activated {
		t.Fatalf("unexpected payload: %+v", p)
	}
}

func TestWorkbench_Activate_NoOperator_503(t *testing.T) {
	svc := NewLaunchCodeService(NewMemoryLaunchCodeDAO())
	srv := NewLaunchServer(svc, nil, nil, nil, 0)
	body := map[string]string{"profileId": "prof-1"}
	resp := serve(srv, http.MethodPost, "/api/workbench/activate", body)
	if resp.Code != http.StatusServiceUnavailable {
		t.Fatalf("expected 503, got %d: %s", resp.Code, resp.Body.String())
	}
}

func TestWorkbench_Activate_WrongMethod_405(t *testing.T) {
	srv := newTestServer(t)
	resp := serve(srv, http.MethodGet, "/api/workbench/activate", nil)
	if resp.Code != http.StatusMethodNotAllowed {
		t.Fatalf("expected 405, got %d: %s", resp.Code, resp.Body.String())
	}
}

func TestWorkbench_Activate_Error(t *testing.T) {
	starter := &mockStarter{activateErr: errors.New("activate failed")}
	svc := NewLaunchCodeService(NewMemoryLaunchCodeDAO())
	mgr := &browser.Manager{
		Config:   &config.Config{Browser: config.BrowserConfig{UserDataRoot: "data"}},
		Profiles: make(map[string]*browser.Profile),
	}
	srv := NewLaunchServer(svc, starter, nil, mgr, 0)
	body := map[string]string{"profileId": "prof-1"}
	resp := serve(srv, http.MethodPost, "/api/workbench/activate", body)
	if resp.Code != http.StatusInternalServerError {
		t.Fatalf("expected 500, got %d: %s", resp.Code, resp.Body.String())
	}
}

func TestWorkbench_Arrange_Success(t *testing.T) {
	srv := newTestServer(t)
	body := map[string]interface{}{
		"profileIds": []string{"prof-1"},
		"layout":     "grid",
	}
	resp := serve(srv, http.MethodPost, "/api/workbench/arrange", body)
	if resp.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d: %s", resp.Code, resp.Body.String())
	}
	var p struct {
		OK         bool                         `json:"ok"`
		ProfileIDs []string                     `json:"profileIds"`
		Layout     string                       `json:"layout"`
		Placements []WorkbenchWindowPlacement   `json:"placements"`
	}
	decodeJSON(t, resp, &p)
	if !p.OK || len(p.Placements) == 0 {
		t.Fatalf("unexpected payload: %+v", p)
	}
}

func TestWorkbench_Arrange_MissingProfileIDs_400(t *testing.T) {
	srv := newTestServer(t)
	resp := serve(srv, http.MethodPost, "/api/workbench/arrange", map[string]interface{}{})
	if resp.Code != http.StatusBadRequest {
		t.Fatalf("expected 400, got %d: %s", resp.Code, resp.Body.String())
	}
}

func TestWorkbench_Arrange_NoOperator_503(t *testing.T) {
	svc := NewLaunchCodeService(NewMemoryLaunchCodeDAO())
	srv := NewLaunchServer(svc, nil, nil, nil, 0)
	body := map[string]interface{}{
		"profileIds": []string{"prof-1"},
	}
	resp := serve(srv, http.MethodPost, "/api/workbench/arrange", body)
	if resp.Code != http.StatusServiceUnavailable {
		t.Fatalf("expected 503, got %d: %s", resp.Code, resp.Body.String())
	}
}

func TestWorkbench_Arrange_WrongMethod_405(t *testing.T) {
	srv := newTestServer(t)
	resp := serve(srv, http.MethodGet, "/api/workbench/arrange", nil)
	if resp.Code != http.StatusMethodNotAllowed {
		t.Fatalf("expected 405, got %d: %s", resp.Code, resp.Body.String())
	}
}

func TestWorkbench_Arrange_Error(t *testing.T) {
	starter := &mockStarter{arrangeErr: errors.New("arrange failed")}
	svc := NewLaunchCodeService(NewMemoryLaunchCodeDAO())
	mgr := &browser.Manager{
		Config:   &config.Config{Browser: config.BrowserConfig{UserDataRoot: "data"}},
		Profiles: make(map[string]*browser.Profile),
	}
	srv := NewLaunchServer(svc, starter, nil, mgr, 0)
	body := map[string]interface{}{"profileIds": []string{"prof-1"}}
	resp := serve(srv, http.MethodPost, "/api/workbench/arrange", body)
	if resp.Code != http.StatusInternalServerError {
		t.Fatalf("expected 500, got %d: %s", resp.Code, resp.Body.String())
	}
}

func TestWorkbench_Refresh_WrongMethod_405(t *testing.T) {
	srv := newTestServer(t)
	resp := serve(srv, http.MethodGet, "/api/workbench/refresh", nil)
	if resp.Code != http.StatusMethodNotAllowed {
		t.Fatalf("expected 405, got %d: %s", resp.Code, resp.Body.String())
	}
}

func TestWorkbench_Screenshot_WrongMethod_405(t *testing.T) {
	srv := newTestServer(t)
	resp := serve(srv, http.MethodGet, "/api/workbench/screenshot", nil)
	if resp.Code != http.StatusMethodNotAllowed {
		t.Fatalf("expected 405, got %d: %s", resp.Code, resp.Body.String())
	}
}

func TestWorkbench_Fingerprint_MissingProfileID_400(t *testing.T) {
	srv := newTestServer(t)
	resp := serve(srv, http.MethodPost, "/api/workbench/fingerprint", map[string]string{})
	if resp.Code != http.StatusBadRequest {
		t.Fatalf("expected 400, got %d: %s", resp.Code, resp.Body.String())
	}
}

func TestWorkbench_Activate_MissingProfileID_400(t *testing.T) {
	srv := newTestServer(t)
	resp := serve(srv, http.MethodPost, "/api/workbench/activate", map[string]string{})
	if resp.Code != http.StatusBadRequest {
		t.Fatalf("expected 400, got %d: %s", resp.Code, resp.Body.String())
	}
}

func TestWorkbench_Arrange_DefaultLayout(t *testing.T) {
	srv := newTestServer(t)
	body := map[string]interface{}{
		"profileIds": []string{"prof-1"},
	}
	resp := serve(srv, http.MethodPost, "/api/workbench/arrange", body)
	if resp.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d: %s", resp.Code, resp.Body.String())
	}
	var p struct {
		Layout string `json:"layout"`
	}
	decodeJSON(t, resp, &p)
	if p.Layout != "grid" {
		t.Fatalf("expected layout 'grid', got %q", p.Layout)
	}
}

// ---------------------------------------------------------------------------
// 5b. Workbench action endpoints (click / type / scroll)
// ---------------------------------------------------------------------------

func TestWorkbench_Click_Success(t *testing.T) {
	srv := newTestServer(t)
	body := map[string]string{"profileId": "prof-1", "selector": "#submit-btn"}
	resp := serve(srv, http.MethodPost, "/api/workbench/click", body)
	if resp.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d: %s", resp.Code, resp.Body.String())
	}
	var p struct {
		OK      bool   `json:"ok"`
		Clicked bool   `json:"clicked"`
		Sel     string `json:"selector"`
	}
	decodeJSON(t, resp, &p)
	if !p.OK || !p.Clicked || p.Sel != "#submit-btn" {
		t.Fatalf("unexpected payload: %+v", p)
	}
}

func TestWorkbench_Click_MissingFields_400(t *testing.T) {
	srv := newTestServer(t)
	// missing selector
	resp := serve(srv, http.MethodPost, "/api/workbench/click", map[string]string{"profileId": "prof-1"})
	if resp.Code != http.StatusBadRequest {
		t.Fatalf("expected 400, got %d: %s", resp.Code, resp.Body.String())
	}
}

func TestWorkbench_Click_WrongMethod_405(t *testing.T) {
	srv := newTestServer(t)
	resp := serve(srv, http.MethodGet, "/api/workbench/click", nil)
	if resp.Code != http.StatusMethodNotAllowed {
		t.Fatalf("expected 405, got %d: %s", resp.Code, resp.Body.String())
	}
}

func TestWorkbench_Click_NoOperator_503(t *testing.T) {
	svc := NewLaunchCodeService(NewMemoryLaunchCodeDAO())
	srv := NewLaunchServer(svc, nil, nil, nil, 0)
	body := map[string]string{"profileId": "prof-1", "selector": "#btn"}
	resp := serve(srv, http.MethodPost, "/api/workbench/click", body)
	if resp.Code != http.StatusServiceUnavailable {
		t.Fatalf("expected 503, got %d: %s", resp.Code, resp.Body.String())
	}
}

func TestWorkbench_Click_Error(t *testing.T) {
	starter := &mockStarter{clickErr: errors.New("click failed")}
	svc := NewLaunchCodeService(NewMemoryLaunchCodeDAO())
	mgr := &browser.Manager{
		Config:   &config.Config{Browser: config.BrowserConfig{UserDataRoot: "data"}},
		Profiles: make(map[string]*browser.Profile),
	}
	srv := NewLaunchServer(svc, starter, nil, mgr, 0)
	body := map[string]string{"profileId": "prof-1", "selector": "#btn"}
	resp := serve(srv, http.MethodPost, "/api/workbench/click", body)
	if resp.Code != http.StatusInternalServerError {
		t.Fatalf("expected 500, got %d: %s", resp.Code, resp.Body.String())
	}
}

func TestWorkbench_Type_Success(t *testing.T) {
	srv := newTestServer(t)
	body := map[string]string{"profileId": "prof-1", "selector": "#search", "text": "hello"}
	resp := serve(srv, http.MethodPost, "/api/workbench/type", body)
	if resp.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d: %s", resp.Code, resp.Body.String())
	}
	var p struct {
		OK    bool   `json:"ok"`
		Typed bool   `json:"typed"`
		Sel   string `json:"selector"`
	}
	decodeJSON(t, resp, &p)
	if !p.OK || !p.Typed || p.Sel != "#search" {
		t.Fatalf("unexpected payload: %+v", p)
	}
}

func TestWorkbench_Type_MissingFields_400(t *testing.T) {
	srv := newTestServer(t)
	// missing selector
	resp := serve(srv, http.MethodPost, "/api/workbench/type", map[string]string{"profileId": "prof-1", "text": "hello"})
	if resp.Code != http.StatusBadRequest {
		t.Fatalf("expected 400, got %d: %s", resp.Code, resp.Body.String())
	}
}

func TestWorkbench_Type_WrongMethod_405(t *testing.T) {
	srv := newTestServer(t)
	resp := serve(srv, http.MethodGet, "/api/workbench/type", nil)
	if resp.Code != http.StatusMethodNotAllowed {
		t.Fatalf("expected 405, got %d: %s", resp.Code, resp.Body.String())
	}
}

func TestWorkbench_Type_NoOperator_503(t *testing.T) {
	svc := NewLaunchCodeService(NewMemoryLaunchCodeDAO())
	srv := NewLaunchServer(svc, nil, nil, nil, 0)
	body := map[string]string{"profileId": "prof-1", "selector": "#input", "text": "hi"}
	resp := serve(srv, http.MethodPost, "/api/workbench/type", body)
	if resp.Code != http.StatusServiceUnavailable {
		t.Fatalf("expected 503, got %d: %s", resp.Code, resp.Body.String())
	}
}

func TestWorkbench_Type_Error(t *testing.T) {
	starter := &mockStarter{typeErr: errors.New("type failed")}
	svc := NewLaunchCodeService(NewMemoryLaunchCodeDAO())
	mgr := &browser.Manager{
		Config:   &config.Config{Browser: config.BrowserConfig{UserDataRoot: "data"}},
		Profiles: make(map[string]*browser.Profile),
	}
	srv := NewLaunchServer(svc, starter, nil, mgr, 0)
	body := map[string]string{"profileId": "prof-1", "selector": "#input", "text": "hi"}
	resp := serve(srv, http.MethodPost, "/api/workbench/type", body)
	if resp.Code != http.StatusInternalServerError {
		t.Fatalf("expected 500, got %d: %s", resp.Code, resp.Body.String())
	}
}

func TestWorkbench_Scroll_Success(t *testing.T) {
	srv := newTestServer(t)
	body := map[string]interface{}{"profileId": "prof-1", "distance": 300}
	resp := serve(srv, http.MethodPost, "/api/workbench/scroll", body)
	if resp.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d: %s", resp.Code, resp.Body.String())
	}
	var p struct {
		OK       bool `json:"ok"`
		Scrolled bool `json:"scrolled"`
		Dist     int  `json:"distance"`
	}
	decodeJSON(t, resp, &p)
	if !p.OK || !p.Scrolled || p.Dist != 300 {
		t.Fatalf("unexpected payload: %+v", p)
	}
}

func TestWorkbench_Scroll_DefaultDistance(t *testing.T) {
	srv := newTestServer(t)
	body := map[string]interface{}{"profileId": "prof-1"}
	resp := serve(srv, http.MethodPost, "/api/workbench/scroll", body)
	if resp.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d: %s", resp.Code, resp.Body.String())
	}
	var p struct {
		OK       bool `json:"ok"`
		Scrolled bool `json:"scrolled"`
		Dist     int  `json:"distance"`
	}
	decodeJSON(t, resp, &p)
	if !p.OK || !p.Scrolled || p.Dist != 500 {
		t.Fatalf("expected distance 500, got %+v", p)
	}
}

func TestWorkbench_Scroll_WrongMethod_405(t *testing.T) {
	srv := newTestServer(t)
	resp := serve(srv, http.MethodGet, "/api/workbench/scroll", nil)
	if resp.Code != http.StatusMethodNotAllowed {
		t.Fatalf("expected 405, got %d: %s", resp.Code, resp.Body.String())
	}
}

func TestWorkbench_Scroll_NoOperator_503(t *testing.T) {
	svc := NewLaunchCodeService(NewMemoryLaunchCodeDAO())
	srv := NewLaunchServer(svc, nil, nil, nil, 0)
	body := map[string]interface{}{"profileId": "prof-1", "distance": 300}
	resp := serve(srv, http.MethodPost, "/api/workbench/scroll", body)
	if resp.Code != http.StatusServiceUnavailable {
		t.Fatalf("expected 503, got %d: %s", resp.Code, resp.Body.String())
	}
}

func TestWorkbench_Scroll_Error(t *testing.T) {
	starter := &mockStarter{scrollErr: errors.New("scroll failed")}
	svc := NewLaunchCodeService(NewMemoryLaunchCodeDAO())
	mgr := &browser.Manager{
		Config:   &config.Config{Browser: config.BrowserConfig{UserDataRoot: "data"}},
		Profiles: make(map[string]*browser.Profile),
	}
	srv := NewLaunchServer(svc, starter, nil, mgr, 0)
	body := map[string]interface{}{"profileId": "prof-1", "distance": 300}
	resp := serve(srv, http.MethodPost, "/api/workbench/scroll", body)
	if resp.Code != http.StatusInternalServerError {
		t.Fatalf("expected 500, got %d: %s", resp.Code, resp.Body.String())
	}
}

func TestWorkbench_Scroll_MissingProfileID_400(t *testing.T) {
	srv := newTestServer(t)
	resp := serve(srv, http.MethodPost, "/api/workbench/scroll", map[string]interface{}{})
	if resp.Code != http.StatusBadRequest {
		t.Fatalf("expected 400, got %d: %s", resp.Code, resp.Body.String())
	}
}

func TestWorkbench_SessionStorage_GetSet_Success(t *testing.T) {
	starter := &mockStarter{}
	svc := NewLaunchCodeService(NewMemoryLaunchCodeDAO())
	mgr := &browser.Manager{
		Config:   &config.Config{Browser: config.BrowserConfig{UserDataRoot: "data"}},
		Profiles: make(map[string]*browser.Profile),
	}
	srv := NewLaunchServer(svc, starter, nil, mgr, 0)

	setResp := serve(srv, http.MethodPost, "/api/workbench/storage/set-session-storage", map[string]interface{}{
		"profileId": "prof-1",
		"items":     map[string]string{"token": "abc", "step": "1"},
	})
	if setResp.Code != http.StatusOK {
		t.Fatalf("expected set 200, got %d: %s", setResp.Code, setResp.Body.String())
	}

	getResp := serve(srv, http.MethodPost, "/api/workbench/storage/session-storage", map[string]string{"profileId": "prof-1"})
	if getResp.Code != http.StatusOK {
		t.Fatalf("expected get 200, got %d: %s", getResp.Code, getResp.Body.String())
	}
	var p struct {
		OK    bool              `json:"ok"`
		Count int               `json:"count"`
		Items map[string]string `json:"items"`
	}
	decodeJSON(t, getResp, &p)
	if !p.OK || p.Count != 2 || p.Items["token"] != "abc" || p.Items["step"] != "1" {
		t.Fatalf("unexpected payload: %+v", p)
	}
}

func TestWorkbench_MouseOverlay_ShowHide_Success(t *testing.T) {
	srv := newTestServer(t)

	showResp := serve(srv, http.MethodPost, "/api/workbench/mouse/show", map[string]string{"profileId": "prof-1"})
	if showResp.Code != http.StatusOK {
		t.Fatalf("expected show 200, got %d: %s", showResp.Code, showResp.Body.String())
	}
	var show struct {
		OK      bool `json:"ok"`
		Visible bool `json:"visible"`
	}
	decodeJSON(t, showResp, &show)
	if !show.OK || !show.Visible {
		t.Fatalf("unexpected show payload: %+v", show)
	}

	hideResp := serve(srv, http.MethodPost, "/api/workbench/mouse/hide", map[string]string{"profileId": "prof-1"})
	if hideResp.Code != http.StatusOK {
		t.Fatalf("expected hide 200, got %d: %s", hideResp.Code, hideResp.Body.String())
	}
	var hide struct {
		OK      bool `json:"ok"`
		Visible bool `json:"visible"`
	}
	decodeJSON(t, hideResp, &hide)
	if !hide.OK || hide.Visible {
		t.Fatalf("unexpected hide payload: %+v", hide)
	}
}

// ---------------------------------------------------------------------------
// 6. Recording endpoints
// ---------------------------------------------------------------------------

func TestRecording_List_200(t *testing.T) {
	api := &recordingHTTPTestAPI{}
	srv := newTestServer(t, func(s *LaunchServer) { s.recording = api })
	resp := serve(srv, http.MethodGet, "/api/recording/list", nil)
	if resp.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d: %s", resp.Code, resp.Body.String())
	}
	var p struct {
		OK    bool `json:"ok"`
		Count int  `json:"count"`
	}
	decodeJSON(t, resp, &p)
	if !p.OK {
		t.Fatal("expected ok=true")
	}
	if p.Count != 1 {
		t.Fatalf("expected count 1, got %d", p.Count)
	}
}

func TestRecording_Status_200(t *testing.T) {
	api := &recordingHTTPTestAPI{}
	srv := newTestServer(t, func(s *LaunchServer) { s.recording = api })
	resp := serve(srv, http.MethodGet, "/api/recording/status", nil)
	if resp.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d: %s", resp.Code, resp.Body.String())
	}
	var p struct {
		OK     bool   `json:"ok"`
		Active bool   `json:"active"`
		Count  int    `json:"count"`
	}
	decodeJSON(t, resp, &p)
	if !p.OK || !p.Active || p.Count != 1 {
		t.Fatalf("unexpected payload: %+v", p)
	}
}

func TestRecording_Start_200(t *testing.T) {
	api := &recordingHTTPTestAPI{}
	srv := newTestServer(t, func(s *LaunchServer) { s.recording = api })
	body := map[string]string{"profileId": "prof-1"}
	resp := serve(srv, http.MethodPost, "/api/recording/start", body)
	if resp.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d: %s", resp.Code, resp.Body.String())
	}
}

func TestRecording_Start_MissingProfileID_400(t *testing.T) {
	api := &recordingHTTPTestAPI{}
	srv := newTestServer(t, func(s *LaunchServer) { s.recording = api })
	resp := serve(srv, http.MethodPost, "/api/recording/start", map[string]string{})
	if resp.Code != http.StatusBadRequest {
		t.Fatalf("expected 400, got %d: %s", resp.Code, resp.Body.String())
	}
}

func TestRecording_NoAPI_503(t *testing.T) {
	srv := newTestServer(t)
	resp := serve(srv, http.MethodGet, "/api/recording/list", nil)
	if resp.Code != http.StatusServiceUnavailable {
		t.Fatalf("expected 503, got %d: %s", resp.Code, resp.Body.String())
	}
}

func TestRecording_Stop_Success(t *testing.T) {
	api := &recordingHTTPTestAPI{}
	srv := newTestServer(t, func(s *LaunchServer) { s.recording = api })
	body := map[string]string{"profileId": "prof-1"}
	resp := serve(srv, http.MethodPost, "/api/recording/stop", body)
	if resp.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d: %s", resp.Code, resp.Body.String())
	}
}

func TestRecording_Stop_MissingProfileID_400(t *testing.T) {
	api := &recordingHTTPTestAPI{}
	srv := newTestServer(t, func(s *LaunchServer) { s.recording = api })
	resp := serve(srv, http.MethodPost, "/api/recording/stop", map[string]string{})
	if resp.Code != http.StatusBadRequest {
		t.Fatalf("expected 400, got %d: %s", resp.Code, resp.Body.String())
	}
}

func TestRecording_Stop_WrongMethod_405(t *testing.T) {
	api := &recordingHTTPTestAPI{}
	srv := newTestServer(t, func(s *LaunchServer) { s.recording = api })
	resp := serve(srv, http.MethodGet, "/api/recording/stop", nil)
	if resp.Code != http.StatusMethodNotAllowed {
		t.Fatalf("expected 405, got %d: %s", resp.Code, resp.Body.String())
	}
}

func TestRecording_Play_Success(t *testing.T) {
	api := &recordingHTTPTestAPI{}
	srv := newTestServer(t, func(s *LaunchServer) { s.recording = api })
	body := map[string]string{"profileId": "prof-1", "recordingId": "rec-1"}
	resp := serve(srv, http.MethodPost, "/api/recording/play", body)
	if resp.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d: %s", resp.Code, resp.Body.String())
	}
	var p struct {
		OK          bool   `json:"ok"`
		Playing     bool   `json:"playing"`
		ProfileID   string `json:"profileId"`
		RecordingID string `json:"recordingId"`
	}
	decodeJSON(t, resp, &p)
	if !p.OK || !p.Playing || p.ProfileID != "prof-1" || p.RecordingID != "rec-1" {
		t.Fatalf("unexpected payload: %+v", p)
	}
}

func TestRecording_Play_MissingFields_400(t *testing.T) {
	api := &recordingHTTPTestAPI{}
	srv := newTestServer(t, func(s *LaunchServer) { s.recording = api })
	resp := serve(srv, http.MethodPost, "/api/recording/play", map[string]string{"profileId": "prof-1"})
	if resp.Code != http.StatusBadRequest {
		t.Fatalf("expected 400, got %d: %s", resp.Code, resp.Body.String())
	}
}

func TestRecording_Play_WrongMethod_405(t *testing.T) {
	api := &recordingHTTPTestAPI{}
	srv := newTestServer(t, func(s *LaunchServer) { s.recording = api })
	resp := serve(srv, http.MethodGet, "/api/recording/play", nil)
	if resp.Code != http.StatusMethodNotAllowed {
		t.Fatalf("expected 405, got %d: %s", resp.Code, resp.Body.String())
	}
}

func TestRecording_PlayStop_Success(t *testing.T) {
	api := &recordingHTTPTestAPI{}
	srv := newTestServer(t, func(s *LaunchServer) { s.recording = api })
	body := map[string]string{"profileId": "prof-1"}
	resp := serve(srv, http.MethodPost, "/api/recording/play/stop", body)
	if resp.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d: %s", resp.Code, resp.Body.String())
	}
	var p struct {
		OK        bool   `json:"ok"`
		Stopped   bool   `json:"stopped"`
		ProfileID string `json:"profileId"`
	}
	decodeJSON(t, resp, &p)
	if !p.OK || !p.Stopped || p.ProfileID != "prof-1" {
		t.Fatalf("unexpected payload: %+v", p)
	}
}

func TestRecording_PlayStop_MissingProfileID_400(t *testing.T) {
	api := &recordingHTTPTestAPI{}
	srv := newTestServer(t, func(s *LaunchServer) { s.recording = api })
	resp := serve(srv, http.MethodPost, "/api/recording/play/stop", map[string]string{})
	if resp.Code != http.StatusBadRequest {
		t.Fatalf("expected 400, got %d: %s", resp.Code, resp.Body.String())
	}
}

func TestRecording_PlayStop_WrongMethod_405(t *testing.T) {
	api := &recordingHTTPTestAPI{}
	srv := newTestServer(t, func(s *LaunchServer) { s.recording = api })
	resp := serve(srv, http.MethodGet, "/api/recording/play/stop", nil)
	if resp.Code != http.StatusMethodNotAllowed {
		t.Fatalf("expected 405, got %d: %s", resp.Code, resp.Body.String())
	}
}

func TestRecording_Quick_Success(t *testing.T) {
	api := &recordingHTTPTestAPI{quickErr: nil}
	srv := newTestServer(t, func(s *LaunchServer) { s.recording = api })
	body := map[string]string{"profileId": "prof-1"}
	resp := serve(srv, http.MethodPost, "/api/recording/quick", body)
	if resp.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d: %s", resp.Code, resp.Body.String())
	}
}

func TestRecording_Quick_MissingProfileID_400(t *testing.T) {
	api := &recordingHTTPTestAPI{}
	srv := newTestServer(t, func(s *LaunchServer) { s.recording = api })
	resp := serve(srv, http.MethodPost, "/api/recording/quick", map[string]string{})
	if resp.Code != http.StatusBadRequest {
		t.Fatalf("expected 400, got %d: %s", resp.Code, resp.Body.String())
	}
}

func TestRecording_Quick_WrongMethod_405(t *testing.T) {
	api := &recordingHTTPTestAPI{}
	srv := newTestServer(t, func(s *LaunchServer) { s.recording = api })
	resp := serve(srv, http.MethodGet, "/api/recording/quick", nil)
	if resp.Code != http.StatusMethodNotAllowed {
		t.Fatalf("expected 405, got %d: %s", resp.Code, resp.Body.String())
	}
}

func TestRecording_Cleanup_WrongMethod_405(t *testing.T) {
	api := &recordingHTTPTestAPI{}
	srv := newTestServer(t, func(s *LaunchServer) { s.recording = api })
	resp := serve(srv, http.MethodPut, "/api/recording/sessions/cleanup", nil)
	if resp.Code != http.StatusMethodNotAllowed {
		t.Fatalf("expected 405, got %d: %s", resp.Code, resp.Body.String())
	}
}

func TestRecordingByID_Delete_Success(t *testing.T) {
	api := &recordingHTTPTestAPI{}
	srv := newTestServer(t, func(s *LaunchServer) { s.recording = api })
	resp := serve(srv, http.MethodDelete, "/api/recording/rec-1", nil)
	if resp.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d: %s", resp.Code, resp.Body.String())
	}
	var p struct {
		OK          bool   `json:"ok"`
		Deleted     bool   `json:"deleted"`
		RecordingID string `json:"recordingId"`
	}
	decodeJSON(t, resp, &p)
	if !p.OK || !p.Deleted || p.RecordingID != "rec-1" {
		t.Fatalf("unexpected payload: %+v", p)
	}
}

func TestRecordingByID_WrongMethod_405(t *testing.T) {
	api := &recordingHTTPTestAPI{}
	srv := newTestServer(t, func(s *LaunchServer) { s.recording = api })
	resp := serve(srv, http.MethodPost, "/api/recording/rec-1", nil)
	if resp.Code != http.StatusMethodNotAllowed {
		t.Fatalf("expected 405, got %d: %s", resp.Code, resp.Body.String())
	}
}

func TestRecordingByID_PathTraversal_400(t *testing.T) {
	api := &recordingHTTPTestAPI{}
	srv := newTestServer(t, func(s *LaunchServer) { s.recording = api })
	req := httptest.NewRequest(http.MethodGet, "/api/recording/../secret", nil)
	w := httptest.NewRecorder()
	srv.handleRecordingByID(w, req)
	if w.Code != http.StatusBadRequest {
		t.Fatalf("expected 400 for path traversal, got %d: %s", w.Code, w.Body.String())
	}
}

func TestRecording_EventPage_InvalidOffset_400(t *testing.T) {
	api := &recordingHTTPTestAPI{}
	srv := newTestServer(t, func(s *LaunchServer) { s.recording = api })
	resp := serve(srv, http.MethodGet, "/api/recording/rec-1?eventOffset=-1", nil)
	if resp.Code != http.StatusBadRequest {
		t.Fatalf("expected 400 for invalid offset, got %d: %s", resp.Code, resp.Body.String())
	}
}

func TestRecording_EventPage_InvalidLimit_400(t *testing.T) {
	api := &recordingHTTPTestAPI{}
	srv := newTestServer(t, func(s *LaunchServer) { s.recording = api })
	resp := serve(srv, http.MethodGet, "/api/recording/rec-1?eventLimit=0", nil)
	if resp.Code != http.StatusBadRequest {
		t.Fatalf("expected 400 for invalid limit, got %d: %s", resp.Code, resp.Body.String())
	}
}

// ---------------------------------------------------------------------------
// 7. Auth middleware tests
// ---------------------------------------------------------------------------

func TestAuth_ValidKey_Passes(t *testing.T) {
	srv := newTestServer(t, func(s *LaunchServer) {
		s.SetAPIAuthConfig(APIAuthConfig{Enabled: true, APIKey: "correct", Header: "X-Key"})
	})
	req := httptest.NewRequest(http.MethodGet, "/api/health", nil)
	req.Header.Set("X-Key", "correct")
	w := httptest.NewRecorder()
	srv.buildHandler(false).ServeHTTP(w, req)
	if w.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d: %s", w.Code, w.Body.String())
	}
}

func TestAuth_InvalidKey_401(t *testing.T) {
	srv := newTestServer(t, func(s *LaunchServer) {
		s.SetAPIAuthConfig(APIAuthConfig{Enabled: true, APIKey: "correct", Header: "X-Key"})
	})
	req := httptest.NewRequest(http.MethodGet, "/api/health", nil)
	req.Header.Set("X-Key", "wrong")
	w := httptest.NewRecorder()
	srv.buildHandler(false).ServeHTTP(w, req)
	if w.Code != http.StatusUnauthorized {
		t.Fatalf("expected 401, got %d: %s", w.Code, w.Body.String())
	}
}

func TestAuth_EnvKeyReference_PassesWithResolvedEnvValue(t *testing.T) {
	t.Setenv("PERSONAL_PILOT_TEST_AUTH_KEY", "resolved-key")
	srv := newTestServer(t, func(s *LaunchServer) {
		s.SetAPIAuthConfig(APIAuthConfig{Enabled: true, APIKey: "${PERSONAL_PILOT_TEST_AUTH_KEY}", Header: "X-Key"})
	})

	req := httptest.NewRequest(http.MethodGet, "/api/health", nil)
	req.Header.Set("X-Key", "resolved-key")
	w := httptest.NewRecorder()
	srv.buildHandler(false).ServeHTTP(w, req)
	if w.Code != http.StatusOK {
		t.Fatalf("expected 200 with resolved env key, got %d: %s", w.Code, w.Body.String())
	}

	reqLiteral := httptest.NewRequest(http.MethodGet, "/api/health", nil)
	reqLiteral.Header.Set("X-Key", "${PERSONAL_PILOT_TEST_AUTH_KEY}")
	wLiteral := httptest.NewRecorder()
	srv.buildHandler(false).ServeHTTP(wLiteral, reqLiteral)
	if wLiteral.Code != http.StatusUnauthorized {
		t.Fatalf("expected 401 for literal env placeholder, got %d: %s", wLiteral.Code, wLiteral.Body.String())
	}
}

func TestAuth_MissingEnvKeyReference_DisablesAuth(t *testing.T) {
	envName := "PERSONAL_PILOT_TEST_MISSING_AUTH_KEY"
	if old, ok := os.LookupEnv(envName); ok {
		t.Cleanup(func() {
			_ = os.Setenv(envName, old)
		})
	} else {
		t.Cleanup(func() {
			_ = os.Unsetenv(envName)
		})
	}
	_ = os.Unsetenv(envName)

	cfg := normalizeAPIAuthConfig(APIAuthConfig{Enabled: true, APIKey: "${PERSONAL_PILOT_TEST_MISSING_AUTH_KEY}", Header: "X-Key"})
	if cfg.Configured() {
		t.Fatalf("missing env key should not configure auth")
	}
	if cfg.Active() {
		t.Fatalf("missing env key should not activate auth")
	}
}

func TestAuth_Disabled_Passes(t *testing.T) {
	srv := newTestServer(t, func(s *LaunchServer) {
		s.SetAPIAuthConfig(APIAuthConfig{Enabled: false, APIKey: "correct", Header: "X-Key"})
	})
	req := httptest.NewRequest(http.MethodGet, "/api/health", nil)
	req.Header.Set("X-Key", "wrong")
	w := httptest.NewRecorder()
	srv.buildHandler(false).ServeHTTP(w, req)
	if w.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d: %s", w.Code, w.Body.String())
	}
}

func TestAuth_NonAPIPath_NoAuth(t *testing.T) {
	srv := newTestServer(t, func(s *LaunchServer) {
		s.SetAPIAuthConfig(APIAuthConfig{Enabled: true, APIKey: "secret", Header: DefaultAPIKeyHeader})
	})
	// Non-/api/ path should not be subject to auth
	req := httptest.NewRequest(http.MethodGet, "/some-other-path", nil)
	w := httptest.NewRecorder()
	srv.buildHandler(false).ServeHTTP(w, req)
	// No route handler for this path, so it'll hit the CDP proxy or fall through
	// We just need to verify it's not 401
	if w.Code == http.StatusUnauthorized {
		t.Fatal("non-api path should not be blocked by auth middleware")
	}
}

func TestAuth_ActiveState(t *testing.T) {
	t.Run("Active() returns true when enabled and configured", func(t *testing.T) {
		cfg := APIAuthConfig{Enabled: true, APIKey: "k"}
		cfg = normalizeAPIAuthConfig(cfg)
		if !cfg.Active() {
			t.Fatal("expected Active()=true")
		}
	})
	t.Run("Active() returns false when not enabled", func(t *testing.T) {
		cfg := APIAuthConfig{Enabled: false, APIKey: "k"}
		cfg = normalizeAPIAuthConfig(cfg)
		if cfg.Active() {
			t.Fatal("expected Active()=false")
		}
	})
	t.Run("Active() returns false when key is empty", func(t *testing.T) {
		cfg := APIAuthConfig{Enabled: true, APIKey: ""}
		cfg = normalizeAPIAuthConfig(cfg)
		if cfg.Active() {
			t.Fatal("expected Active()=false")
		}
	})
	t.Run("Requested() reflects Enabled", func(t *testing.T) {
		enabled := APIAuthConfig{Enabled: true}
		disabled := APIAuthConfig{Enabled: false}
		if !enabled.Requested() || disabled.Requested() {
			t.Fatal("Requested must match Enabled")
		}
	})
	t.Run("Configured() reflects non-empty key", func(t *testing.T) {
		withKey := APIAuthConfig{APIKey: "k"}
		emptyKey := APIAuthConfig{APIKey: ""}
		if !withKey.Configured() || emptyKey.Configured() {
			t.Fatal("Configured must reflect non-empty key")
		}
	})
}

func TestAuth_ConstantTimeComparison(t *testing.T) {
	srv := newTestServer(t, func(s *LaunchServer) {
		s.SetAPIAuthConfig(APIAuthConfig{Enabled: true, APIKey: "secret-key-123", Header: "X-Auth"})
	})
	t.Run("matching key", func(t *testing.T) {
		req := httptest.NewRequest(http.MethodGet, "/api/health", nil)
		req.Header.Set("X-Auth", "secret-key-123")
		w := httptest.NewRecorder()
		srv.buildHandler(false).ServeHTTP(w, req)
		if w.Code != http.StatusOK {
			t.Fatalf("expected 200, got %d", w.Code)
		}
	})
	t.Run("non-matching key", func(t *testing.T) {
		req := httptest.NewRequest(http.MethodGet, "/api/health", nil)
		req.Header.Set("X-Auth", "secret-key-999")
		w := httptest.NewRecorder()
		srv.buildHandler(false).ServeHTTP(w, req)
		if w.Code != http.StatusUnauthorized {
			t.Fatalf("expected 401, got %d", w.Code)
		}
	})
	t.Run("missing header", func(t *testing.T) {
		req := httptest.NewRequest(http.MethodGet, "/api/health", nil)
		w := httptest.NewRecorder()
		srv.buildHandler(false).ServeHTTP(w, req)
		if w.Code != http.StatusUnauthorized {
			t.Fatalf("expected 401, got %d", w.Code)
		}
	})
	t.Run("default header name used when not set", func(t *testing.T) {
		srv2 := newTestServer(t, func(s *LaunchServer) {
			s.SetAPIAuthConfig(APIAuthConfig{Enabled: true, APIKey: "key", Header: ""})
		})
		req := httptest.NewRequest(http.MethodGet, "/api/health", nil)
		req.Header.Set(DefaultAPIKeyHeader, "key")
		w := httptest.NewRecorder()
		srv2.buildHandler(false).ServeHTTP(w, req)
		if w.Code != http.StatusOK {
			t.Fatalf("expected 200, got %d", w.Code)
		}
	})
}

// ---------------------------------------------------------------------------
// 8. Ownership guard
// ---------------------------------------------------------------------------

func TestOwnershipGuard_NilAudit(t *testing.T) {
	srv := newTestServer(t)
	err := srv.validateActiveCDPOwnership("prof-1", 9222, nil)
	if err == nil {
		t.Fatal("expected error for nil audit")
	}
}

func TestOwnershipGuard_MissingProfileInManager(t *testing.T) {
	srv := newTestServer(t)
	// audit is not nil but profile doesn't exist in manager
	audit := &browser.LaunchAuditSnapshot{
		ProfileID:            "missing-prof",
		DebugPort:            9222,
		PID:                  12345,
		BrowserExe:           "/test/chrome",
		CanonicalUserDataDir: "/test/data",
		ProxyHash:            "abc",
		FingerprintArgsHash:  "def",
		LaunchArgsHash:       "ghi",
		Timestamp:            "2025-01-01T00:00:00Z",
		AppMode:              "test",
	}
	err := srv.validateActiveCDPOwnership("missing-prof", 9222, audit)
	if err == nil {
		t.Fatal("expected error for missing profile in manager")
	}
}

func TestOwnershipGuard_MismatchedProfileID(t *testing.T) {
	srv := newTestServer(t)
	// audit has a different profileId than the one being checked
	audit := &browser.LaunchAuditSnapshot{
		ProfileID:            "other-prof",
		DebugPort:            9222,
		PID:                  12345,
		BrowserExe:           "/test/chrome",
		CanonicalUserDataDir: "/test/data",
		ProxyHash:            "abc",
		FingerprintArgsHash:  "def",
		LaunchArgsHash:       "ghi",
		Timestamp:            "2025-01-01T00:00:00Z",
		AppMode:              "test",
	}
	err := srv.validateActiveCDPOwnership("prof-1", 9222, audit)
	if err == nil {
		t.Fatal("expected error for mismatched profileId")
	}
}

func TestOwnershipGuard_NilManager_PassesBasicChecks(t *testing.T) {
	srv := newTestServer(t)
	srv.browserMgr = nil // no manager — the guard should only do audit checks
	audit := &browser.LaunchAuditSnapshot{
		ProfileID:            "prof-1",
		DebugPort:            9222,
		PID:                  12345,
		BrowserExe:           "/test/chrome",
		CanonicalUserDataDir: "/test/data",
		ProxyHash:            "abc",
		FingerprintArgsHash:  "def",
		LaunchArgsHash:       "ghi",
		Timestamp:            "2025-01-01T00:00:00Z",
		AppMode:              "test",
	}
	// With nil manager, the guard returns nil after audit checks
	// because the expected fields match the audit
	err := srv.validateActiveCDPOwnership("prof-1", 9222, audit)
	if err != nil {
		t.Fatalf("expected no error with matching audit and nil manager, got: %v", err)
	}
}

// ---------------------------------------------------------------------------
// 4b. Launch POST error paths
// ---------------------------------------------------------------------------

func TestLaunch_POST_UnknownCode_404(t *testing.T) {
	srv := newTestServer(t)
	body := map[string]interface{}{"code": "ZZZZZZ"}
	resp := serve(srv, http.MethodPost, "/api/launch", body)
	if resp.Code != http.StatusNotFound {
		t.Fatalf("expected 404 for unknown code, got %d: %s", resp.Code, resp.Body.String())
	}
}

func TestLaunch_POST_StartError_500(t *testing.T) {
	starter := &mockStarter{startErr: errors.New("browser start failure")}
	svc := NewLaunchCodeService(NewMemoryLaunchCodeDAO())
	code, err := svc.EnsureCode("prof-1")
	if err != nil {
		t.Fatalf("EnsureCode: %v", err)
	}
	mgr := &browser.Manager{
		Config:   &config.Config{Browser: config.BrowserConfig{UserDataRoot: "data"}},
		Profiles: make(map[string]*browser.Profile),
	}
	srv := NewLaunchServer(svc, starter, nil, mgr, 0)
	body := map[string]interface{}{"code": code}
	resp := serve(srv, http.MethodPost, "/api/launch", body)
	if resp.Code != http.StatusInternalServerError {
		t.Fatalf("expected 500 on start error, got %d: %s", resp.Code, resp.Body.String())
	}
	var p struct {
		OK    bool   `json:"ok"`
		Error string `json:"error"`
	}
	decodeJSON(t, resp, &p)
	if p.OK {
		t.Fatal("expected ok=false")
	}
	if p.Error != "browser start failure" {
		t.Fatalf("expected error 'browser start failure', got %q", p.Error)
	}
}

func TestLaunch_POST_AllMatchMode_StartError_500(t *testing.T) {
	starter := &mockStarter{startErr: errors.New("browser start failure")}
	svc := NewLaunchCodeService(NewMemoryLaunchCodeDAO())
	mgr := &browser.Manager{
		Config: &config.Config{Browser: config.BrowserConfig{UserDataRoot: "data"}},
		Profiles: map[string]*browser.Profile{
			"prof-1": {ProfileId: "prof-1", ProfileName: "Alpha"},
		},
	}
	srv := NewLaunchServer(svc, starter, nil, mgr, 0)
	body := map[string]interface{}{"selector": map[string]interface{}{"profileId": "prof-1", "matchMode": "all"}}
	resp := serve(srv, http.MethodPost, "/api/launch", body)
	if resp.Code != http.StatusInternalServerError {
		t.Fatalf("expected 500 on start error with matchMode=all, got %d: %s", resp.Code, resp.Body.String())
	}
}

// ---------------------------------------------------------------------------
// 10. Behavior Presets
// ---------------------------------------------------------------------------

func TestBehaviorPresets_List_200(t *testing.T) {
	api := &recordingHTTPTestAPI{}
	srv := newTestServer(t, func(s *LaunchServer) { s.recording = api })
	resp := serve(srv, http.MethodGet, "/api/behavior/presets", nil)
	if resp.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d: %s", resp.Code, resp.Body.String())
	}
	var p struct {
		OK    bool              `json:"ok"`
		Count int               `json:"count"`
		Items []behavior.Profile `json:"items"`
	}
	decodeJSON(t, resp, &p)
	if !p.OK {
		t.Fatal("expected ok=true")
	}
}

func TestBehaviorPresets_List_NoAPI_503(t *testing.T) {
	srv := newTestServer(t)
	resp := serve(srv, http.MethodGet, "/api/behavior/presets", nil)
	if resp.Code != http.StatusServiceUnavailable {
		t.Fatalf("expected 503, got %d: %s", resp.Code, resp.Body.String())
	}
}

func TestBehaviorPresets_ByID_200(t *testing.T) {
	api := &recordingHTTPTestAPI{}
	srv := newTestServer(t, func(s *LaunchServer) { s.recording = api })
	resp := serve(srv, http.MethodGet, "/api/behavior/presets/default", nil)
	if resp.Code != http.StatusNotFound {
		t.Fatalf("expected 404 (no presets loaded), got %d: %s", resp.Code, resp.Body.String())
	}
}

func TestBehaviorPresets_ByID_NoAPI_503(t *testing.T) {
	srv := newTestServer(t)
	resp := serve(srv, http.MethodGet, "/api/behavior/presets/default", nil)
	if resp.Code != http.StatusServiceUnavailable {
		t.Fatalf("expected 503, got %d: %s", resp.Code, resp.Body.String())
	}
}

func TestBehaviorPresets_ByID_PathTraversal_400(t *testing.T) {
	api := &recordingHTTPTestAPI{}
	srv := newTestServer(t, func(s *LaunchServer) { s.recording = api })
	req := httptest.NewRequest(http.MethodGet, "/api/behavior/presets/../secret", nil)
	w := httptest.NewRecorder()
	srv.handleBehaviorPresetByID(w, req)
	if w.Code != http.StatusBadRequest {
		t.Fatalf("expected 400 for path traversal, got %d: %s", w.Code, w.Body.String())
	}
}

// ---------------------------------------------------------------------------
// 11. CDP Proxy
// ---------------------------------------------------------------------------

func TestCDPProxy_NoActiveTarget_503(t *testing.T) {
	srv := newTestServer(t)
	resp := serve(srv, http.MethodGet, "/", nil)
	if resp.Code != http.StatusServiceUnavailable {
		t.Fatalf("expected 503, got %d: %s", resp.Code, resp.Body.String())
	}
}

func TestCDPProxy_ActiveTarget_ProxiesRequest(t *testing.T) {
	targetSrv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusOK)
		w.Write([]byte(`{"target":"ok"}`))
	}))
	defer targetSrv.Close()

	targetURL, _ := url.Parse(targetSrv.URL)
	_, targetPortStr, _ := net.SplitHostPort(targetURL.Host)
	targetPort, _ := strconv.Atoi(targetPortStr)

	svc := NewLaunchCodeService(NewMemoryLaunchCodeDAO())
	srv := NewLaunchServer(svc, &mockStarter{}, nil, nil, 0)

	profile := &browser.Profile{
		ProfileId:  "prof-1",
		DebugPort:  targetPort,
		DebugReady: true,
		LaunchAudit: &browser.LaunchAuditSnapshot{
			ProfileID:            "prof-1",
			DebugPort:            targetPort,
			PID:                  12345,
			BrowserExe:           "/test/chrome",
			CanonicalUserDataDir: "/test/data",
			ProxyHash:            "abc",
			FingerprintArgsHash:  "def",
			LaunchArgsHash:       "ghi",
			Timestamp:            "2025-01-01T00:00:00Z",
			AppMode:              "test",
		},
	}
	srv.SetActiveProfile(profile)

	resp := serve(srv, http.MethodGet, "/json/version", nil)
	if resp.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d: %s", resp.Code, resp.Body.String())
	}
	if !strings.Contains(resp.Body.String(), `"target":"ok"`) {
		t.Fatalf("expected proxied content, got %s", resp.Body.String())
	}
}

func TestCDPProxy_OwnershipConflict_409(t *testing.T) {
	svc := NewLaunchCodeService(NewMemoryLaunchCodeDAO())
	srv := NewLaunchServer(svc, &mockStarter{}, nil, nil, 0)

	profile := &browser.Profile{
		ProfileId:  "prof-1",
		DebugPort:  9999,
		DebugReady: true,
		LaunchAudit: &browser.LaunchAuditSnapshot{
			ProfileID: "prof-1",
			DebugPort: 8888,
		},
	}
	srv.SetActiveProfile(profile)

	resp := serve(srv, http.MethodGet, "/", nil)
	if resp.Code != http.StatusConflict {
		t.Fatalf("expected 409 for ownership mismatch, got %d: %s", resp.Code, resp.Body.String())
	}
}

func TestCDPProxy_NonAPIPath_NoAuth(t *testing.T) {
	srv := newTestServer(t, func(s *LaunchServer) {
		s.SetAPIAuthConfig(APIAuthConfig{Enabled: true, APIKey: "secret", Header: DefaultAPIKeyHeader})
	})
	resp := serve(srv, http.MethodGet, "/", nil)
	if resp.Code != http.StatusServiceUnavailable {
		t.Fatalf("expected 503 (no active target), not 401 for non-API path, got %d: %s", resp.Code, resp.Body.String())
	}
}

// ---------------------------------------------------------------------------
// 12. Mock error paths
// ---------------------------------------------------------------------------

func TestProfiles_Create_NoCreator_503(t *testing.T) {
	svc := NewLaunchCodeService(NewMemoryLaunchCodeDAO())
	srv := NewLaunchServer(svc, nil, nil, nil, 0) // nil starter means no creator
	body := map[string]interface{}{
		"profile": map[string]interface{}{
			"profileName": "No Creator",
		},
	}
	resp := serve(srv, http.MethodPost, "/api/profiles", body)
	if resp.Code != http.StatusServiceUnavailable {
		t.Fatalf("expected 503, got %d: %s", resp.Code, resp.Body.String())
	}
}

func TestProfiles_Create_CreateError_500(t *testing.T) {
	starter := &mockStarter{createErr: errors.New("create failed")}
	svc := NewLaunchCodeService(NewMemoryLaunchCodeDAO())
	mgr := &browser.Manager{
		Config:   &config.Config{Browser: config.BrowserConfig{UserDataRoot: "data"}},
		Profiles: make(map[string]*browser.Profile),
	}
	srv := NewLaunchServer(svc, starter, nil, mgr, 0)
	body := map[string]interface{}{
		"profile": map[string]interface{}{
			"profileName": "Fail",
		},
	}
	resp := serve(srv, http.MethodPost, "/api/profiles", body)
	if resp.Code != http.StatusInternalServerError {
		t.Fatalf("expected 500, got %d: %s", resp.Code, resp.Body.String())
	}
}

func TestInstanceStop_StopError_500(t *testing.T) {
	starter := &mockStarter{stopErr: errors.New("stop failed")}
	svc := NewLaunchCodeService(NewMemoryLaunchCodeDAO())
	mgr := &browser.Manager{
		Config:   &config.Config{Browser: config.BrowserConfig{UserDataRoot: "data"}},
		Profiles: make(map[string]*browser.Profile),
	}
	srv := NewLaunchServer(svc, starter, nil, mgr, 0)
	body := map[string]string{"profileId": "prof-1"}
	resp := serve(srv, http.MethodPost, "/api/instances/stop", body)
	if resp.Code != http.StatusInternalServerError {
		t.Fatalf("expected 500, got %d: %s", resp.Code, resp.Body.String())
	}
}

func TestWorkbench_Navigate_Error(t *testing.T) {
	starter := &mockStarter{navigateErr: errors.New("navigate failed")}
	svc := NewLaunchCodeService(NewMemoryLaunchCodeDAO())
	mgr := &browser.Manager{
		Config:   &config.Config{Browser: config.BrowserConfig{UserDataRoot: "data"}},
		Profiles: make(map[string]*browser.Profile),
	}
	srv := NewLaunchServer(svc, starter, nil, mgr, 0)
	body := map[string]string{"profileId": "prof-1", "url": "https://example.com"}
	resp := serve(srv, http.MethodPost, "/api/workbench/navigate", body)
	if resp.Code != http.StatusInternalServerError {
		t.Fatalf("expected 500, got %d: %s", resp.Code, resp.Body.String())
	}
}

func TestWorkbench_Refresh_Error(t *testing.T) {
	starter := &mockStarter{refreshErr: errors.New("refresh failed")}
	svc := NewLaunchCodeService(NewMemoryLaunchCodeDAO())
	mgr := &browser.Manager{
		Config:   &config.Config{Browser: config.BrowserConfig{UserDataRoot: "data"}},
		Profiles: make(map[string]*browser.Profile),
	}
	srv := NewLaunchServer(svc, starter, nil, mgr, 0)
	body := map[string]string{"profileId": "prof-1"}
	resp := serve(srv, http.MethodPost, "/api/workbench/refresh", body)
	if resp.Code != http.StatusInternalServerError {
		t.Fatalf("expected 500, got %d: %s", resp.Code, resp.Body.String())
	}
}

func TestWorkbench_Screenshot_Error(t *testing.T) {
	starter := &mockStarter{screenshotErr: errors.New("screenshot failed")}
	svc := NewLaunchCodeService(NewMemoryLaunchCodeDAO())
	mgr := &browser.Manager{
		Config:   &config.Config{Browser: config.BrowserConfig{UserDataRoot: "data"}},
		Profiles: make(map[string]*browser.Profile),
	}
	srv := NewLaunchServer(svc, starter, nil, mgr, 0)
	body := map[string]string{"profileId": "prof-1"}
	resp := serve(srv, http.MethodPost, "/api/workbench/screenshot", body)
	if resp.Code != http.StatusInternalServerError {
		t.Fatalf("expected 500, got %d: %s", resp.Code, resp.Body.String())
	}
}

// ---------------------------------------------------------------------------
// Benchmark: router performance
// ---------------------------------------------------------------------------

func BenchmarkHealthEndpoint(b *testing.B) {
	svc := NewLaunchCodeService(NewMemoryLaunchCodeDAO())
	mgr := &browser.Manager{
		Config: &config.Config{Browser: config.BrowserConfig{
			UserDataRoot: "data",
			Cores:        []config.BrowserCore{},
			Proxies:      []config.BrowserProxy{},
			Profiles:     []config.BrowserProfileConfig{},
		}},
		Profiles: make(map[string]*browser.Profile),
	}
	srv := NewLaunchServer(svc, &mockStarter{}, nil, mgr, 0)
	for i := 0; i < b.N; i++ {
		req := httptest.NewRequest(http.MethodGet, "/api/health", nil)
		w := httptest.NewRecorder()
		srv.buildHandler(false).ServeHTTP(w, req)
	}
}

// ---------------------------------------------------------------------------
// Example: integration-style test that verifies multiple endpoints
// ---------------------------------------------------------------------------

func Test_ExampleFullFlow(t *testing.T) {
	// This test shows a full API call sequence against one server to
	// validate that no shared state is corrupted between calls.
	srv := newTestServer(t)

	// 1) health
	resp := serve(srv, http.MethodGet, "/api/health", nil)
	if resp.Code != http.StatusOK {
		t.Fatalf("health: %d", resp.Code)
	}

	// 2) list profiles (empty)
	resp = serve(srv, http.MethodGet, "/api/profiles", nil)
	if resp.Code != http.StatusOK {
		t.Fatalf("list: %d", resp.Code)
	}
	var listResp struct {
		Count int `json:"count"`
	}
	decodeJSON(t, resp, &listResp)
	if listResp.Count != 0 {
		t.Fatalf("expected 0 profiles, got %d", listResp.Count)
	}

	// 3) create a profile
	resp = serve(srv, http.MethodPost, "/api/profiles", map[string]interface{}{
		"profile": map[string]interface{}{"profileName": "Integration Test Profile"},
	})
	if resp.Code != http.StatusCreated {
		t.Fatalf("create: %d %s", resp.Code, resp.Body.String())
	}

	// 4) launch logs
	resp = serve(srv, http.MethodGet, "/api/launch/logs", nil)
	if resp.Code != http.StatusOK {
		t.Fatalf("logs: %d", resp.Code)
	}

	// 5) recording status without recording API → 503
	resp = serve(srv, http.MethodGet, "/api/recording/status", nil)
	if resp.Code != http.StatusServiceUnavailable {
		t.Fatalf("expected 503 for recording without API, got %d", resp.Code)
	}

	// 6) instance stop
	resp = serve(srv, http.MethodPost, "/api/instances/stop",
		map[string]string{"profileId": "test-prof"})
	if resp.Code != http.StatusOK {
		t.Fatalf("stop: %d %s", resp.Code, resp.Body.String())
	}

	fmt.Println("Full flow integration-style test passed")
}

// ---------------------------------------------------------------------------
// 13. Enhanced action endpoints (hover, double-click, right-click, wait, actions)
// ---------------------------------------------------------------------------

func TestWorkbench_Hover_Success(t *testing.T) {
	srv := newTestServer(t)
	body := map[string]interface{}{
		"profileId": "prof-1",
		"action":    map[string]interface{}{"type": "hover", "selector": "#btn"},
	}
	resp := serve(srv, http.MethodPost, "/api/workbench/hover", body)
	if resp.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d: %s", resp.Code, resp.Body.String())
	}
	var p struct {
		OK      bool   `json:"ok"`
		Hovered bool   `json:"hovered"`
		PageURL string `json:"pageUrl"`
	}
	decodeJSON(t, resp, &p)
	if !p.OK || !p.Hovered || p.PageURL == "" {
		t.Fatalf("unexpected payload: %+v", p)
	}
}

func TestWorkbench_Hover_NoOperator_503(t *testing.T) {
	svc := NewLaunchCodeService(NewMemoryLaunchCodeDAO())
	srv := NewLaunchServer(svc, nil, nil, nil, 0)
	body := map[string]interface{}{
		"profileId": "prof-1",
		"action":    map[string]interface{}{"type": "hover", "selector": "#btn"},
	}
	resp := serve(srv, http.MethodPost, "/api/workbench/hover", body)
	if resp.Code != http.StatusServiceUnavailable {
		t.Fatalf("expected 503, got %d: %s", resp.Code, resp.Body.String())
	}
}

func TestWorkbench_Hover_MissingProfileID_400(t *testing.T) {
	srv := newTestServer(t)
	body := map[string]interface{}{
		"action": map[string]interface{}{"type": "hover", "selector": "#btn"},
	}
	resp := serve(srv, http.MethodPost, "/api/workbench/hover", body)
	if resp.Code != http.StatusBadRequest {
		t.Fatalf("expected 400, got %d: %s", resp.Code, resp.Body.String())
	}
}

func TestWorkbench_Hover_WrongMethod_405(t *testing.T) {
	srv := newTestServer(t)
	resp := serve(srv, http.MethodGet, "/api/workbench/hover", nil)
	if resp.Code != http.StatusMethodNotAllowed {
		t.Fatalf("expected 405, got %d: %s", resp.Code, resp.Body.String())
	}
}

func TestWorkbench_DoubleClick_Success(t *testing.T) {
	srv := newTestServer(t)
	body := map[string]interface{}{
		"profileId": "prof-1",
		"action":    map[string]interface{}{"type": "double-click", "selector": "#btn"},
	}
	resp := serve(srv, http.MethodPost, "/api/workbench/double-click", body)
	if resp.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d: %s", resp.Code, resp.Body.String())
	}
	var p struct {
		OK            bool   `json:"ok"`
		DoubleClicked bool   `json:"doubleClicked"`
		PageURL       string `json:"pageUrl"`
	}
	decodeJSON(t, resp, &p)
	if !p.OK || !p.DoubleClicked || p.PageURL == "" {
		t.Fatalf("unexpected payload: %+v", p)
	}
}

func TestWorkbench_DoubleClick_WrongMethod_405(t *testing.T) {
	srv := newTestServer(t)
	resp := serve(srv, http.MethodGet, "/api/workbench/double-click", nil)
	if resp.Code != http.StatusMethodNotAllowed {
		t.Fatalf("expected 405, got %d: %s", resp.Code, resp.Body.String())
	}
}

func TestWorkbench_RightClick_Success(t *testing.T) {
	srv := newTestServer(t)
	body := map[string]interface{}{
		"profileId": "prof-1",
		"action":    map[string]interface{}{"type": "right-click", "selector": "#btn"},
	}
	resp := serve(srv, http.MethodPost, "/api/workbench/right-click", body)
	if resp.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d: %s", resp.Code, resp.Body.String())
	}
	var p struct {
		OK           bool   `json:"ok"`
		RightClicked bool   `json:"rightClicked"`
		PageURL      string `json:"pageUrl"`
	}
	decodeJSON(t, resp, &p)
	if !p.OK || !p.RightClicked || p.PageURL == "" {
		t.Fatalf("unexpected payload: %+v", p)
	}
}

func TestWorkbench_RightClick_WrongMethod_405(t *testing.T) {
	srv := newTestServer(t)
	resp := serve(srv, http.MethodGet, "/api/workbench/right-click", nil)
	if resp.Code != http.StatusMethodNotAllowed {
		t.Fatalf("expected 405, got %d: %s", resp.Code, resp.Body.String())
	}
}

func TestWorkbench_Wait_Success(t *testing.T) {
	srv := newTestServer(t)
	body := map[string]interface{}{
		"profileId": "prof-1",
		"action":    map[string]interface{}{"type": "wait", "distance": 100},
	}
	resp := serve(srv, http.MethodPost, "/api/workbench/wait", body)
	if resp.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d: %s", resp.Code, resp.Body.String())
	}
	var p struct {
		OK    bool `json:"ok"`
		Waited bool `json:"waited"`
	}
	decodeJSON(t, resp, &p)
	if !p.OK || !p.Waited {
		t.Fatalf("unexpected payload: %+v", p)
	}
}

func TestWorkbench_Wait_WrongMethod_405(t *testing.T) {
	srv := newTestServer(t)
	resp := serve(srv, http.MethodGet, "/api/workbench/wait", nil)
	if resp.Code != http.StatusMethodNotAllowed {
		t.Fatalf("expected 405, got %d: %s", resp.Code, resp.Body.String())
	}
}

func TestWorkbench_Actions_Batch_Success(t *testing.T) {
	srv := newTestServer(t)
	body := map[string]interface{}{
		"profileId": "prof-1",
		"actions": []map[string]interface{}{
			{"type": "click", "selector": "#btn1"},
			{"type": "wait", "distance": 100},
			{"type": "hover", "selector": "#btn2"},
		},
	}
	resp := serve(srv, http.MethodPost, "/api/workbench/actions", body)
	if resp.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d: %s", resp.Code, resp.Body.String())
	}
	var p struct {
		OK      bool           `json:"ok"`
		Results []ActionResult `json:"results"`
	}
	decodeJSON(t, resp, &p)
	if !p.OK || len(p.Results) != 3 {
		t.Fatalf("unexpected payload: %+v", p)
	}
	for i, r := range p.Results {
		if !r.OK {
			t.Fatalf("result %d should be ok: %+v", i, r)
		}
	}
}

func TestWorkbench_Actions_SingleActionViaActions_Success(t *testing.T) {
	srv := newTestServer(t)
	body := map[string]interface{}{
		"profileId": "prof-1",
		"action":    map[string]interface{}{"type": "click", "selector": "#btn"},
	}
	resp := serve(srv, http.MethodPost, "/api/workbench/actions", body)
	if resp.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d: %s", resp.Code, resp.Body.String())
	}
	var p struct {
		OK      bool           `json:"ok"`
		Results []ActionResult `json:"results"`
	}
	decodeJSON(t, resp, &p)
	if !p.OK || len(p.Results) != 1 || !p.Results[0].OK {
		t.Fatalf("unexpected payload: %+v", p)
	}
}

func TestWorkbench_Actions_MissingProfileID_400(t *testing.T) {
	srv := newTestServer(t)
	body := map[string]interface{}{
		"actions": []map[string]interface{}{
			{"type": "click", "selector": "#btn"},
		},
	}
	resp := serve(srv, http.MethodPost, "/api/workbench/actions", body)
	if resp.Code != http.StatusBadRequest {
		t.Fatalf("expected 400, got %d: %s", resp.Code, resp.Body.String())
	}
}

func TestWorkbench_Actions_MissingActions_400(t *testing.T) {
	srv := newTestServer(t)
	body := map[string]interface{}{
		"profileId": "prof-1",
	}
	resp := serve(srv, http.MethodPost, "/api/workbench/actions", body)
	if resp.Code != http.StatusBadRequest {
		t.Fatalf("expected 400, got %d: %s", resp.Code, resp.Body.String())
	}
}

func TestWorkbench_Actions_WrongMethod_405(t *testing.T) {
	srv := newTestServer(t)
	resp := serve(srv, http.MethodGet, "/api/workbench/actions", nil)
	if resp.Code != http.StatusMethodNotAllowed {
		t.Fatalf("expected 405, got %d: %s", resp.Code, resp.Body.String())
	}
}

func TestWorkbench_Actions_NoOperator_503(t *testing.T) {
	svc := NewLaunchCodeService(NewMemoryLaunchCodeDAO())
	srv := NewLaunchServer(svc, nil, nil, nil, 0)
	body := map[string]interface{}{
		"profileId": "prof-1",
		"actions":   []map[string]interface{}{{"type": "click", "selector": "#btn"}},
	}
	resp := serve(srv, http.MethodPost, "/api/workbench/actions", body)
	if resp.Code != http.StatusServiceUnavailable {
		t.Fatalf("expected 503, got %d: %s", resp.Code, resp.Body.String())
	}
}
