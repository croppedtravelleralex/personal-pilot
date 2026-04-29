//go:build wails_embedded_experiment
// +build wails_embedded_experiment

package backend

import (
	"encoding/json"
	"fmt"
	"net/http"
	"time"

	"ant-chrome/backend/internal/events"
	"ant-chrome/backend/internal/logger"

	"github.com/gorilla/websocket"
)

var embedLog = logger.New("ChromeEmbed")

// EmbeddedBrowserStart starts Chrome in headless mode and begins screencast.
func (a *App) EmbeddedBrowserStart(profileId string) (*BrowserProfile, error) {
	profile, err := a.BrowserInstanceStartWithParams(
		profileId,
		[]string{"--headless", "--window-size=1920,1080"},
		[]string{"about:blank"}, // single target only (headless doesn't support multiple)
		true,                    // skip default verification URLs
	)
	if err != nil {
		return nil, err
	}

	// Start screencast in background
	go a.startScreencast(profileId, profile.DebugPort)

	return profile, nil
}

// EmbeddedBrowserStop stops screencast and the browser instance.
func (a *App) EmbeddedBrowserStop(profileId string) error {
	a.stopScreencast(profileId)
	_, err := a.BrowserInstanceStop(profileId)
	return err
}

// EmbeddedBrowserFocus switches the active tab (frontend handles canvas switching).
func (a *App) EmbeddedBrowserFocus(profileId string) {
	// Frontend handles tab switching - just emit focus event
	a.emit(events.EventEmbeddedFocus, map[string]interface{}{"profileId": profileId})
}

// EmbeddedBrowserResize updates the screencast viewport size.
func (a *App) EmbeddedBrowserResize(containerW, containerH int) {
	// Frontend handles canvas scaling via CSS
}

// screencastSession holds the CDP connection for one screencast instance.
var screencastSessions = make(map[string]*screencastSession)

type screencastSession struct {
	ws     *websocket.Conn
	cancel chan struct{}
}

func (a *App) startScreencast(profileId string, debugPort int) {
	// Connect to Chrome CDP page target
	ws, err := connectCDPScreencast(debugPort)
	if err != nil {
		embedLog.Error("screencast杩炴帴澶辫触", logger.F("profileId", profileId), logger.F("error", err))
		return
	}

	session := &screencastSession{ws: ws, cancel: make(chan struct{})}
	screencastSessions[profileId] = session

	// Start Page.startScreencast
	if err := ws.WriteJSON(map[string]interface{}{
		"id":     1,
		"method": "Page.startScreencast",
		"params": map[string]interface{}{
			"format":        "jpeg",
			"quality":       50,
			"maxWidth":      1280,
			"maxHeight":     720,
			"everyNthFrame": 1,
		},
	}); err != nil {
		embedLog.Error("鍚姩screencast澶辫触", logger.F("profileId", profileId), logger.F("error", err))
		return
	}

	embedLog.Info("screencast started", logger.F("profileId", profileId))

	// Read frames in loop
	for {
		select {
		case <-session.cancel:
			ws.WriteJSON(map[string]interface{}{
				"id": 999, "method": "Page.stopScreencast",
			})
			ws.Close()
			return
		default:
		}

		ws.SetReadDeadline(time.Now().Add(3 * time.Second))
		_, msg, err := ws.ReadMessage()
		if err != nil {
			select {
			case <-session.cancel:
				return
			default:
				embedLog.Info("screencast杩炴帴鏂紑", logger.F("profileId", profileId))
				return
			}
		}

		var evt struct {
			Method string `json:"method"`
			Params struct {
				Data      string `json:"data"`
				SessionID int    `json:"sessionId"`
			} `json:"params"`
		}
		if err := json.Unmarshal(msg, &evt); err != nil {
			continue
		}

		if evt.Method == "Page.screencastFrame" && evt.Params.Data != "" {
			if a.ctx != nil {
				a.emit(events.EventEmbeddedFrame+":"+profileId, evt.Params.Data)
			}
			// Acknowledge
			ws.WriteJSON(map[string]interface{}{
				"id":     0,
				"method": "Page.screencastFrameAck",
				"params": map[string]interface{}{
					"sessionId": evt.Params.SessionID,
				},
			})
		}
	}
}

func (a *App) stopScreencast(profileId string) {
	if s, ok := screencastSessions[profileId]; ok {
		close(s.cancel)
		delete(screencastSessions, profileId)
	}
}

func connectCDPScreencast(debugPort int) (*websocket.Conn, error) {
	resp, err := http.Get(fmt.Sprintf("http://127.0.0.1:%d/json", debugPort))
	if err != nil {
		return nil, fmt.Errorf("get CDP targets: %w", err)
	}
	defer resp.Body.Close()

	var pages []struct {
		WebSocketDebuggerURL string `json:"webSocketDebuggerUrl"`
		Type                 string `json:"type"`
	}
	if err := json.NewDecoder(resp.Body).Decode(&pages); err != nil {
		return nil, fmt.Errorf("decode CDP: %w", err)
	}

	for _, p := range pages {
		if p.Type == "page" && p.WebSocketDebuggerURL != "" {
			ws, _, err := websocket.DefaultDialer.Dial(p.WebSocketDebuggerURL, nil)
			if err != nil {
				return nil, fmt.Errorf("dial: %w", err)
			}
			return ws, nil
		}
	}
	return nil, fmt.Errorf("no page target on port %d", debugPort)
}
