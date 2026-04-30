package backend

import (
	"encoding/json"
	"errors"
	"fmt"
	"math"
	"strings"
	"time"

	"personal-pilot/backend/internal/behavior"
	"personal-pilot/backend/internal/behavior/offsets"
	"personal-pilot/backend/internal/events"
	"personal-pilot/backend/internal/llm"
	"personal-pilot/backend/internal/logger"

	"github.com/gorilla/websocket"
)

var llmLog = logger.New("LLM")

// llmClient 全局 LLM 客户端（在 startup 时初始化）
var llmClient *llm.Client

// InitLLMClient 初始化 LLM 客户端
func (a *App) InitLLMClient() {
	if a.config != nil {
		llmClient = llm.NewClient(a.config.LLM)
		if llmClient.HasKey() {
			llmLog.Info("LLM 客户端已初始化", logger.F("provider", a.config.LLM.Provider), logger.F("model", a.config.LLM.Model))
		} else {
			llmLog.Warn("LLM API key 未配置，自然语言任务功能不可用")
		}
	}
}

// LLMPlanOnly 仅生成动作计划预览（不执行）
func (a *App) LLMPlanOnly(taskDescription string) ([]map[string]interface{}, error) {
	if llmClient == nil || !llmClient.HasKey() {
		return nil, fmt.Errorf("LLM 未配置，请在 config.yaml 中设置 llm.api_key")
	}

	actions, err := llmClient.PlanActions(taskDescription)
	if err != nil {
		return nil, fmt.Errorf("规划失败: %w", err)
	}

	result := make([]map[string]interface{}, len(actions))
	for i, act := range actions {
		result[i] = map[string]interface{}{
			"type":        act.Type,
			"url":         act.URL,
			"selector":    act.Selector,
			"text":        act.Text,
			"durationMs":  act.DurationMs,
			"direction":   act.Direction,
			"description": act.Description,
		}
	}
	return result, nil
}

// LLMExecuteTask 执行自然语言任务（规划 + 执行 + 录制）
func (a *App) LLMExecuteTask(profileId string, taskDescription string) error {
	if llmClient == nil || !llmClient.HasKey() {
		return fmt.Errorf("LLM 未配置")
	}

	store, err := a.requireRecordingStore()
	if err != nil {
		return err
	}

	bp, err := a.runningRecordingProfile(profileId)
	if err != nil {
		return err
	}

	// 1. LLM 生成动作计划
	a.emit(events.EventLLMTaskPlanning, map[string]interface{}{
		"profileId": profileId, "message": "正在生成动作计划...",
	})

	planActions, err := llmClient.PlanActions(taskDescription)
	if err != nil {
		return fmt.Errorf("规划失败: %w", err)
	}

	actionsJSON := make([]map[string]interface{}, len(planActions))
	for i, pa := range planActions {
		actionsJSON[i] = map[string]interface{}{
			"type": pa.Type, "url": pa.URL, "selector": pa.Selector,
			"text": pa.Text, "durationMs": pa.DurationMs,
			"direction": pa.Direction, "description": pa.Description,
		}
	}
	a.emit(events.EventLLMTaskPlanReady, map[string]interface{}{
		"profileId": profileId, "actions": actionsJSON,
	})

	// 2. 开始录制
	a.recMu.Lock()
	if a.recorders == nil {
		a.recorders = make(map[string]*behavior.Recorder)
	}
	if _, active := a.recorders[profileId]; active {
		a.recMu.Unlock()
		return fmt.Errorf("already recording on profile %s", profileId)
	}
	rec := behavior.NewRecorder()
	if err := rec.StartRecording(bp.DebugPort); err != nil {
		a.recMu.Unlock()
		return fmt.Errorf("start recording: %w", err)
	}
	a.recorders[profileId] = rec
	a.recMu.Unlock()

	// 3. 执行每个动作
	for i, pa := range planActions {
		a.emit(events.EventLLMTaskExecuting, map[string]interface{}{
			"profileId": profileId, "step": i + 1, "total": len(planActions), "action": pa.Description,
		})

		if err := executeLlmAction(bp.DebugPort, pa); err != nil {
			a.recMu.Lock()
			delete(a.recorders, profileId)
			a.recMu.Unlock()
			_, _ = rec.StopRecording(taskDescription)
			return fmt.Errorf("step %d (%s) failed: %w", i+1, pa.Description, err)
		}

		// Human-like delay between actions
		if pa.Type == "wait" && pa.DurationMs > 0 {
			time.Sleep(time.Duration(pa.DurationMs) * time.Millisecond)
		} else {
			time.Sleep(time.Duration(500+pa.DurationMs/10) * time.Millisecond)
		}

		a.emit(events.EventLLMTaskStepDone, map[string]interface{}{
			"profileId": profileId, "step": i + 1, "total": len(planActions),
		})
	}

	// 4. 停止录制并保存
	a.recMu.Lock()
	delete(a.recorders, profileId)
	a.recMu.Unlock()

	recording, err := rec.StopRecording(taskDescription)
	if err != nil {
		return fmt.Errorf("stop recording: %w", err)
	}
	if err := store.Save(recording); err != nil {
		return fmt.Errorf("save recording: %w", err)
	}

	a.emit(events.EventLLMTaskComplete, map[string]interface{}{
		"profileId":   profileId,
		"recordingId": recording.ID,
		"durationMs":  recording.DurationMs,
		"eventCount":  len(recording.Events),
	})

	return nil
}

