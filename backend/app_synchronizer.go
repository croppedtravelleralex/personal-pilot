package backend

import (
	"ant-chrome/backend/internal/behavior"
	"encoding/json"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"sync"
	"time"

	"github.com/gorilla/websocket"
)

// ─── Types ────────────────────────────────────────────────────────────────────

// SyncWindow 表示一个运行中的浏览器窗口。
type SyncWindow struct {
	ProfileID   string `json:"profileId"`
	ProfileName string `json:"profileName"`
	URL         string `json:"url"`
	Title       string `json:"title"`
	DebugPort   int    `json:"debugPort"`
	Pid         int    `json:"pid"`
	Status      string `json:"status"` // "running" | "loading"
	GroupID     string `json:"groupId"`
}

// SyncGroup 表示一个运行中实例的分组。
type SyncGroup struct {
	ID      string       `json:"id"`
	Name    string       `json:"name"`
	Windows []SyncWindow `json:"windows"`
}

// SyncOperation 记录一次广播操作。
type SyncOperation struct {
	ID        string                 `json:"id"`
	Type      string                 `json:"type"`
	GroupID   string                 `json:"groupId"`
	Payload   map[string]interface{} `json:"payload,omitempty"`
	Timestamp string                 `json:"timestamp"`
	Status    string                 `json:"status"`
	Error     string                 `json:"error,omitempty"`
}

// SyncWindowPlacement records the result of an external top-level window move.
type SyncWindowPlacement struct {
	ProfileID   string `json:"profileId"`
	ProfileName string `json:"profileName"`
	Pid         int    `json:"pid"`
	Found       bool   `json:"found"`
	X           int    `json:"x"`
	Y           int    `json:"y"`
	Width       int    `json:"width"`
	Height      int    `json:"height"`
	Error       string `json:"error,omitempty"`
}

// WorkbenchTask is a persisted view of recent frontend workbench tasks.
type WorkbenchTask struct {
	ID          string `json:"id"`
	Type        string `json:"type"`
	ProfileID   string `json:"profileId"`
	ProfileName string `json:"profileName"`
	Detail      string `json:"detail"`
	Status      string `json:"status"`
	CreatedAt   string `json:"createdAt"`
	UpdatedAt   string `json:"updatedAt"`
	Error       string `json:"error,omitempty"`
}

type workbenchWindowRect struct {
	x      int
	y      int
	width  int
	height int
}

// ─── State ────────────────────────────────────────────────────────────────────

type syncState struct {
	mu         sync.RWMutex
	operations []SyncOperation
	opSeq      int
}

var syncSt = &syncState{}

const (
	maxOps            = 200
	maxWorkbenchTasks = 200
	cdpOpTimeout      = 5 * time.Second
)

// ─── CDP Helpers ──────────────────────────────────────────────────────────────

// cdpNavigate 通过 CDP 让浏览器实例导航到指定 URL。
func cdpNavigate(debugPort int, url string) error {
	conn, err := cdpConnect(debugPort)
	if err != nil {
		return err
	}
	defer conn.Close()

	_, err = cdpSend(conn, "Page.navigate", map[string]interface{}{
		"url": url,
	})
	return err
}

// cdpReload 通过 CDP 让浏览器实例重新加载当前页面。
func cdpReload(debugPort int) error {
	conn, err := cdpConnect(debugPort)
	if err != nil {
		return err
	}
	defer conn.Close()

	_, err = cdpSend(conn, "Page.reload", map[string]interface{}{
		"ignoreCache": true,
	})
	return err
}

