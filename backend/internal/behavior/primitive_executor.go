package behavior

import (
	"encoding/json"
	"fmt"
	"strings"
	"time"

	"personal-pilot/backend/internal/behavior/humanize"
)

// PrimitiveStep is one executable behavior primitive invocation.
type PrimitiveStep struct {
	Primitive       string `json:"primitive"`
	Selector        string `json:"selector,omitempty"`
	Target          string `json:"target,omitempty"`
	Text            string `json:"text,omitempty"`
	URL             string `json:"url,omitempty"`
	DurationMs      int64  `json:"duration_ms,omitempty"`
	TimeoutMs       int64  `json:"timeout_ms,omitempty"`
	StableWindowMs  int64  `json:"stable_window_ms,omitempty"`
	Segments        int    `json:"segments,omitempty"`
	Ratio           int    `json:"ratio,omitempty"`
	MaxCorrections  int    `json:"max_corrections,omitempty"`
}

func (s PrimitiveStep) timeout(defaultMs int64) time.Duration {
	if s.TimeoutMs > 0 {
		return time.Duration(s.TimeoutMs) * time.Millisecond
	}
	if defaultMs <= 0 {
		defaultMs = 15000
	}
	return time.Duration(defaultMs) * time.Millisecond
}

func (s PrimitiveStep) selector() string {
	if strings.TrimSpace(s.Selector) != "" {
		return strings.TrimSpace(s.Selector)
	}
	return targetToSelector(strings.TrimSpace(s.Target))
}

func targetToSelector(target string) string {
	switch target {
	case "primary-form-field":
		return "input:not([type=hidden]),textarea,select"
	case "ranked-visible-item":
		return "a[href],button,[role=button]"
	case "primary-media":
		return "img,video,canvas"
	case "interactive-module":
		return "[role=button],a,button,input"
	default:
		if target == "" {
			return "body"
		}
		return target
	}
}

