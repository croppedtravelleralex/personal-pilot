package launchcode

import (
	"net/http"
	"strings"

	"personal-pilot/backend/internal/browser"
)

// CoreAPI 内核 API

// handleCores GET /api/cores
func (s *LaunchServer) handleCores(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodGet {
		writeJSON(w, http.StatusMethodNotAllowed, map[string]interface{}{
			"ok":    false,
			"error": "method not allowed",
		})
		return
	}

	if s.browserMgr == nil {
		writeJSON(w, http.StatusServiceUnavailable, map[string]interface{}{
			"ok":    false,
			"error": "browser manager not available",
		})
		return
	}

	var cores []browser.Core
	if s.browserMgr.CoreDAO != nil {
		var err error
		cores, err = s.browserMgr.CoreDAO.List()
		if err != nil {
			writeJSON(w, http.StatusInternalServerError, map[string]interface{}{
				"ok":    false,
				"error": err.Error(),
			})
			return
		}
	} else {
		cores = s.browserMgr.ListCores()
	}

	if cores == nil {
		cores = []browser.Core{}
	}

	writeJSON(w, http.StatusOK, map[string]interface{}{
		"ok":    true,
		"count": len(cores),
		"items": cores,
	})
}

// handleCoreByID DELETE /api/cores/{id}
func (s *LaunchServer) handleCoreByID(w http.ResponseWriter, r *http.Request) {
	id := strings.TrimPrefix(r.URL.Path, "/api/cores/")
	id = strings.TrimSpace(id)
	if id == "" || strings.Contains(id, "/") {
		writeJSON(w, http.StatusBadRequest, map[string]interface{}{
			"ok":    false,
			"error": "invalid core id",
		})
		return
	}

	if r.Method != http.MethodDelete {
		writeJSON(w, http.StatusMethodNotAllowed, map[string]interface{}{
			"ok":    false,
			"error": "method not allowed",
		})
		return
	}

	if s.browserMgr == nil {
		writeJSON(w, http.StatusServiceUnavailable, map[string]interface{}{
			"ok":    false,
			"error": "browser manager not available",
		})
		return
	}

	var err error
	if s.browserMgr.CoreDAO != nil {
		err = s.browserMgr.CoreDAO.Delete(id)
	} else {
		err = s.browserMgr.DeleteCore(id)
	}

	if err != nil {
		writeJSON(w, http.StatusInternalServerError, map[string]interface{}{
			"ok":    false,
			"error": err.Error(),
		})
		return
	}

	writeJSON(w, http.StatusOK, map[string]interface{}{
		"ok":      true,
		"deleted": true,
		"id":      id,
	})
}
