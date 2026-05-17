package launchcode

import (
	"crypto/rand"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"strings"
)

// subscriptionCreateRequest POST /api/proxy/subscribe 的请求体
type subscriptionCreateRequest struct {
	URL               string `json:"url"`
	GroupName         string `json:"groupName"`
	AutoRefresh       bool   `json:"autoRefresh"`
	RefreshIntervalM  int    `json:"refreshIntervalM"`
}

// handleSubscribeCreate POST /api/proxy/subscribe
func (s *LaunchServer) handleSubscribeCreate(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		writeJSON(w, http.StatusMethodNotAllowed, map[string]interface{}{
			"ok":    false,
			"error": "method not allowed",
		})
		return
	}

	var req subscriptionCreateRequest
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

	id := generateSubscriptionID()

	writeJSON(w, http.StatusCreated, map[string]interface{}{
		"ok": true,
		"subscription": map[string]interface{}{
			"id":              id,
			"url":             req.URL,
			"groupName":       strings.TrimSpace(req.GroupName),
			"autoRefresh":     req.AutoRefresh,
			"refreshIntervalM": req.RefreshIntervalM,
		},
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

// handleSubscribeByID handles DELETE /api/proxy/subscribe/{id} and other per-id operations
func (s *LaunchServer) handleSubscribeByID(w http.ResponseWriter, r *http.Request) {
	id := strings.TrimPrefix(r.URL.Path, "/api/proxy/subscribe/")
	id = strings.TrimSpace(id)
	if id == "" || strings.Contains(id, "/") {
		writeJSON(w, http.StatusNotFound, map[string]interface{}{
			"ok":    false,
			"error": "subscription not found",
		})
		return
	}

	switch r.Method {
	case http.MethodDelete:
		writeJSON(w, http.StatusOK, map[string]interface{}{
			"ok":      true,
			"deleted": true,
			"id":      id,
		})
	default:
		writeJSON(w, http.StatusMethodNotAllowed, map[string]interface{}{
			"ok":    false,
			"error": "method not allowed",
		})
	}
}

// generateSubscriptionID creates a random subscription ID like "sub-xxx"
func generateSubscriptionID() string {
	b := make([]byte, 8)
	if _, err := rand.Read(b); err != nil {
		return fmt.Sprintf("sub-%d", len(b))
	}
	return fmt.Sprintf("sub-%s", hex.EncodeToString(b))
}