// ExecutePrimitive runs one shipped primitive on the connected page.
func (e *CDPExecutor) ExecutePrimitive(step PrimitiveStep) (map[string]interface{}, error) {
	if e == nil {
		return nil, fmt.Errorf("cdp executor is nil")
	}
	name := strings.TrimSpace(step.Primitive)
	if !IsShippedPrimitive(name) {
		return nil, fmt.Errorf("unsupported primitive: %s", name)
	}
	result := map[string]interface{}{"primitive": name}
	switch name {
	case "idle", "pause_on_content":
		d := step.DurationMs
		if d <= 0 {
			d = 500
		}
		humanize.NaturalDelay(humanize.DefaultBioNoiseConfig())
		return result, e.Wait(time.Duration(d) * time.Millisecond)
	case "wait_for_readiness":
		if err := e.WaitForSelector("body", step.timeout(20000)); err != nil {
			return result, err
		}
		deadline := time.Now().Add(step.timeout(20000))
		for time.Now().Before(deadline) {
			state, err := e.EvaluateJS(`document.readyState`)
			if err == nil && (state == "interactive" || state == "complete") {
				return result, nil
			}
			time.Sleep(150 * time.Millisecond)
		}
		return result, fmt.Errorf("timeout waiting for document readiness")
	case "wait_for_content_stable":
		windowMs := step.StableWindowMs
		if windowMs <= 0 {
			windowMs = 800
		}
		stableSince := time.Time{}
		deadline := time.Now().Add(step.timeout(20000))
		for time.Now().Before(deadline) {
			hash, err := e.EvaluateJS(`(function(){ return document.body ? document.body.innerText.length + '|' + document.querySelectorAll('*').length : '0|0'; })()`)
			if err == nil {
				if stableSince.IsZero() {
					stableSince = time.Now()
				} else if time.Since(stableSince) >= time.Duration(windowMs)*time.Millisecond {
					result["contentFingerprint"] = hash
					return result, nil
				}
			} else {
				stableSince = time.Time{}
			}
			time.Sleep(200 * time.Millisecond)
		}
		return result, fmt.Errorf("content did not stabilize")
	case "wait_for_selector":
		return result, e.WaitForSelectorVisible(step.selector(), step.timeout(15000))
	case "wait_for_navigation":
		startURL, _ := e.GetPageURL()
		deadline := time.Now().Add(step.timeout(30000))
		for time.Now().Before(deadline) {
			url, _ := e.GetPageURL()
			state, _ := e.EvaluateJS(`document.readyState`)
			if url != startURL && (state == "interactive" || state == "complete") {
				result["url"] = url
				return result, nil
			}
			time.Sleep(250 * time.Millisecond)
		}
		return result, fmt.Errorf("navigation did not complete")
	case "scroll_progressive":
		segments := step.Segments
		if segments <= 0 {
			segments = 3
		}
		for i := 0; i < segments; i++ {
			if err := e.ExecuteHumanizedScroll(uint32(120 + i*80)); err != nil {
				return result, err
			}
			time.Sleep(time.Duration(250+i*120) * time.Millisecond)
		}
		return result, nil
	case "scroll_to_ratio":
		ratio := step.Ratio
		if ratio <= 0 {
			ratio = 50
		}
		js := fmt.Sprintf(`window.scrollTo({top: Math.max(0, (document.body.scrollHeight * %d) / 100), behavior: 'auto'})`, ratio)
		return result, e.evaluateRaw(js)
	case "scroll_into_view":
		sel := step.selector()
		js := fmt.Sprintf(`(function(){ var el=document.querySelector(%q); if(!el) return false; el.scrollIntoView({block:'center'}); return true; })()`, sel)
		val, err := e.EvaluateJS(js)
		if err != nil {
			return result, err
		}
		if val != "true" {
			return result, fmt.Errorf("element not found: %s", sel)
		}
		return result, nil
	case "focus_element":
		sel := step.selector()
		js := fmt.Sprintf(`(function(){ var el=document.querySelector(%q); if(!el) return false; el.focus(); return true; })()`, sel)
		if val, err := e.EvaluateJS(js); err != nil || val != "true" {
			return result, fmt.Errorf("focus failed: %s", sel)
		}
		return result, nil
	case "blur_element":
		sel := step.selector()
		js := fmt.Sprintf(`(function(){ var el=document.querySelector(%q); if(!el) return false; el.blur(); return true; })()`, sel)
		if val, err := e.EvaluateJS(js); err != nil || val != "true" {
			return result, fmt.Errorf("blur failed: %s", sel)
		}
		return result, nil
	case "hover_candidate":
		return result, e.HoverElement(step.selector())
	case "click_element":
		return result, e.ExecuteHumanizedClick(step.selector())
	case "double_click_element":
		return result, e.DoubleClickElement(step.selector())
	case "right_click_element":
		return result, e.RightClickElement(step.selector())
	case "type_with_rhythm":
		text := step.Text
		if text == "" {
			text = "sample"
		}
		return result, e.ExecuteHumanizedTypeEx(step.selector(), text, true, false)
	case "clear_with_corrections":
		max := step.MaxCorrections
		if max <= 0 {
			max = 2
		}
		for i := 0; i < max; i++ {
			_ = e.ExecuteHumanizedTypeEx(step.selector(), "", true, false)
			time.Sleep(time.Duration(120+i*80) * time.Millisecond)
		}
		return result, nil
	case "fill_form_field":
		return result, e.ExecuteHumanizedTypeEx(step.selector(), step.Text, true, false)
	case "press_key", "press_key_combo":
		key := step.Text
		if key == "" {
			key = "Enter"
		}
		return result, e.dispatchKeyEvent("keyDown", key)
	case "open_url":
		url := strings.TrimSpace(step.URL)
		if url == "" {
			return result, fmt.Errorf("open_url requires url")
		}
		return result, e.Navigate(url)
	case "get_page_html":
		html, err := e.GetPageHTML(step.selector(), nil)
		if err != nil {
			return result, err
		}
		result["length"] = len(html)
		return result, nil
	case "get_element_text":
		text, err := e.GetElementText(step.selector())
		if err != nil {
			return result, err
		}
		result["text"] = text
		return result, nil
	case "dom_snapshot":
		snap, err := e.CaptureDOMSnapshot()
		if err != nil {
			return result, err
		}
		result["length"] = len(snap)
		return result, nil
	case "capture_screenshot":
		shot, err := e.CaptureScreenshot()
		if err != nil {
			return result, err
		}
		result["length"] = len(shot)
		return result, nil
	case "evaluate_script":
		out, err := e.EvaluateJS(step.Text)
		if err != nil {
			return result, err
		}
		result["output"] = out
		return result, nil
	case "simulate_natural_browsing":
		d := step.DurationMs
		if d <= 0 {
			d = 4000
		}
		return result, e.SimulateNaturalBrowsing(time.Duration(d) * time.Millisecond)
	case "show_mouse_pointer":
		return result, e.ShowMousePointerOverlay()
	case "hide_mouse_pointer":
		return result, e.HideMousePointerOverlay()
	case "persist_session_state", "soft_abort_if_budget_exceeded":
		return result, nil
	default:
		return result, fmt.Errorf("primitive not wired: %s", name)
	}
}

// ExecutePrimitivePlan runs steps sequentially.
func (e *CDPExecutor) ExecutePrimitivePlan(steps []PrimitiveStep) ([]map[string]interface{}, error) {
	out := make([]map[string]interface{}, 0, len(steps))
	for i, step := range steps {
		res, err := e.ExecutePrimitive(step)
		if err != nil {
			return out, fmt.Errorf("step %d (%s): %w", i, step.Primitive, err)
		}
		out = append(out, res)
	}
	return out, nil
}

// ParsePrimitivePlanJSON decodes a JSON array of primitive steps.
func ParsePrimitivePlanJSON(raw string) ([]PrimitiveStep, error) {
	raw = strings.TrimSpace(raw)
	if raw == "" {
		return nil, fmt.Errorf("empty primitive plan")
	}
	var steps []PrimitiveStep
	if err := json.Unmarshal([]byte(raw), &steps); err != nil {
		return nil, err
	}
	for _, step := range steps {
		if !IsShippedPrimitive(step.Primitive) {
			return nil, fmt.Errorf("unsupported primitive in plan: %s", step.Primitive)
		}
	}
	return steps, nil
}
