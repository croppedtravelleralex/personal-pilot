package launchcode

import (
	"bytes"
	"encoding/json"
	"errors"
	"net/http"
	"net/http/httptest"
	"testing"

	"personal-pilot/backend/internal/behavior"
)

type recordingHTTPTestAPI struct {
	startErr   error
	detailErr  error
	cleanupHit int
}

func (a *recordingHTTPTestAPI) StartRecording(profileId string) error {
	return a.startErr
}

func (a *recordingHTTPTestAPI) StopRecording(profileId string, name string) (*behavior.Recording, error) {
	return nil, nil
}

func (a *recordingHTTPTestAPI) ListRecordings() ([]*behavior.Recording, error) {
	return nil, nil
}

func (a *recordingHTTPTestAPI) ListRecordingSummaries() ([]*behavior.RecordingSummary, error) {
	return []*behavior.RecordingSummary{
		{
			ID:         "rec-1",
			Name:       "summary",
			EventCount: 2,
			DurationMs: 100,
			ViewportW:  1920,
			ViewportH:  1080,
			CreatedAt:  "2025-06-01T00:00:00Z",
		},
	}, nil
}

func (a *recordingHTTPTestAPI) GetRecording(id string) (*behavior.Recording, error) {
	if a.detailErr != nil {
		return nil, a.detailErr
	}
	return &behavior.Recording{
		ID:         id,
		Name:       "detail",
		EventCount: 2,
		Events: []behavior.RecordedEvent{
			{T: 0, Type: "move"},
			{T: 100, Type: "scroll"},
		},
		DurationMs: 100,
		ViewportW:  1920,
		ViewportH:  1080,
		CreatedAt:  "2025-06-01T00:00:00Z",
	}, nil
}

func (a *recordingHTTPTestAPI) GetRecordingDetail(id string, eventOffset int, eventLimit int) (*behavior.RecordingDetailPage, error) {
	rec, err := a.GetRecording(id)
	if err != nil {
		return nil, err
	}
	return rec.DetailPage(eventOffset, eventLimit), nil
}

func (a *recordingHTTPTestAPI) DeleteRecording(id string) error {
	return nil
}

func (a *recordingHTTPTestAPI) PlayRecording(profileId string, recordingId string, variation behavior.VariationConfig) error {
	return nil
}

func (a *recordingHTTPTestAPI) StopPlayback(profileId string) error {
	return nil
}

func (a *recordingHTTPTestAPI) QuickRecord(profileId string) (*behavior.Recording, error) {
	return nil, errors.New("boom")
}

func (a *recordingHTTPTestAPI) CleanupStaleSessions() error {
	a.cleanupHit++
	return nil
}

func (a *recordingHTTPTestAPI) GetBehaviorPresets() []behavior.Profile {
	return nil
}

func (a *recordingHTTPTestAPI) ActiveRecordingStatus() (*behavior.ActiveRecordingStatus, error) {
	return &behavior.ActiveRecordingStatus{
		Active:     true,
		Count:      1,
		ProfileID:  "profile-1",
		ProfileIDs: []string{"profile-1"},
	}, nil
}

type testHTTPStatusError struct {
	status int
	msg    string
}

func (e testHTTPStatusError) Error() string {
	return e.msg
}

func (e testHTTPStatusError) HTTPStatusCode() int {
	return e.status
}

func serveRecordingRequest(api RecordingAPI, method string, path string, body any) *httptest.ResponseRecorder {
	srv := NewLaunchServer(NewLaunchCodeService(NewMemoryLaunchCodeDAO()), nil, api, nil, 0)
	var reader *bytes.Reader
	if body == nil {
		reader = bytes.NewReader(nil)
	} else {
		data, _ := json.Marshal(body)
		reader = bytes.NewReader(data)
	}
	req := httptest.NewRequest(method, path, reader)
	w := httptest.NewRecorder()
	srv.buildHandler(false).ServeHTTP(w, req)
	return w
}