// LLMHasKey 检查 LLM API key 是否已配置
func (a *App) LLMHasKey() bool {
	return llmClient != nil && llmClient.HasKey()
}

// LLMGetOffsetLibrary 获取50方向偏移模板库
func (a *App) LLMGetOffsetLibrary() []map[string]interface{} {
	lib := offsets.BuiltinOffsetLibrary()
	result := make([]map[string]interface{}, len(lib))
	for i, v := range lib {
		result[i] = map[string]interface{}{
			"id": v.ID, "category": v.Category, "name": v.Name,
			"description": v.Description, "behaviorPreset": v.BehaviorPreset,
		}
	}
	return result
}

type llmCDPMessage struct {
	Method string
	Params map[string]interface{}
}

type llmElementInfo struct {
	OK           bool    `json:"ok"`
	Error        string  `json:"error"`
	X            float64 `json:"x"`
	Y            float64 `json:"y"`
	Tag          string  `json:"tag"`
	InputType    string  `json:"inputType"`
	ID           string  `json:"id"`
	Name         string  `json:"name"`
	Autocomplete string  `json:"autocomplete"`
	Placeholder  string  `json:"placeholder"`
	AriaLabel    string  `json:"ariaLabel"`
	Role         string  `json:"role"`
	ClassName    string  `json:"className"`
}

// executeLlmAction 执行单个 CDP 动作
func executeLlmAction(debugPort int, pa llm.PlannedAction) error {
	ws, err := behavior.ConnectPageCDP(debugPort)
	if err != nil {
		return fmt.Errorf("connect CDP: %w", err)
	}
	defer ws.Close()

	msgID := 1
	send := func(method string, params map[string]interface{}) (json.RawMessage, error) {
		result, err := sendLlmCDPCommand(ws, msgID, method, params)
		msgID++
		return result, err
	}

	switch pa.Type {
	case "goto":
		if pa.URL == "" {
			return fmt.Errorf("goto requires url")
		}
		_, err := send("Page.navigate", map[string]interface{}{"url": pa.URL})
		return err

	case "wait":
		return nil

	case "click":
		if pa.Selector == "" {
			return fmt.Errorf("click requires selector")
		}
		target, err := resolveLlmElement(send, pa.Selector, false, false)
		if err != nil {
			return err
		}
		return sendLlmCDPMessages(send, buildLlmClickCommands(target.X, target.Y))

	case "scroll":
		dy := 500
		if pa.DistancePx != nil {
			dy = int(*pa.DistancePx)
		}
		if pa.Direction == "up" {
			dy = -dy
		}
		x, y, err := resolveLlmViewportCenter(send)
		if err != nil {
			return err
		}
		return sendLlmCDPMessages(send, buildLlmScrollCommands(x, y, float64(dy)))

	case "type":
		if pa.Selector == "" || pa.Text == "" {
			return fmt.Errorf("type requires selector and text")
		}
		target, err := resolveLlmElement(send, pa.Selector, true, true)
		if err != nil {
			return err
		}
		if isSensitiveLlmTarget(target) {
			if _, err := send("Runtime.evaluate", map[string]interface{}{
				"expression":    buildSetInputValueExpression(pa.Selector, pa.Text),
				"returnByValue": true,
			}); err != nil {
				return err
			}
			expr, err := buildAppendRecordedEventsExpression(buildRedactedTypingEvents(pa.Text))
			if err != nil {
				return err
			}
			_, err = send("Runtime.evaluate", map[string]interface{}{
				"expression":    expr,
				"returnByValue": true,
			})
			return err
		}
		return sendLlmCDPMessages(send, buildLlmTypeKeyCommands(pa.Text))

	default:
		return fmt.Errorf("unknown action type: %s", pa.Type)
	}
}

func sendLlmCDPMessages(send func(string, map[string]interface{}) (json.RawMessage, error), messages []llmCDPMessage) error {
	for _, msg := range messages {
		if _, err := send(msg.Method, msg.Params); err != nil {
			return err
		}
	}
	return nil
}