// cdpCaptureScreenshot captures the current page as a browser-rendered preview.
func cdpCaptureScreenshot(debugPort int) (string, error) {
	conn, err := cdpConnect(debugPort)
	if err != nil {
		return "", err
	}
	defer conn.Close()

	raw, err := cdpSend(conn, "Page.captureScreenshot", map[string]interface{}{
		"format":      "jpeg",
		"quality":     60,
		"fromSurface": true,
	})
	if err != nil {
		return "", err
	}

	var result struct {
		Data string `json:"data"`
	}
	if err := json.Unmarshal(raw, &result); err != nil {
		return "", fmt.Errorf("parse screenshot response: %w", err)
	}
	if result.Data == "" {
		return "", fmt.Errorf("empty screenshot data")
	}
	return "data:image/jpeg;base64," + result.Data, nil
}

// cdpFetchURL 通过 CDP 获取浏览器当前 URL 和标题（best effort，失败返回空字符串）。
func cdpFetchURL(debugPort int) (url, title string) {
	conn, err := cdpConnect(debugPort)
	if err != nil {
		return "", ""
	}
	defer conn.Close()

	// 并行获取 URL 和 Title
	type evalResult struct {
		Result struct {
			Value string `json:"value"`
		} `json:"result"`
	}

	raw, err := cdpSend(conn, "Runtime.evaluate", map[string]interface{}{
		"expression":    "window.location.href",
		"returnByValue": true,
	})
	if err == nil {
		var r evalResult
		if json.Unmarshal(raw, &r) == nil {
			url = trimJSONString(r.Result.Value)
		}
	}

	raw, err = cdpSend(conn, "Runtime.evaluate", map[string]interface{}{
		"expression":    "document.title",
		"returnByValue": true,
	})
	if err == nil {
		var r evalResult
		if json.Unmarshal(raw, &r) == nil {
			title = trimJSONString(r.Result.Value)
		}
	}

	return url, title
}

func trimJSONString(s string) string {
	if len(s) >= 2 && s[0] == '"' && s[len(s)-1] == '"' {
		return s[1 : len(s)-1]
	}
	return s
}

// cdpConnect 连接浏览器页面级 CDP 端点（通过 behavior.ConnectPageCDP）。
func cdpConnect(debugPort int) (*websocket.Conn, error) {
	return behavior.ConnectPageCDP(debugPort)
}

// cdpSend 发送 CDP 命令并返回结果。
func cdpSend(ws *websocket.Conn, method string, params interface{}) (json.RawMessage, error) {
	type req struct {
		ID     int         `json:"id"`
		Method string      `json:"method"`
		Params interface{} `json:"params,omitempty"`
	}
	type resp struct {
		ID     int             `json:"id"`
		Result json.RawMessage `json:"result"`
		Error  *struct {
			Message string `json:"message"`
		} `json:"error"`
	}

	id := int(time.Now().UnixNano() % 1000000000)
	ws.SetWriteDeadline(time.Now().Add(cdpOpTimeout))
	if err := ws.WriteJSON(req{ID: id, Method: method, Params: params}); err != nil {
		return nil, fmt.Errorf("write %s: %w", method, err)
	}

	ws.SetReadDeadline(time.Now().Add(cdpOpTimeout))
	for {
		var raw map[string]json.RawMessage
		if err := ws.ReadJSON(&raw); err != nil {
			return nil, fmt.Errorf("read %s resp: %w", method, err)
		}
		rawID, ok := raw["id"]
		if !ok {
			continue
		}
		var gotID int
		if err := json.Unmarshal(rawID, &gotID); err != nil || gotID != id {
			continue
		}

		encoded, err := json.Marshal(raw)
		if err != nil {
			return nil, fmt.Errorf("marshal %s resp: %w", method, err)
		}
		var r resp
		if err := json.Unmarshal(encoded, &r); err != nil {
			return nil, fmt.Errorf("decode %s resp: %w", method, err)
		}
		if r.Error != nil {
			return nil, fmt.Errorf("cdp error %s: %s", method, r.Error.Message)
		}
		return r.Result, nil
	}
}

