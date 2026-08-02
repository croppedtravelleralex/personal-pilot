package backend

import (
	"context"
	"encoding/json"
	"fmt"
	"strings"
	"time"

	"personal-pilot/backend/internal/behavior"
	"personal-pilot/backend/internal/behavior/humanize"
)

type browserActionExecutor interface {
	Navigate(string) error
	ExecuteHumanizedClick(string) error
	ExecuteHumanizedType(string, string) error
	EvaluateRaw(string) ([]byte, error)
	EvaluateJS(string) (string, error)
	MoveMouseTo(float64, float64) error
	Click() error
}

func (a *App) BrowserInstanceExecAction(profileId string, action BrowserAction) (BrowserActionResult, error) {
	profile, err := a.resolveRunningProfile(profileId)
	if err != nil {
		return BrowserActionResult{Success: false, Action: action.Type}, err
	}

	if strings.TrimSpace(action.Type) == "" {
		return BrowserActionResult{Success: false, Action: action.Type}, fmt.Errorf("action type is required")
	}

	ctx, cancel := context.WithTimeout(context.Background(), 15*time.Second)
	defer cancel()

	ws, err := behavior.ConnectPageCDP(profile.DebugPort)
	if err != nil {
		return BrowserActionResult{Success: false, Action: action.Type}, fmt.Errorf("connect page CDP: %w", err)
	}
	defer ws.Close()

	executor := behavior.NewCDPExecutor(ws, humanize.DefaultConfig())
	if executor == nil {
		return BrowserActionResult{Success: false, Action: action.Type}, fmt.Errorf("create CDP executor")
	}
	defer executor.Close()

	result, err := executeBrowserAction(ctx, executor, action)
	if err != nil {
		return BrowserActionResult{Success: false, Action: action.Type}, err
	}
	return BrowserActionResult{Success: true, Action: action.Type, Data: result}, nil
}

func (a *App) resolveRunningProfile(profileId string) (*BrowserProfile, error) {
	if a.browserMgr == nil {
		return nil, fmt.Errorf("profile not found")
	}

	a.browserMgr.Mutex.Lock()
	defer a.browserMgr.Mutex.Unlock()

	profile, exists := a.browserMgr.Profiles[profileId]
	if !exists {
		return nil, fmt.Errorf("profile not found")
	}
	if !profile.Running || profile.DebugPort <= 0 {
		return nil, fmt.Errorf("profile is not running")
	}
	return profile, nil
}

func executeBrowserAction(ctx context.Context, executor browserActionExecutor, action BrowserAction) (interface{}, error) {
	switch strings.ToLower(strings.TrimSpace(action.Type)) {
	case BrowserActionNavigate:
		targetURL := strings.TrimSpace(action.URL)
		if targetURL == "" {
			targetURL = strings.TrimSpace(action.Value)
		}
		if targetURL == "" {
			return nil, fmt.Errorf("navigate action requires url")
		}
		return nil, executor.Navigate(targetURL)

	case BrowserActionClick:
		if strings.TrimSpace(action.Selector) != "" {
			return nil, executor.ExecuteHumanizedClick(action.Selector)
		}
		if action.X == nil || action.Y == nil {
			return nil, fmt.Errorf("click action requires selector or coordinates")
		}
		if err := executor.MoveMouseTo(*action.X, *action.Y); err != nil {
			return nil, err
		}
		return nil, executor.Click()

	case BrowserActionType:
		if strings.TrimSpace(action.Selector) == "" {
			return nil, fmt.Errorf("type action requires selector")
		}
		return nil, executor.ExecuteHumanizedType(action.Selector, action.Value)

	case BrowserActionScroll:
		if action.DeltaY == 0 && action.DeltaX == 0 {
			return nil, fmt.Errorf("scroll action requires deltaX or deltaY")
		}
		js := fmt.Sprintf("window.scrollBy(%d, %d)", action.DeltaX, action.DeltaY)
		raw, err := executor.EvaluateRaw(js)
		if err != nil {
			return nil, err
		}
		return json.RawMessage(raw), nil

	case BrowserActionEval:
		if strings.TrimSpace(action.Script) == "" {
			return nil, fmt.Errorf("evaluate action requires script")
		}
		return executor.EvaluateJS(action.Script)

	case BrowserActionWait:
		if action.WaitMs <= 0 {
			return nil, fmt.Errorf("wait action requires waitMs")
		}
		select {
		case <-ctx.Done():
			return nil, ctx.Err()
		case <-time.After(time.Duration(action.WaitMs) * time.Millisecond):
			return nil, nil
		}

	case BrowserActionMove:
		if action.X == nil || action.Y == nil {
			return nil, fmt.Errorf("move action requires coordinates")
		}
		return nil, executor.MoveMouseTo(*action.X, *action.Y)

	default:
		return nil, fmt.Errorf("unsupported action type: %s", action.Type)
	}
}