func TestRecordingHTTPListReturnsSummariesAndDetailReturnsEvents(t *testing.T) {
	api := &recordingHTTPTestAPI{}

	listResp := serveRecordingRequest(api, http.MethodGet, "/api/recording/list", nil)
	if listResp.Code != http.StatusOK {
		t.Fatalf("list status = %d body=%s", listResp.Code, listResp.Body.String())
	}
	var listPayload struct {
		Items []struct {
			ID         string                   `json:"id"`
			EventCount int                      `json:"eventCount"`
			Events     []behavior.RecordedEvent `json:"events"`
		} `json:"items"`
	}
	if err := json.Unmarshal(listResp.Body.Bytes(), &listPayload); err != nil {
		t.Fatalf("decode list: %v", err)
	}
	if len(listPayload.Items) != 1 {
		t.Fatalf("list item count = %d, want 1", len(listPayload.Items))
	}
	if listPayload.Items[0].EventCount != 2 {
		t.Fatalf("list eventCount = %d, want 2", listPayload.Items[0].EventCount)
	}
	if listPayload.Items[0].Events != nil {
		t.Fatalf("list should not include events, got %#v", listPayload.Items[0].Events)
	}

	detailResp := serveRecordingRequest(api, http.MethodGet, "/api/recording/rec-1", nil)
	if detailResp.Code != http.StatusOK {
		t.Fatalf("detail status = %d body=%s", detailResp.Code, detailResp.Body.String())
	}
	var detailPayload struct {
		Recording behavior.Recording `json:"recording"`
	}
	if err := json.Unmarshal(detailResp.Body.Bytes(), &detailPayload); err != nil {
		t.Fatalf("decode detail: %v", err)
	}
	if len(detailPayload.Recording.Events) != 2 {
		t.Fatalf("detail events length = %d, want 2", len(detailPayload.Recording.Events))
	}

	pagedResp := serveRecordingRequest(api, http.MethodGet, "/api/recording/rec-1?eventOffset=1&eventLimit=1", nil)
	if pagedResp.Code != http.StatusOK {
		t.Fatalf("paged detail status = %d body=%s", pagedResp.Code, pagedResp.Body.String())
	}
	var pagedPayload struct {
		Recording struct {
			ID     string                   `json:"id"`
			Events []behavior.RecordedEvent `json:"events"`
		} `json:"recording"`
		Events      []behavior.RecordedEvent     `json:"events"`
		EventOffset int                          `json:"eventOffset"`
		EventLimit  int                          `json:"eventLimit"`
		EventTotal  int                          `json:"eventTotal"`
		Stats       behavior.RecordingEventStats `json:"stats"`
	}
	if err := json.Unmarshal(pagedResp.Body.Bytes(), &pagedPayload); err != nil {
		t.Fatalf("decode paged detail: %v", err)
	}
	if pagedPayload.EventOffset != 1 || pagedPayload.EventLimit != 1 || pagedPayload.EventTotal != 2 {
		t.Fatalf("unexpected page metadata: %+v", pagedPayload)
	}
	if len(pagedPayload.Events) != 1 || pagedPayload.Events[0].Type != "scroll" {
		t.Fatalf("paged events = %#v, want one scroll", pagedPayload.Events)
	}
	if pagedPayload.Recording.Events != nil {
		t.Fatalf("paged recording summary should not include events, got %#v", pagedPayload.Recording.Events)
	}
	if pagedPayload.Stats.Total != 2 || pagedPayload.Stats.Move != 1 || pagedPayload.Stats.Scroll != 1 {
		t.Fatalf("unexpected stats: %+v", pagedPayload.Stats)
	}
}

func TestRecordingHTTPCleanupSupportsPostAndDeprecatedGet(t *testing.T) {
	api := &recordingHTTPTestAPI{}

	postResp := serveRecordingRequest(api, http.MethodPost, "/api/recording/sessions/cleanup", nil)
	if postResp.Code != http.StatusOK {
		t.Fatalf("POST cleanup status = %d body=%s", postResp.Code, postResp.Body.String())
	}

	getResp := serveRecordingRequest(api, http.MethodGet, "/api/recording/sessions/cleanup", nil)
	if getResp.Code != http.StatusOK {
		t.Fatalf("GET cleanup status = %d body=%s", getResp.Code, getResp.Body.String())
	}
	var payload map[string]any
	if err := json.Unmarshal(getResp.Body.Bytes(), &payload); err != nil {
		t.Fatalf("decode GET cleanup: %v", err)
	}
	if payload["deprecated"] != true {
		t.Fatalf("GET cleanup should mark compatibility response deprecated: %#v", payload)
	}
	if api.cleanupHit != 2 {
		t.Fatalf("cleanup hit count = %d, want 2", api.cleanupHit)
	}
}

func TestRecordingHTTPStatusEndpoint(t *testing.T) {
	resp := serveRecordingRequest(&recordingHTTPTestAPI{}, http.MethodGet, "/api/recording/status", nil)
	if resp.Code != http.StatusOK {
		t.Fatalf("status code = %d body=%s", resp.Code, resp.Body.String())
	}
	var payload struct {
		OK        bool   `json:"ok"`
		Active    bool   `json:"active"`
		Count     int    `json:"count"`
		ProfileID string `json:"profileId"`
	}
	if err := json.Unmarshal(resp.Body.Bytes(), &payload); err != nil {
		t.Fatalf("decode status: %v", err)
	}
	if !payload.OK || !payload.Active || payload.Count != 1 || payload.ProfileID != "profile-1" {
		t.Fatalf("unexpected status payload: %+v", payload)
	}
}

func TestRecordingHTTPBusinessErrorStatusCodes(t *testing.T) {
	cases := []struct {
		name   string
		api    RecordingAPI
		method string
		path   string
		body   any
		want   int
	}{
		{
			name:   "bad request",
			api:    &recordingHTTPTestAPI{},
			method: http.MethodPost,
			path:   "/api/recording/start",
			body:   map[string]string{},
			want:   http.StatusBadRequest,
		},
		{
			name:   "not found",
			api:    &recordingHTTPTestAPI{detailErr: behavior.ErrRecordingNotFound},
			method: http.MethodGet,
			path:   "/api/recording/missing",
			want:   http.StatusNotFound,
		},
		{
			name:   "conflict",
			api:    &recordingHTTPTestAPI{startErr: testHTTPStatusError{status: http.StatusConflict, msg: "already recording"}},
			method: http.MethodPost,
			path:   "/api/recording/start",
			body:   map[string]string{"profileId": "profile-1"},
			want:   http.StatusConflict,
		},
		{
			name:   "service unavailable",
			api:    nil,
			method: http.MethodPost,
			path:   "/api/recording/start",
			body:   map[string]string{"profileId": "profile-1"},
			want:   http.StatusServiceUnavailable,
		},
		{
			name:   "internal",
			api:    &recordingHTTPTestAPI{},
			method: http.MethodPost,
			path:   "/api/recording/quick",
			body:   map[string]string{"profileId": "profile-1"},
			want:   http.StatusInternalServerError,
		},
	}

	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			resp := serveRecordingRequest(tc.api, tc.method, tc.path, tc.body)
			if resp.Code != tc.want {
				t.Fatalf("status = %d, want %d body=%s", resp.Code, tc.want, resp.Body.String())
			}
		})
	}
}