func normalizeWorkbenchURL(raw string) (string, error) {
	value := strings.TrimSpace(raw)
	if value == "" {
		return "", fmt.Errorf("URL 不能为空")
	}
	lower := strings.ToLower(value)
	if strings.HasPrefix(lower, "http://") ||
		strings.HasPrefix(lower, "https://") ||
		strings.HasPrefix(lower, "about:") ||
		strings.HasPrefix(lower, "chrome:") ||
		strings.HasPrefix(lower, "file:") {
		return value, nil
	}
	return "https://" + value, nil
}

func (a *App) runningProfileForWorkbench(profileID string) (*BrowserProfile, error) {
	a.browserMgr.Mutex.Lock()

	profile, exists := a.browserMgr.Profiles[profileID]
	if !exists || profile == nil {
		a.browserMgr.Mutex.Unlock()
		return nil, fmt.Errorf("实例不存在: %s", profileID)
	}
	if !profile.Running {
		a.browserMgr.Mutex.Unlock()
		return nil, fmt.Errorf("实例未运行: %s", profile.ProfileName)
	}
	if profile.DebugPort <= 0 || !profile.DebugReady {
		a.browserMgr.Mutex.Unlock()
		return nil, fmt.Errorf("实例调试接口未就绪: %s", profile.ProfileName)
	}
	snapshot := *profile
	a.browserMgr.Mutex.Unlock()
	if err := a.validateProfileCDPOwnership(&snapshot); err != nil {
		return nil, err
	}
	return &snapshot, nil
}

func (a *App) runningProfileForWindowAction(profileID string) (*BrowserProfile, error) {
	a.browserMgr.Mutex.Lock()
	defer a.browserMgr.Mutex.Unlock()

	profile, exists := a.browserMgr.Profiles[profileID]
	if !exists || profile == nil {
		return nil, fmt.Errorf("实例不存在: %s", profileID)
	}
	if !profile.Running {
		return nil, fmt.Errorf("实例未运行: %s", profile.ProfileName)
	}
	if profile.Pid <= 0 {
		return nil, fmt.Errorf("实例进程未就绪: %s", profile.ProfileName)
	}
	snapshot := *profile
	return &snapshot, nil
}

// ─── Wails-bound Methods ──────────────────────────────────────────────────────

// SynchronizerListGroups 返回按 GroupId 分组的运行中实例。
func (a *App) SynchronizerListGroups() ([]SyncGroup, error) {
	instances := a.GetRunningInstances()

	// 收集 groupId，同时获取每个实例的 URL
	type groupKey struct {
		id   string
		name string
	}
	groupMap := make(map[groupKey][]SyncWindow)
	groupOrder := make([]groupKey, 0)

	for _, inst := range instances {
		gid := inst.GroupId
		gname := inst.GroupId
		if gid == "" {
			gid = "__ungrouped__"
			gname = "未分组"
		}

		key := groupKey{id: gid, name: gname}
		if _, exists := groupMap[key]; !exists {
			groupOrder = append(groupOrder, key)
		}

		status := "running"
		if !inst.DebugReady {
			status = "loading"
		}

		// 尝试获取当前 URL（best effort，避免阻塞）
		url, title := "", ""
		if inst.DebugReady && inst.DebugPort > 0 && a.validateProfileCDPOwnership(&inst) == nil {
			url, title = cdpFetchURL(inst.DebugPort)
		}

		sw := SyncWindow{
			ProfileID:   inst.ProfileId,
			ProfileName: inst.ProfileName,
			URL:         url,
			Title:       title,
			DebugPort:   inst.DebugPort,
			Pid:         inst.Pid,
			Status:      status,
			GroupID:     inst.GroupId,
		}
		groupMap[key] = append(groupMap[key], sw)
	}

	result := make([]SyncGroup, 0, len(groupOrder))
	for _, key := range groupOrder {
		windows := groupMap[key]
		result = append(result, SyncGroup{
			ID:      key.id,
			Name:    key.name,
			Windows: windows,
		})
	}

	return result, nil
}