func sendLlmCDPCommand(ws *websocket.Conn, id int, method string, params map[string]interface{}) (json.RawMessage, error) {
	msg := map[string]interface{}{
		"id":     id,
		"method": method,
	}
	if params != nil {
		msg["params"] = params
	}
	if err := ws.WriteJSON(msg); err != nil {
		return nil, fmt.Errorf("write %s: %w", method, err)
	}

	deadline := time.Now().Add(5 * time.Second)
	defer ws.SetReadDeadline(time.Time{})
	for time.Now().Before(deadline) {
		ws.SetReadDeadline(deadline)
		_, raw, err := ws.ReadMessage()
		if err != nil {
			return nil, fmt.Errorf("read %s response: %w", method, err)
		}

		var resp struct {
			ID     int             `json:"id"`
			Result json.RawMessage `json:"result"`
			Error  *struct {
				Message string `json:"message"`
			} `json:"error"`
		}
		if err := json.Unmarshal(raw, &resp); err != nil || resp.ID != id {
			continue
		}
		if resp.Error != nil {
			return nil, fmt.Errorf("cdp %s: %s", method, resp.Error.Message)
		}
		return resp.Result, nil
	}

	return nil, fmt.Errorf("read %s response: no response for id %d", method, id)
}

func resolveLlmElement(send func(string, map[string]interface{}) (json.RawMessage, error), selector string, focus bool, selectText bool) (llmElementInfo, error) {
	raw, err := send("Runtime.evaluate", map[string]interface{}{
		"expression":    buildElementProbeExpression(selector, focus, selectText),
		"returnByValue": true,
	})
	if err != nil {
		return llmElementInfo{}, err
	}

	var info llmElementInfo
	if err := decodeRuntimeValue(raw, &info); err != nil {
		return llmElementInfo{}, fmt.Errorf("decode selector probe: %w", err)
	}
	if !info.OK {
		if info.Error == "" {
			info.Error = "selector not found"
		}
		return llmElementInfo{}, errors.New(info.Error)
	}
	return info, nil
}

func resolveLlmViewportCenter(send func(string, map[string]interface{}) (json.RawMessage, error)) (float64, float64, error) {
	raw, err := send("Runtime.evaluate", map[string]interface{}{
		"expression":    `({x: Math.max(1, Math.floor(window.innerWidth / 2)), y: Math.max(1, Math.floor(window.innerHeight / 2))})`,
		"returnByValue": true,
	})
	if err != nil {
		return 0, 0, err
	}
	var center struct {
		X float64 `json:"x"`
		Y float64 `json:"y"`
	}
	if err := decodeRuntimeValue(raw, &center); err != nil {
		return 0, 0, fmt.Errorf("decode viewport center: %w", err)
	}
	if center.X <= 0 || center.Y <= 0 {
		return 0, 0, fmt.Errorf("invalid viewport center")
	}
	return center.X, center.Y, nil
}

func decodeRuntimeValue(raw json.RawMessage, out interface{}) error {
	var wrapper struct {
		Result struct {
			Value json.RawMessage `json:"value"`
		} `json:"result"`
	}
	if err := json.Unmarshal(raw, &wrapper); err != nil {
		return err
	}
	if len(wrapper.Result.Value) == 0 {
		return fmt.Errorf("missing Runtime.evaluate value")
	}
	return json.Unmarshal(wrapper.Result.Value, out)
}

func buildElementProbeExpression(selector string, focus bool, selectText bool) string {
	return fmt.Sprintf(`(function(selector, focusTarget, selectTarget) {
  const el = document.querySelector(selector);
  if (!el) return { ok: false, error: 'selector not found: ' + selector };
  if (el.scrollIntoView) el.scrollIntoView({ block: 'center', inline: 'center', behavior: 'auto' });
  if (focusTarget && el.focus) el.focus({ preventScroll: true });
  if (selectTarget && typeof el.select === 'function') el.select();
  const rect = el.getBoundingClientRect();
  return {
    ok: true,
    x: Math.round(rect.left + rect.width / 2),
    y: Math.round(rect.top + rect.height / 2),
    tag: (el.tagName || '').toLowerCase(),
    inputType: (el.getAttribute('type') || '').toLowerCase(),
    id: el.id || '',
    name: el.getAttribute('name') || '',
    autocomplete: el.getAttribute('autocomplete') || '',
    placeholder: el.getAttribute('placeholder') || '',
    ariaLabel: el.getAttribute('aria-label') || '',
    role: el.getAttribute('role') || '',
    className: String(el.className || '')
  };
})(%s, %t, %t)`, jsString(selector), focus, selectText)
}

