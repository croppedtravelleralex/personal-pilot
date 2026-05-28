package launchcode

import (
	"encoding/json"
	"io"
	"net/http"
	"strings"
	"sync"
	"time"

	"personal-pilot/backend/internal/workflow"
	"personal-pilot/backend/internal/workflow/plugin"
)

type workflowRuntime struct {
	mu        sync.RWMutex
	workflows map[string]workflow.Workflow
	states    map[string]workflow.ExecutionState
	plugins   plugin.Registry
}

func newWorkflowRuntime() *workflowRuntime {
	return &workflowRuntime{workflows: map[string]workflow.Workflow{}, states: map[string]workflow.ExecutionState{}, plugins: plugin.NewRegistry()}
}

func (rt *workflowRuntime) put(wf workflow.Workflow) workflow.ExecutionState {
	rt.mu.Lock()
	defer rt.mu.Unlock()
	if wf.ID == "" {
		wf.ID = "wf-" + time.Now().UTC().Format("20060102150405")
	}
	rt.workflows[wf.ID] = wf
	state := workflow.Engine{}.Plan(wf)
	rt.states[wf.ID] = state
	return state
}

func (rt *workflowRuntime) execute(id string) (workflow.ExecutionState, bool) {
	rt.mu.Lock()
	defer rt.mu.Unlock()
	wf, ok := rt.workflows[id]
	if !ok {
		return workflow.ExecutionState{}, false
	}
	state := workflow.Engine{}.ExecuteReady(wf, rt.states[id])
	rt.states[id] = state
	return state, true
}

func (rt *workflowRuntime) status(id string) (workflow.ExecutionState, bool) {
	rt.mu.RLock()
	defer rt.mu.RUnlock()
	state, ok := rt.states[id]
	return state, ok
}

func (s *LaunchServer) workflowRuntime() *workflowRuntime {
	if s == nil {
		return newWorkflowRuntime()
	}
	if s.workflow == nil {
		s.workflow = newWorkflowRuntime()
	}
	return s.workflow
}
func (s *LaunchServer) handleWorkflowCreate(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		writeJSON(w, http.StatusMethodNotAllowed, map[string]interface{}{"ok": false, "error": "only POST is allowed"})
		return
	}
	var wf workflow.Workflow
	if err := json.NewDecoder(io.LimitReader(r.Body, 1<<20)).Decode(&wf); err != nil {
		writeJSON(w, http.StatusBadRequest, map[string]interface{}{"ok": false, "error": "invalid JSON: " + err.Error()})
		return
	}
	state := s.workflowRuntime().put(wf)
	writeJSON(w, http.StatusOK, map[string]interface{}{"ok": true, "workflowId": state.WorkflowID, "state": state})
}

func (s *LaunchServer) handleWorkflowByID(w http.ResponseWriter, r *http.Request) {
	path := strings.TrimPrefix(r.URL.Path, "/api/workflow/")
	path = strings.Trim(path, "/")
	parts := strings.Split(path, "/")
	if len(parts) == 0 || parts[0] == "" {
		writeJSON(w, http.StatusNotFound, map[string]interface{}{"ok": false, "error": "workflow id required"})
		return
	}
	id := parts[0]
	action := "status"
	if len(parts) > 1 {
		action = parts[1]
	}
	switch {
	case r.Method == http.MethodPost && action == "execute":
		state, ok := s.workflowRuntime().execute(id)
		if !ok {
			writeJSON(w, http.StatusNotFound, map[string]interface{}{"ok": false, "error": "workflow not found"})
			return
		}
		writeJSON(w, http.StatusOK, map[string]interface{}{"ok": true, "workflowId": id, "state": state})
	case r.Method == http.MethodGet && action == "status":
		state, ok := s.workflowRuntime().status(id)
		if !ok {
			writeJSON(w, http.StatusNotFound, map[string]interface{}{"ok": false, "error": "workflow not found"})
			return
		}
		writeJSON(w, http.StatusOK, map[string]interface{}{"ok": true, "workflowId": id, "state": state})
	case r.Method == http.MethodPost && action == "export":
		state, ok := s.workflowRuntime().status(id)
		if !ok {
			writeJSON(w, http.StatusNotFound, map[string]interface{}{"ok": false, "error": "workflow not found"})
			return
		}
		writeJSON(w, http.StatusOK, map[string]interface{}{"ok": true, "workflowId": id, "state": state, "format": "json"})
	default:
		writeJSON(w, http.StatusMethodNotAllowed, map[string]interface{}{"ok": false, "error": "unsupported workflow operation"})
	}
}

func (s *LaunchServer) handlePluginInstall(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		writeJSON(w, http.StatusMethodNotAllowed, map[string]interface{}{"ok": false, "error": "only POST is allowed"})
		return
	}
	writeJSON(w, http.StatusOK, map[string]interface{}{"ok": true, "installed": false, "status": "builtin_registry_only"})
}