// SynchronizerBroadcastNavigate 向分组内所有窗口广播导航指令。
func (a *App) SynchronizerBroadcastNavigate(groupID, url string) error {
	instances := a.GetRunningInstances()
	targetURL, err := normalizeWorkbenchURL(url)
	if err != nil {
		return err
	}

	targetGroup := groupID
	if targetGroup == "__ungrouped__" {
		targetGroup = ""
	}

	successCount := 0
	failCount := 0

	for _, inst := range instances {
		// 匹配分组
		if inst.GroupId != targetGroup {
			continue
		}
		if inst.DebugPort == 0 || !inst.DebugReady {
			recordSyncOp("navigate", targetGroup, map[string]interface{}{"url": url, "profileId": inst.ProfileId}, "failed", "debug port not ready")
			failCount++
			continue
		}
		if err := a.validateProfileCDPOwnership(&inst); err != nil {
			recordSyncOp("navigate", targetGroup, map[string]interface{}{"url": targetURL, "profileId": inst.ProfileId}, "failed", err.Error())
			failCount++
			continue
		}

		if err := cdpNavigate(inst.DebugPort, targetURL); err != nil {
			recordSyncOp("navigate", targetGroup, map[string]interface{}{"url": targetURL, "profileId": inst.ProfileId}, "failed", err.Error())
			failCount++
			continue
		}
		successCount++
	}

	if successCount == 0 && failCount == 0 {
		return fmt.Errorf("分组 %s 中没有运行中的实例", groupID)
	}

	recordSyncOp("navigate", targetGroup, map[string]interface{}{"url": targetURL, "success": successCount, "failed": failCount}, "success", "")
	return nil
}

// SynchronizerNavigateProfile navigates a single running instance.
func (a *App) SynchronizerNavigateProfile(profileID, rawURL string) error {
	profile, err := a.runningProfileForWorkbench(profileID)
	if err != nil {
		recordSyncOp("navigate", profileID, map[string]interface{}{"profileId": profileID, "url": rawURL}, "failed", err.Error())
		return err
	}
	targetURL, err := normalizeWorkbenchURL(rawURL)
	if err != nil {
		recordSyncOp("navigate", profileID, map[string]interface{}{"profileId": profileID, "url": rawURL}, "failed", err.Error())
		return err
	}
	if err := cdpNavigate(profile.DebugPort, targetURL); err != nil {
		recordSyncOp("navigate", profileID, map[string]interface{}{"profileId": profileID, "url": targetURL}, "failed", err.Error())
		return err
	}
	recordSyncOp("navigate", profileID, map[string]interface{}{"profileId": profileID, "url": targetURL}, "success", "")
	return nil
}

// SynchronizerRefreshProfile refreshes a single running instance.
func (a *App) SynchronizerRefreshProfile(profileID string) error {
	profile, err := a.runningProfileForWorkbench(profileID)
	if err != nil {
		recordSyncOp("refresh", profileID, map[string]interface{}{"profileId": profileID}, "failed", err.Error())
		return err
	}
	if err := cdpReload(profile.DebugPort); err != nil {
		recordSyncOp("refresh", profileID, map[string]interface{}{"profileId": profileID}, "failed", err.Error())
		return err
	}
	recordSyncOp("refresh", profileID, map[string]interface{}{"profileId": profileID}, "success", "")
	return nil
}

// SynchronizerCaptureScreenshot returns a data URL preview for a single running instance.
func (a *App) SynchronizerCaptureScreenshot(profileID string) (string, error) {
	profile, err := a.runningProfileForWorkbench(profileID)
	if err != nil {
		recordSyncOp("screenshot", profileID, map[string]interface{}{"profileId": profileID}, "failed", err.Error())
		return "", err
	}
	dataURL, err := cdpCaptureScreenshot(profile.DebugPort)
	if err != nil {
		recordSyncOp("screenshot", profileID, map[string]interface{}{"profileId": profileID}, "failed", err.Error())
		return "", err
	}
	recordSyncOp("screenshot", profileID, map[string]interface{}{"profileId": profileID}, "success", "")
	return dataURL, nil
}