func buildSetInputValueExpression(selector string, text string) string {
	return fmt.Sprintf(`(function(selector, text) {
  const el = document.querySelector(selector);
  if (!el) return false;
  if (el.focus) el.focus({ preventScroll: true });
  if ('value' in el) {
    el.value = text;
  } else {
    el.textContent = text;
  }
  el.dispatchEvent(new InputEvent('input', { bubbles: true, inputType: 'insertText', data: '' }));
  el.dispatchEvent(new Event('change', { bubbles: true }));
  return true;
})(%s, %s)`, jsString(selector), jsString(text))
}

func buildAppendRecordedEventsExpression(events []behavior.RecordedEvent) (string, error) {
	data, err := json.Marshal(events)
	if err != nil {
		return "", fmt.Errorf("marshal recorded events: %w", err)
	}
	return fmt.Sprintf(`(function(events) {
  const target = window.__antRecordedEvents;
  if (!Array.isArray(target)) return false;
  const last = target.length ? Number(target[target.length - 1].t) || 0 : 0;
  for (let i = 0; i < events.length; i++) {
    const evt = Object.assign({}, events[i]);
    evt.t = last + i + 1;
    target.push(evt);
  }
  return true;
})(%s)`, string(data)), nil
}

func buildLlmClickCommands(x, y float64) []llmCDPMessage {
	x = math.Round(x)
	y = math.Round(y)
	return []llmCDPMessage{
		{
			Method: "Input.dispatchMouseEvent",
			Params: map[string]interface{}{
				"type": "mouseMoved",
				"x":    x,
				"y":    y,
			},
		},
		{
			Method: "Input.dispatchMouseEvent",
			Params: map[string]interface{}{
				"type":       "mousePressed",
				"x":          x,
				"y":          y,
				"button":     "left",
				"clickCount": 1,
			},
		},
		{
			Method: "Input.dispatchMouseEvent",
			Params: map[string]interface{}{
				"type":       "mouseReleased",
				"x":          x,
				"y":          y,
				"button":     "left",
				"clickCount": 1,
			},
		},
	}
}

func buildLlmScrollCommands(x, y, deltaY float64) []llmCDPMessage {
	return []llmCDPMessage{
		{
			Method: "Input.dispatchMouseEvent",
			Params: map[string]interface{}{
				"type":   "mouseWheel",
				"x":      math.Round(x),
				"y":      math.Round(y),
				"deltaX": 0,
				"deltaY": math.Round(deltaY),
			},
		},
	}
}

func buildLlmTypeKeyCommands(text string) []llmCDPMessage {
	commands := make([]llmCDPMessage, 0, len([]rune(text))*3)
	for _, r := range text {
		key := string(r)
		keyText := string(r)
		if r == '\n' {
			key = "Enter"
			keyText = "\r"
		}
		commands = append(commands,
			llmCDPMessage{
				Method: "Input.dispatchKeyEvent",
				Params: map[string]interface{}{
					"type": "rawKeyDown",
					"key":  key,
					"text": keyText,
				},
			},
			llmCDPMessage{
				Method: "Input.dispatchKeyEvent",
				Params: map[string]interface{}{
					"type": "char",
					"key":  key,
					"text": keyText,
				},
			},
			llmCDPMessage{
				Method: "Input.dispatchKeyEvent",
				Params: map[string]interface{}{
					"type": "keyUp",
					"key":  key,
				},
			},
		)
	}
	return commands
}

func buildRedactedTypingEvents(text string) []behavior.RecordedEvent {
	count := len([]rune(text))
	if count == 0 {
		count = 1
	}
	events := make([]behavior.RecordedEvent, count)
	for i := range events {
		events[i] = behavior.RecordedEvent{Type: "key", Key: "[redacted]"}
	}
	return events
}

func isSensitiveLlmTarget(info llmElementInfo) bool {
	if info.InputType == "password" {
		return true
	}
	probe := strings.ToLower(strings.Join([]string{
		info.InputType,
		info.ID,
		info.Name,
		info.Autocomplete,
		info.Placeholder,
		info.AriaLabel,
		info.Role,
		info.ClassName,
	}, " "))
	keywords := []string{
		"password", "passwd", "passcode", "pwd",
		"captcha", "verification", "verifycode", "verify-code",
		"otp", "one-time", "one_time", "smscode", "sms-code", "authcode", "auth-code",
		"验证码", "校验码", "动态码", "短信码",
	}
	for _, keyword := range keywords {
		if strings.Contains(probe, keyword) {
			return true
		}
	}
	return false
}

func jsString(value string) string {
	data, _ := json.Marshal(value)
	return string(data)
}
