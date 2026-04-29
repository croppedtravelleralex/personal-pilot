package llm

import (
	"encoding/json"
	"fmt"

	"ant-chrome/backend/internal/behavior/humanize"
)

// PlannedAction is a parsed LLM-generated action with metadata.
type PlannedAction struct {
	Type        string  `json:"type"`
	URL         string  `json:"url,omitempty"`
	Selector    string  `json:"selector,omitempty"`
	Text        string  `json:"text,omitempty"`
	DurationMs  uint32  `json:"durationMs,omitempty"`
	Direction   string  `json:"direction,omitempty"`
	DistancePx  *uint32 `json:"distancePx,omitempty"`
	Description string  `json:"description"`
}

// OffsetVariantSpec is a parsed LLM-generated offset variant.
type OffsetVariantSpec struct {
	ID             string              `json:"id"`
	Name           string              `json:"name"`
	Description    string              `json:"description"`
	TimingOffset   TimingVariantSpec   `json:"timingOffset"`
	PositionOffset PositionVariantSpec `json:"positionOffset"`
	BehaviorCurve  BehaviorCurveSpec   `json:"behaviorCurve"`
}

type TimingVariantSpec struct {
	PreDelayMs  int     `json:"preDelayMs"`
	PostDelayMs int     `json:"postDelayMs"`
	JitterRatio float64 `json:"jitterRatio"`
}

type PositionVariantSpec struct {
	RadiusPx int    `json:"radiusPx"`
	BiasDir  string `json:"biasDir"`
}

type BehaviorCurveSpec struct {
	HoverMs     int    `json:"hoverMs"`
	DoubleClick bool   `json:"doubleClick"`
	CurveStyle  string `json:"curveStyle"`
}

// OffsetPlan is the full LLM-generated offset variant plan.
type OffsetPlan struct {
	Category string              `json:"category"`
	Variants []OffsetVariantSpec `json:"variants"`
}

// PlanActions converts natural language to a sequence of browser actions.
func (c *Client) PlanActions(taskDescription string) ([]PlannedAction, error) {
	response, err := c.Chat(SystemPromptActionPlanning, taskDescription)
	if err != nil {
		return nil, fmt.Errorf("LLM plan failed: %w", err)
	}

	// Extract JSON from response (may be wrapped in markdown)
	jsonStr := extractJSON(response)

	var actions []PlannedAction
	if err := json.Unmarshal([]byte(jsonStr), &actions); err != nil {
		return nil, fmt.Errorf("parse LLM plan: %w\nRaw response: %s", err, response)
	}

	if len(actions) == 0 {
		return nil, fmt.Errorf("LLM returned empty plan")
	}

	return actions, nil
}

// GenerateOffsets generates offset variants for a specific operation category.
func (c *Client) GenerateOffsets(category string, baseDescription string, count int) (*OffsetPlan, error) {
	prompt := fmt.Sprintf(
		"为基础操作 [%s] 生成 %d 个偏移变体。基础操作描述: %s",
		category, count, baseDescription,
	)
	response, err := c.Chat(SystemPromptOffsetVariants, prompt)
	if err != nil {
		return nil, fmt.Errorf("LLM offset generation failed: %w", err)
	}

	jsonStr := extractJSON(response)
	var plan OffsetPlan
	if err := json.Unmarshal([]byte(jsonStr), &plan); err != nil {
		return nil, fmt.Errorf("parse LLM offsets: %w", err)
	}

	plan.Category = category
	return &plan, nil
}

// ConvertToLlmAction converts a PlannedAction to a humanize.LlmAction.
func (pa *PlannedAction) ConvertToLlmAction() humanize.LlmAction {
	action := humanize.LlmAction{
		URL:        pa.URL,
		Selector:   pa.Selector,
		Text:       pa.Text,
		DurationMs: pa.DurationMs,
	}

	switch pa.Type {
	case "goto":
		action.Type = humanize.ActionGoto
	case "click":
		action.Type = humanize.ActionClick
	case "scroll":
		action.Type = humanize.ActionScroll
		if pa.Direction == "up" {
			action.Direction = humanize.ScrollUp
		} else {
			action.Direction = humanize.ScrollDown
		}
		if pa.DistancePx != nil {
			action.DistancePx = pa.DistancePx
		}
	case "type":
		action.Type = humanize.ActionTypeText
	case "wait":
		action.Type = humanize.ActionWait
	}

	return action
}

// extractJSON attempts to extract JSON content from a response that may be
// wrapped in markdown code fences.
func extractJSON(response string) string {
	// Try to find JSON between ```json and ``` markers
	start := -1
	end := -1

	for i := 0; i < len(response); i++ {
		if response[i] == '`' {
			// Check for ```json
			if i+7 < len(response) && response[i:i+7] == "```json" {
				start = i + 7
				// Skip to end of line after ```json
				for start < len(response) && (response[start] == '\r' || response[start] == '\n') {
					start++
				}
				i = start - 1
				continue
			}
			// Check for ``` at the start (no json tag)
			if i+3 < len(response) && response[i:i+3] == "```" && start == -1 {
				start = i + 3
				for start < len(response) && (response[start] == '\r' || response[start] == '\n') {
					start++
				}
				i = start - 1
				continue
			}
			// Check for closing ```
			if i+3 <= len(response) && response[i:i+3] == "```" && start != -1 {
				end = i
				break
			}
		}
	}

	if start >= 0 && end > start {
		return response[start:end]
	}

	// Try to find JSON array/object directly
	for i, ch := range response {
		if ch == '[' || ch == '{' {
			return response[i:]
		}
	}

	return response
}
