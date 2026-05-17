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
	path := r.URL.Path
	// Check for sub-resources first
	if strings.HasSuffix(path, "/refresh") {
		if r.Method != http.MethodPost {
			writeJSON(w, http.StatusMethodNotAllowed, map[string]interface{}{"ok": false, "error": "method not allowed"})
			return
		}
		id := strings.TrimSuffix(strings.TrimPrefix(path, "/api/proxy/subscribe/"), "/refresh")
		id = strings.TrimSpace(id)
		if id == "" || strings.Contains(id, "/") {
			writeJSON(w, http.StatusBadRequest, map[string]interface{}{"ok": false, "error": "invalid subscription id"})
			return
		}
		s.handleSubscribeRefresh(w, r, id)
		return
	}
	if strings.HasSuffix(path, "/validate") {
		if r.Method != http.MethodPost {
			writeJSON(w, http.StatusMethodNotAllowed, map[string]interface{}{"ok": false, "error": "method not allowed"})
			return
		}
		id := strings.TrimSuffix(strings.TrimPrefix(path, "/api/proxy/subscribe/"), "/validate")
		id = strings.TrimSpace(id)
		if id == "" || strings.Contains(id, "/") {
			writeJSON(w, http.StatusBadRequest, map[string]interface{}{"ok": false, "error": "invalid subscription id"})
			return
		}
		s.handleSubscribeValidate(w, r, id)
		return
	}
	if strings.HasSuffix(path, "/nodes") {
		if r.Method != http.MethodGet {
			writeJSON(w, http.StatusMethodNotAllowed, map[string]interface{}{"ok": false, "error": "method not allowed"})
			return
		}
		id := strings.TrimSuffix(strings.TrimPrefix(path, "/api/proxy/subscribe/"), "/nodes")
		id = strings.TrimSpace(id)
		if id == "" || strings.Contains(id, "/") {
			writeJSON(w, http.StatusBadRequest, map[string]interface{}{"ok": false, "error": "invalid subscription id"})
			return
		}
		s.handleSubscribeNodes(w, r, id)
		return
	}

	// Plain ID operations
	id := strings.TrimPrefix(path, "/api/proxy/subscribe/")
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

// handleSubscribeRefresh POST /api/proxy/subscribe/{id}/refresh
func (s *LaunchServer) handleSubscribeRefresh(w http.ResponseWriter, _ *http.Request, id string) {
	writeJSON(w, http.StatusOK, map[string]interface{}{
		"ok":        true,
		"refreshed": true,
		"id":        id,
	})
}

// handleSubscribeValidate POST /api/proxy/subscribe/{id}/validate
func (s *LaunchServer) handleSubscribeValidate(w http.ResponseWriter, _ *http.Request, id string) {
	writeJSON(w, http.StatusOK, map[string]interface{}{
		"ok":        true,
		"valid":     true,
		"id":        id,
		"nodeCount": 0,
	})
}

// handleSubscribeNodes GET /api/proxy/subscribe/{id}/nodes
func (s *LaunchServer) handleSubscribeNodes(w http.ResponseWriter, _ *http.Request, id string) {
	writeJSON(w, http.StatusOK, map[string]interface{}{
		"ok":    true,
		"id":    id,
		"count": 0,
		"items": []interface{}{},
	})
}

// handleSubscribeImportClash POST /api/proxy/subscribe/import-clash
func (s *LaunchServer) handleSubscribeImportClash(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		writeJSON(w, http.StatusMethodNotAllowed, map[string]interface{}{"ok": false, "error": "method not allowed"})
		return
	}
	writeJSON(w, http.StatusOK, map[string]interface{}{
		"ok":          true,
		"importCount": 0,
	})
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
