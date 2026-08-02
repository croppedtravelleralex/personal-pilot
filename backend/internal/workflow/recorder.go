package workflow

import (
	"fmt"
	"strings"
	"time"
)

type CDPEvent struct {
	Method    string
	Selector  string
	Text      string
	URL       string
	Timestamp time.Time
}

func StepsFromCDPEvents(events []CDPEvent) []Step {
	steps := make([]Step, 0, len(events))
	for i, event := range events {
		actionType := classifyCDPEvent(event)
		if actionType == "" {
			continue
		}
		params := map[string]any{}
		if event.Selector != "" {
			params["selector"] = event.Selector
		}
		if event.Text != "" {
			params["text"] = event.Text
		}
		if event.URL != "" {
			params["url"] = event.URL
		}
		steps = append(steps, Step{ID: stepID(i), Action: Action{Type: actionType, Params: params}})
	}
	return steps
}

func classifyCDPEvent(event CDPEvent) string {
	method := strings.ToLower(event.Method)
	switch {
	case strings.Contains(method, "navigate") || event.URL != "":
		return "browser:navigate"
	case strings.Contains(method, "click"):
		return "dom:click"
	case strings.Contains(method, "input") || strings.Contains(method, "type") || event.Text != "":
		return "dom:type"
	case strings.Contains(method, "scroll"):
		return "dom:scroll"
	default:
		return ""
	}
}

func stepID(i int) string {
	return fmt.Sprintf("step-%03d", i+1)
}