// SynchronizerActivateProfile brings a real external browser top-level window to the foreground.
func (a *App) SynchronizerActivateProfile(profileID string) error {
	profile, err := a.runningProfileForWindowAction(profileID)
	if err != nil {
		recordSyncOp("activate", profileID, map[string]interface{}{"profileId": profileID}, "failed", err.Error())
		return err
	}
	if err := activateExternalWindowByPID(profile.Pid); err != nil {
		recordSyncOp("activate", profileID, map[string]interface{}{"profileId": profileID, "pid": profile.Pid}, "failed", err.Error())
		return err
	}
	recordSyncOp("activate", profileID, map[string]interface{}{"profileId": profileID, "pid": profile.Pid}, "success", "")
	return nil
}

// SynchronizerArrangeProfiles moves selected real browser windows without re-parenting or embedding them.
func (a *App) SynchronizerArrangeProfiles(profileIDs []string, layout string) ([]SyncWindowPlacement, error) {
	ids := uniqueProfileIDs(profileIDs)
	if len(ids) == 0 {
		return nil, fmt.Errorf("请选择至少一个运行中实例")
	}

	profiles := make([]*BrowserProfile, 0, len(ids))
	placements := make([]SyncWindowPlacement, 0, len(ids))
	for _, id := range ids {
		profile, err := a.runningProfileForWindowAction(id)
		if err != nil {
			placements = append(placements, SyncWindowPlacement{
				ProfileID: id,
				Found:     false,
				Error:     err.Error(),
			})
			continue
		}
		profiles = append(profiles, profile)
	}
	if len(profiles) == 0 {
		recordSyncOp("arrange", "window-layout", map[string]interface{}{"layout": layout, "failed": len(placements)}, "failed", "没有可排列的运行中实例")
		return placements, fmt.Errorf("没有可排列的运行中实例")
	}

	workArea, err := externalWindowWorkArea()
	if err != nil {
		recordSyncOp("arrange", "window-layout", map[string]interface{}{"layout": layout}, "failed", err.Error())
		return placements, err
	}

	rects := workbenchLayoutRects(workArea, len(profiles), layout)
	successCount := 0
	for idx, profile := range profiles {
		rect := rects[idx]
		placement := SyncWindowPlacement{
			ProfileID:   profile.ProfileId,
			ProfileName: profile.ProfileName,
			Pid:         profile.Pid,
			X:           rect.x,
			Y:           rect.y,
			Width:       rect.width,
			Height:      rect.height,
		}
		found, err := moveExternalWindowByPID(profile.Pid, rect)
		placement.Found = found
		if err != nil {
			placement.Error = err.Error()
		} else {
			successCount++
		}
		placements = append(placements, placement)
	}

	status := "success"
	errMsg := ""
	if successCount == 0 {
		status = "failed"
		errMsg = "未找到可移动的浏览器窗口"
	}
	recordSyncOp("arrange", "window-layout", map[string]interface{}{
		"layout":  normalizedWorkbenchLayout(layout),
		"total":   len(profiles),
		"success": successCount,
		"failed":  len(profiles) - successCount,
	}, status, errMsg)

	if successCount == 0 {
		return placements, errors.New(errMsg)
	}
	return placements, nil
}

