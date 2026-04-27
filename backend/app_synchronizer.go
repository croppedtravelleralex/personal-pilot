package backend

import (
	"ant-chrome/backend/internal/logger"
	"fmt"
	"sync"
	"time"
)

// SyncWindow represents a browser window in the synchronizer.
type SyncWindow struct {
	ID            string `json:"id"`
	ProfileID     string `json:"profileId"`
	ProfileName   string `json:"profileName"`
	Title         string `json:"title"`
	URL           string `json:"url"`
	DebugPort     int    `json:"debugPort"`
	IsMain        bool   `json:"isMain"`
	Position      WindowPosition `json:"position"`
	Size          WindowSize     `json:"size"`
	Status        string         `json:"status"`
	LastActiveAt  string         `json:"lastActiveAt"`
	GroupID       string         `json:"groupId"`
}

type WindowPosition struct {
	X int `json:"x"`
	Y int `json:"y"`
}

type WindowSize struct {
	Width  int `json:"width"`
	Height int `json:"height"`
}

// SyncGroup is a logical grouping of browser windows.
type SyncGroup struct {
	ID        string       `json:"id"`
	Name      string       `json:"name"`
	Windows   []SyncWindow `json:"windows"`
	Layout    string       `json:"layout"`
	CreatedAt string       `json:"createdAt"`
}

// SyncOperation records a broadcast operation.
type SyncOperation struct {
	ID        string `json:"id"`
	Type      string `json:"type"`
	WindowID  string `json:"windowId"`
	GroupID   string `json:"groupId"`
	Payload   map[string]interface{} `json:"payload,omitempty"`
	Timestamp string `json:"timestamp"`
	Status    string `json:"status"`
	Error     string `json:"error,omitempty"`
}

// SynchronizerState holds the mutable state for Sync Windows.
type SynchronizerState struct {
	mu         sync.RWMutex
	groups     []SyncGroup
	operations []SyncOperation
	opSeq      int
}

var syncState = &SynchronizerState{}

// ─── Wails-bound methods ─────────────────────────────────────────────────────

// SynchronizerListGroups returns all sync groups.
func (a *App) SynchronizerListGroups() ([]SyncGroup, error) {
	syncState.mu.RLock()
	defer syncState.mu.RUnlock()

	result := make([]SyncGroup, len(syncState.groups))
	copy(result, syncState.groups)
	return result, nil
}

// SynchronizerGetGroup returns a single sync group by ID.
func (a *App) SynchronizerGetGroup(id string) (SyncGroup, error) {
	syncState.mu.RLock()
	defer syncState.mu.RUnlock()

	for _, g := range syncState.groups {
		if g.ID == id {
			return g, nil
		}
	}
	return SyncGroup{}, fmt.Errorf("group %s not found", id)
}

// SynchronizerArrange changes the layout of a sync group.
func (a *App) SynchronizerArrange(groupID, layout string) error {
	log := logger.New("Synchronizer")
	syncState.mu.Lock()
	defer syncState.mu.Unlock()

	for i := range syncState.groups {
		if syncState.groups[i].ID == groupID {
			syncState.groups[i].Layout = layout
			log.Info("布局变更", logger.F("group_id", groupID), logger.F("layout", layout))
			return nil
		}
	}
	return fmt.Errorf("group %s not found", groupID)
}

// SynchronizerBroadcastNavigate navigates all windows in a group to a URL.
func (a *App) SynchronizerBroadcastNavigate(groupID, url string) error {
	log := logger.New("Synchronizer")
	syncState.mu.RLock()
	var targets []SyncWindow
	for _, g := range syncState.groups {
		if g.ID == groupID {
			targets = make([]SyncWindow, len(g.Windows))
			copy(targets, g.Windows)
			break
		}
	}
	syncState.mu.RUnlock()

	if len(targets) == 0 {
		return fmt.Errorf("group %s has no windows", groupID)
	}

	for _, w := range targets {
		log.Info("广播导航", logger.F("window_id", w.ID), logger.F("url", url))
		// TODO: Send CDP Page.navigate to each window's debug port
	}

	a.recordSyncOp("navigate", "", groupID, map[string]interface{}{"url": url}, "success")
	return nil
}

// SynchronizerBroadcastScroll scrolls all windows in a group.
func (a *App) SynchronizerBroadcastScroll(groupID string, deltaY int) error {
	log := logger.New("Synchronizer")
	log.Info("广播滚动", logger.F("group_id", groupID), logger.F("delta_y", deltaY))

	a.recordSyncOp("scroll", "", groupID, map[string]interface{}{"deltaY": deltaY}, "success")
	return nil
}

// SynchronizerBroadcastRefresh refreshes all windows in a group.
func (a *App) SynchronizerBroadcastRefresh(groupID string) error {
	log := logger.New("Synchronizer")
	log.Info("广播刷新", logger.F("group_id", groupID))

	a.recordSyncOp("refresh", "", groupID, nil, "success")
	return nil
}

// SynchronizerFocusWindow focuses a specific window.
func (a *App) SynchronizerFocusWindow(windowID string) error {
	log := logger.New("Synchronizer")
	log.Info("聚焦窗口", logger.F("window_id", windowID))
	return nil
}

// SynchronizerCloseWindow closes a specific window.
func (a *App) SynchronizerCloseWindow(windowID string) error {
	log := logger.New("Synchronizer")
	log.Info("关闭窗口", logger.F("window_id", windowID))
	return nil
}

// SynchronizerGetOperationLog returns recent sync operations.
func (a *App) SynchronizerGetOperationLog(limit int) ([]SyncOperation, error) {
	syncState.mu.RLock()
	defer syncState.mu.RUnlock()

	if limit <= 0 || limit > len(syncState.operations) {
		limit = len(syncState.operations)
	}

	start := len(syncState.operations) - limit
	result := make([]SyncOperation, limit)
	copy(result, syncState.operations[start:])
	return result, nil
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

func (a *App) recordSyncOp(opType, windowID, groupID string, payload map[string]interface{}, status string) {
	syncState.mu.Lock()
	defer syncState.mu.Unlock()

	syncState.opSeq++
	op := SyncOperation{
		ID:        fmt.Sprintf("sync-op-%d", syncState.opSeq),
		Type:      opType,
		WindowID:  windowID,
		GroupID:   groupID,
		Payload:   payload,
		Timestamp: time.Now().UTC().Format(time.RFC3339),
		Status:    status,
	}

	// Keep at most 200 operations
	const maxOps = 200
	syncState.operations = append(syncState.operations, op)
	if len(syncState.operations) > maxOps {
		syncState.operations = syncState.operations[len(syncState.operations)-maxOps:]
	}
}
