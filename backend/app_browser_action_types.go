package backend

import "encoding/json"

const (
	BrowserActionNavigate = "navigate"
	BrowserActionClick    = "click"
	BrowserActionType     = "type"
	BrowserActionScroll   = "scroll"
	BrowserActionEval     = "evaluate"
	BrowserActionWait     = "wait"
	BrowserActionMove     = "move"
)

type BrowserAction struct {
	Type     string           `json:"type"`
	Selector string           `json:"selector,omitempty"`
	Value    string           `json:"value,omitempty"`
	URL      string           `json:"url,omitempty"`
	Script   string           `json:"script,omitempty"`
	X        *float64         `json:"x,omitempty"`
	Y        *float64         `json:"y,omitempty"`
	DeltaX   int              `json:"deltaX,omitempty"`
	DeltaY   int              `json:"deltaY,omitempty"`
	WaitMs   int              `json:"waitMs,omitempty"`
	Method   string           `json:"method,omitempty"`
	Params   json.RawMessage  `json:"params,omitempty"`
}

type BrowserActionResult struct {
	Success bool        `json:"success"`
	Action  string      `json:"action"`
	Data    interface{} `json:"data,omitempty"`
}