// SynchronizerBroadcastRefresh 刷新分组内所有窗口。
func (a *App) SynchronizerBroadcastRefresh(groupID string) error {
	instances := a.GetRunningInstances()

	targetGroup := groupID
	if targetGroup == "__ungrouped__" {
		targetGroup = ""
	}

	successCount := 0
	failCount := 0

	for _, inst := range instances {
		if inst.GroupId != targetGroup {
			continue
		}
		if inst.DebugPort == 0 || !inst.DebugReady {
			recordSyncOp("refresh", targetGroup, map[string]interface{}{"profileId": inst.ProfileId}, "failed", "debug port not ready")
			failCount++
			continue
		}
		if err := a.validateProfileCDPOwnership(&inst); err != nil {
			recordSyncOp("refresh", targetGroup, map[string]interface{}{"profileId": inst.ProfileId}, "failed", err.Error())
			failCount++
			continue
		}

		if err := cdpReload(inst.DebugPort); err != nil {
			recordSyncOp("refresh", targetGroup, map[string]interface{}{"profileId": inst.ProfileId}, "failed", err.Error())
			failCount++
			continue
		}
		successCount++
	}

	if successCount == 0 && failCount == 0 {
		return fmt.Errorf("分组 %s 中没有运行中的实例", groupID)
	}

	recordSyncOp("refresh", targetGroup, map[string]interface{}{"success": successCount, "failed": failCount}, "success", "")
	return nil
}

// SynchronizerGetOperationLog 返回最近的同步操作日志。
func (a *App) SynchronizerGetOperationLog(limit int) ([]SyncOperation, error) {
	syncSt.mu.RLock()
	defer syncSt.mu.RUnlock()

	if limit <= 0 || limit > len(syncSt.operations) {
		limit = len(syncSt.operations)
	}

	start := len(syncSt.operations) - limit
	result := make([]SyncOperation, limit)
	copy(result, syncSt.operations[start:])
	return result, nil
}

// SynchronizerListTasks returns persisted recent workbench tasks.
func (a *App) SynchronizerListTasks(limit int) ([]WorkbenchTask, error) {
	tasks, err := a.readWorkbenchTasks()
	if err != nil {
		return nil, err
	}
	if limit <= 0 || limit > len(tasks) {
		limit = len(tasks)
	}
	result := make([]WorkbenchTask, limit)
	copy(result, tasks[:limit])
	return result, nil
}

