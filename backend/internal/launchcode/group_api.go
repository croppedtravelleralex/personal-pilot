package launchcode

import (
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"strings"

	"personal-pilot/backend/internal/browser"
)

func (s *LaunchServer) handleGroups(w http.ResponseWriter, r *http.Request) {
	switch r.Method {
	case http.MethodGet:
		s.handleGroupList(w, r)
	case http.MethodPost:
		s.handleGroupCreate(w, r)
	default:
		writeJSON(w, http.StatusMethodNotAllowed, map[string]interface{}{
			"ok":    false,
			"error": "method not allowed",
		})
	}
}

func (s *LaunchServer) handleGroupByID(w http.ResponseWriter, r *http.Request) {
	id := strings.TrimPrefix(r.URL.Path, "/api/groups/")
	if id == "" || strings.Contains(id, "/") {
		writeJSON(w, http.StatusBadRequest, map[string]interface{}{
			"ok":    false,
			"error": "invalid group id",
		})
		return
	}
	switch r.Method {
	case http.MethodGet:
		s.handleGroupGet(w, r, id)
	case http.MethodPut:
		s.handleGroupUpdate(w, r, id)
	case http.MethodDelete:
		s.handleGroupDelete(w, r, id)
	default:
		writeJSON(w, http.StatusMethodNotAllowed, map[string]interface{}{
			"ok":    false,
			"error": "method not allowed",
		})
	}
}

func (s *LaunchServer) handleGroupList(w http.ResponseWriter, r *http.Request) {
	groups, err := s.browserMgr.GroupDAO.List()
	if err != nil {
		writeJSON(w, http.StatusInternalServerError, map[string]interface{}{
			"ok":    false,
			"error": err.Error(),
		})
		return
	}
	if groups == nil {
		groups = []*browser.Group{}
	}
	writeJSON(w, http.StatusOK, map[string]interface{}{
		"ok":    true,
		"count": len(groups),
		"items": groups,
	})
}

func (s *LaunchServer) handleGroupGet(w http.ResponseWriter, r *http.Request, id string) {
	group, err := s.browserMgr.GroupDAO.GetById(id)
	if err != nil {
		writeJSON(w, http.StatusNotFound, map[string]interface{}{
			"ok":    false,
			"error": err.Error(),
		})
		return
	}
	writeJSON(w, http.StatusOK, map[string]interface{}{
		"ok":    true,
		"group": group,
	})
}

func (s *LaunchServer) handleGroupCreate(w http.ResponseWriter, r *http.Request) {
	var input browser.GroupInput
	dec := json.NewDecoder(io.LimitReader(r.Body, 1<<20))
	dec.DisallowUnknownFields()
	if err := dec.Decode(&input); err != nil {
		writeJSON(w, http.StatusBadRequest, map[string]interface{}{
			"ok":    false,
			"error": "invalid request body",
		})
		return
	}
	if strings.TrimSpace(input.GroupName) == "" {
		writeJSON(w, http.StatusBadRequest, map[string]interface{}{
			"ok":    false,
			"error": "groupName is required",
		})
		return
	}
	group, err := s.browserMgr.GroupDAO.Create(input)
	if err != nil {
		code := http.StatusInternalServerError
		if strings.Contains(err.Error(), "父分组不存在") {
			code = http.StatusBadRequest
		}
		writeJSON(w, code, map[string]interface{}{
			"ok":    false,
			"error": err.Error(),
		})
		return
	}
	writeJSON(w, http.StatusCreated, map[string]interface{}{
		"ok":    true,
		"group": group,
	})
}

func (s *LaunchServer) handleGroupUpdate(w http.ResponseWriter, r *http.Request, id string) {
	var input browser.GroupInput
	dec := json.NewDecoder(io.LimitReader(r.Body, 1<<20))
	dec.DisallowUnknownFields()
	if err := dec.Decode(&input); err != nil {
		writeJSON(w, http.StatusBadRequest, map[string]interface{}{
			"ok":    false,
			"error": "invalid request body",
		})
		return
	}
	if strings.TrimSpace(input.GroupName) == "" {
		writeJSON(w, http.StatusBadRequest, map[string]interface{}{
			"ok":    false,
			"error": "groupName is required",
		})
		return
	}
	group, err := s.browserMgr.GroupDAO.Update(id, input)
	if err != nil {
		code := http.StatusInternalServerError
		if strings.Contains(err.Error(), "父分组不存在") ||
			strings.Contains(err.Error(), "不能将分组设为自己的子分组") ||
			strings.Contains(err.Error(), "不能将分组设为自己的后代分组") {
			code = http.StatusBadRequest
		}
		if strings.Contains(err.Error(), "分组不存在") {
			code = http.StatusNotFound
		}
		writeJSON(w, code, map[string]interface{}{
			"ok":    false,
			"error": err.Error(),
		})
		return
	}
	writeJSON(w, http.StatusOK, map[string]interface{}{
		"ok":    true,
		"group": group,
	})
}

func (s *LaunchServer) handleGroupDelete(w http.ResponseWriter, r *http.Request, id string) {
	err := s.browserMgr.GroupDAO.Delete(id)
	if err != nil {
		code := http.StatusInternalServerError
		if strings.Contains(err.Error(), "分组不存在") {
			code = http.StatusNotFound
		}
		writeJSON(w, code, map[string]interface{}{
			"ok":    false,
			"error": err.Error(),
		})
		return
	}
	writeJSON(w, http.StatusOK, map[string]interface{}{
		"ok":  true,
		"msg": fmt.Sprintf("group %s deleted", id),
	})
}
