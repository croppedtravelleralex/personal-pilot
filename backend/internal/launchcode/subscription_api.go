package launchcode

import (
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"strings"
	"time"
)

// SubscribeRequest POST /api/proxy/subscribe 的请求体
type SubscribeRequest struct {
	URL              string `json:"url"`
	GroupName        string `json:"groupName"`
	AutoRefresh      bool   `json:"autoRefresh"`
	RefreshIntervalM int    `json:"refreshIntervalM"`
}

func (s *LaunchServer) handleSubscribe(w http.ResponseWriter, r *http.Request) {
	switch r.Method {
	case http.MethodPost:
		s.handleSubscribeCreate(w, r)
	default:
		writeJSON(w, http.StatusMethodNotAllowed, map[string]interface{}{
			"ok":    false,
			"error": "method not allowed",
		})
	}
}

// handleSubscribeCreate POST /api/proxy/subscribe
func (s *LaunchServer) handleSubscribeCreate(w http.ResponseWriter, r *http.Request) {
	var req SubscribeRequest
	dec := json.NewDecoder(io.LimitReader(r.Body, 1<<20))
	dec.DisallowUnknownFields()
	if err := dec.Decode(&req); err != nil {
		writeJSON(w, http.StatusBadRequest, map[string]interface{}{
			"ok":    false,
			"error": "invalid request body",
		})
		return
	}

	if strings.TrimSpace(req.URL) == "" {
		writeJSON(w, http.StatusBadRequest, map[string]interface{}{
			"ok":    false,
			"error": "url is required",
		})
		return
	}

	subID := fmt.Sprintf("sub-%d", time.Now().UnixMilli())

	writeJSON(w, http.StatusCreated, map[string]interface{}{
		"ok": true,
		"subscription": map[string]interface{}{
			"id":               subID,
			"url":              req.URL,
			"groupName":        req.GroupName,
			"autoRefresh":      req.AutoRefresh,
			"refreshIntervalM": req.RefreshIntervalM,
		},
	})
}

// handleSubscribeByID handles /api/proxy/subscribe/{id}
func (s *LaunchServer) handleSubscribeByID(w http.ResponseWriter, r *http.Request) {
	id := strings.TrimPrefix(r.URL.Path, "/api/proxy/subscribe/")
	id = strings.TrimSpace(id)
	if id == "" || strings.Contains(id, "/") {
		writeJSON(w, http.StatusBadRequest, map[string]interface{}{
			"ok":    false,
			"error": "invalid subscription id",
		})
		return
	}

	switch r.Method {
	case http.MethodDelete:
		s.handleSubscribeDelete(w, r, id)
	default:
		writeJSON(w, http.StatusMethodNotAllowed, map[string]interface{}{
			"ok":    false,
			"error": "method not allowed",
		})
	}
}

// handleSubscribeDelete DELETE /api/proxy/subscribe/{id}
func (s *LaunchServer) handleSubscribeDelete(w http.ResponseWriter, _ *http.Request, id string) {
	writeJSON(w, http.StatusOK, map[string]interface{}{
		"ok":      true,
		"deleted": true,
		"id":      id,
	})
}

// handleSubscribeList GET /api/proxy/subscribe/list
func (s *LaunchServer) handleSubscribeList(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodGet {
		writeJSON(w, http.StatusMethodNotAllowed, map[string]interface{}{
			"ok":    false,
			"error": "method not allowed",
		})
		return
	}

	writeJSON(w, http.StatusOK, map[string]interface{}{
		"ok":    true,
		"count": 0,
		"items": []interface{}{},
	})
}