// SynchronizerSaveTasks persists recent workbench tasks for app restarts.
func (a *App) SynchronizerSaveTasks(tasks []WorkbenchTask) error {
	normalized := normalizeWorkbenchTasks(tasks)
	if len(normalized) > maxWorkbenchTasks {
		normalized = normalized[:maxWorkbenchTasks]
	}
	data, err := json.MarshalIndent(normalized, "", "  ")
	if err != nil {
		return fmt.Errorf("marshal workbench tasks: %w", err)
	}
	path := a.workbenchTasksPath()
	if err := os.MkdirAll(filepath.Dir(path), 0755); err != nil {
		return fmt.Errorf("create workbench task dir: %w", err)
	}
	if err := os.WriteFile(path, data, 0644); err != nil {
		return fmt.Errorf("write workbench tasks: %w", err)
	}
	return nil
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

func recordSyncOp(opType, groupID string, payload map[string]interface{}, status, errMsg string) {
	syncSt.mu.Lock()
	defer syncSt.mu.Unlock()

	syncSt.opSeq++
	op := SyncOperation{
		ID:        fmt.Sprintf("sync-%d", syncSt.opSeq),
		Type:      opType,
		GroupID:   groupID,
		Payload:   payload,
		Timestamp: time.Now().UTC().Format(time.RFC3339),
		Status:    status,
		Error:     errMsg,
	}

	syncSt.operations = append(syncSt.operations, op)
	if len(syncSt.operations) > maxOps {
		syncSt.operations = syncSt.operations[len(syncSt.operations)-maxOps:]
	}
}

func (a *App) workbenchTasksPath() string {
	if a == nil {
		return filepath.Join("data", "workbench", "tasks.json")
	}
	return a.resolveAppPath(filepath.Join("data", "workbench", "tasks.json"))
}

func (a *App) readWorkbenchTasks() ([]WorkbenchTask, error) {
	path := a.workbenchTasksPath()
	data, err := os.ReadFile(path)
	if err != nil {
		if os.IsNotExist(err) {
			return []WorkbenchTask{}, nil
		}
		return nil, fmt.Errorf("read workbench tasks: %w", err)
	}
	var tasks []WorkbenchTask
	if err := json.Unmarshal(data, &tasks); err != nil {
		return nil, fmt.Errorf("parse workbench tasks: %w", err)
	}
	return normalizeWorkbenchTasks(tasks), nil
}

func normalizeWorkbenchTasks(tasks []WorkbenchTask) []WorkbenchTask {
	result := make([]WorkbenchTask, 0, len(tasks))
	for _, task := range tasks {
		task.ID = strings.TrimSpace(task.ID)
		task.Type = strings.TrimSpace(task.Type)
		task.ProfileID = strings.TrimSpace(task.ProfileID)
		task.ProfileName = strings.TrimSpace(task.ProfileName)
		task.Detail = strings.TrimSpace(task.Detail)
		task.Status = strings.TrimSpace(task.Status)
		task.CreatedAt = strings.TrimSpace(task.CreatedAt)
		task.UpdatedAt = strings.TrimSpace(task.UpdatedAt)
		task.Error = strings.TrimSpace(task.Error)
		if task.ID == "" || task.Type == "" || task.ProfileID == "" {
			continue
		}
		if task.Status == "" {
			task.Status = "pending"
		}
		if task.CreatedAt == "" {
			task.CreatedAt = time.Now().UTC().Format(time.RFC3339)
		}
		if task.UpdatedAt == "" {
			task.UpdatedAt = task.CreatedAt
		}
		result = append(result, task)
	}
	return result
}

func uniqueProfileIDs(profileIDs []string) []string {
	seen := make(map[string]struct{}, len(profileIDs))
	result := make([]string, 0, len(profileIDs))
	for _, id := range profileIDs {
		id = strings.TrimSpace(id)
		if id == "" {
			continue
		}
		if _, ok := seen[id]; ok {
			continue
		}
		seen[id] = struct{}{}
		result = append(result, id)
	}
	return result
}

func normalizedWorkbenchLayout(layout string) string {
	switch strings.ToLower(strings.TrimSpace(layout)) {
	case "main-left", "focus":
		return "main-left"
	default:
		return "grid"
	}
}

func workbenchLayoutRects(workArea workbenchWindowRect, count int, layout string) []workbenchWindowRect {
	if count <= 0 {
		return nil
	}
	if count == 1 {
		return []workbenchWindowRect{workArea}
	}

	gap := 8
	positiveSize := func(value int) int {
		if value < 1 {
			return 1
		}
		return value
	}

	if normalizedWorkbenchLayout(layout) == "main-left" {
		mainWidth := positiveSize((workArea.width * 62) / 100)
		sideWidth := positiveSize(workArea.width - mainWidth - gap)
		sideCount := count - 1
		sideHeight := positiveSize((workArea.height - gap*(sideCount-1)) / sideCount)
		rects := []workbenchWindowRect{{
			x:      workArea.x,
			y:      workArea.y,
			width:  mainWidth,
			height: workArea.height,
		}}
		for i := 0; i < sideCount; i++ {
			rects = append(rects, workbenchWindowRect{
				x:      workArea.x + mainWidth + gap,
				y:      workArea.y + i*(sideHeight+gap),
				width:  sideWidth,
				height: sideHeight,
			})
		}
		return rects
	}

	cols := 1
	for cols*cols < count {
		cols++
	}
	rows := (count + cols - 1) / cols
	cellWidth := positiveSize((workArea.width - gap*(cols-1)) / cols)
	cellHeight := positiveSize((workArea.height - gap*(rows-1)) / rows)
	rects := make([]workbenchWindowRect, 0, count)
	for i := 0; i < count; i++ {
		row := i / cols
		col := i % cols
		rects = append(rects, workbenchWindowRect{
			x:      workArea.x + col*(cellWidth+gap),
			y:      workArea.y + row*(cellHeight+gap),
			width:  cellWidth,
			height: cellHeight,
		})
	}
	return rects
}
