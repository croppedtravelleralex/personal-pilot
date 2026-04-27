package humanize

import (
	"testing"
)

// TestMiddlewareFullPipeline 测试从 LLM 动作到变异执行指令的完整管线。
func TestMiddlewareFullPipeline(t *testing.T) {
	cfg := DefaultConfig()
	mw := NewBehavioralMutationMiddleware(cfg)

	actions := []LlmAction{
		{Type: ActionGoto, URL: "https://example.com/login"},
		{Type: ActionWait, DurationMs: 2000},
		{Type: ActionTypeText, Selector: "#username", Text: "testuser"},
		{Type: ActionTypeText, Selector: "#password", Text: "s3cret!"},
		{Type: ActionClick, Selector: "#submit"},
		{Type: ActionWait, DurationMs: 3000},
		{Type: ActionGetHtml, Selector: "body"},
	}

	var totalPreGap int64
	typingCount := 0
	clickCount := 0
	gotoCount := 0

	for i, action := range actions {
		var bounds *ElementBounds
		if action.Type == ActionClick {
			bounds = &ElementBounds{X1: 100, Y1: 200, X2: 180, Y2: 240, Width: 80, Height: 40}
		}

		mutated := mw.Mutate(action, bounds)

		// 验证变异结果的基本完整性
		if mutated.Type == 0 && action.Type != ActionOpenBrowser {
			t.Errorf("步骤 %d: MutatedType 不应为0, action=%v", i, action.Type)
		}

		totalPreGap += int64(mutated.PreGapMs)

		switch action.Type {
		case ActionGoto:
			gotoCount++
			if mutated.URL != action.URL {
				t.Errorf("步骤 %d: URL 被篡改, got=%q want=%q", i, mutated.URL, action.URL)
			}
		case ActionTypeText:
			typingCount++
			if mutated.TypingPlan == nil {
				t.Errorf("步骤 %d: TypeText 缺少 TypingPlan", i)
			}
		case ActionClick:
			clickCount++
			if mutated.ClickTarget == nil {
				t.Errorf("步骤 %d: Click 缺少 ClickTarget", i)
			}
		}
	}

	if gotoCount != 1 {
		t.Errorf("Goto 动作数 = %d, want 1", gotoCount)
	}
	if typingCount != 2 {
		t.Errorf("TypeText 动作数 = %d, want 2", typingCount)
	}
	if clickCount != 1 {
		t.Errorf("Click 动作数 = %d, want 1", clickCount)
	}
	if totalPreGap == 0 {
		t.Error("所有 PreGap 均为0，Medium 级别应产生间隔")
	}
}

// TestMiddlewareRetryLifecycle 测试重试决策的完整生命周期。
func TestMiddlewareRetryLifecycle(t *testing.T) {
	cfg := DefaultConfig()
	cfg.Failure.Type = FailureHuman
	cfg.Failure.MaxRetries = 4
	cfg.Failure.GiveUpChance = 0

	// 模拟三次连续失败
	tests := []struct {
		code     ActionErrorCode
		attempt  uint32
		wantGive bool
		wantWait bool
	}{
		{ErrNavigationTimeout, 1, false, true},     // 超时可重试
		{ErrNavigationTimeout, 2, false, true},     // 再次超时仍可重试
		{ErrNavigationTimeout, 3, false, true},     // 第三次超时仍可重试
		{ErrSessionCrashed, 1, true, false},        // 会话崩溃应立即放弃
	}

	for i, tc := range tests {
		decision := HumanizedRetryDecision(tc.code, tc.attempt, &cfg)
		isGiveUp := decision.Action.Type == RecoveryGiveUp
		if isGiveUp != tc.wantGive {
			t.Errorf("步骤 %d: GiveUp = %v, want %v (actionType=%v)",
				i, isGiveUp, tc.wantGive, decision.Action.Type)
		}
		if tc.wantWait && decision.Action.WaitMs == 0 &&
			decision.Action.Type != RecoveryRetryAfter {
			t.Errorf("步骤 %d: 未等待重试 (actionType=%v)", i, decision.Action.Type)
		}
	}
}

// TestMiddlewareWithLevels 测试四级预设的完整管线效果。
func TestMiddlewareWithLevels(t *testing.T) {
	levels := []struct {
		level      HumanizationLevel
		expectGaps bool
	}{
		{LevelNone, false},     // None: 无延迟
		{LevelMinimal, true},   // Minimal: 有延迟
		{LevelMedium, true},    // Medium: 有延迟和输入计划
		{LevelHigh, true},      // High: 完整的延迟和计划
	}

	action := LlmAction{Type: ActionTypeText, Selector: "#input", Text: "hello world"}
	bounds := &ElementBounds{X1: 10, Y1: 10, X2: 100, Y2: 50, Width: 90, Height: 40}

	for _, lv := range levels {
		t.Run(levelName(lv.level), func(t *testing.T) {
			cfg := ConfigForLevel(lv.level)
			mw := NewBehavioralMutationMiddleware(cfg)

			// TypeText
			textMutated := mw.Mutate(action, nil)
			if lv.expectGaps && textMutated.PreGapMs == 0 {
				t.Error("期望有 PreGap，但为0")
			}
			if !lv.expectGaps {
				// None 级别应有完整的计划但无延迟
				if textMutated.TypingPlan == nil {
					t.Error("None 级别也应生成 TypingPlan（结构完整，无延迟注入）")
				}
			}

			// Click
			clickMutated := mw.Mutate(LlmAction{Type: ActionClick, Selector: "#btn"}, bounds)
			if lv.expectGaps && clickMutated.PreGapMs == 0 {
				t.Error("Click: 期望有 PreGap，但为0")
			}
			if clickMutated.ClickTarget == nil {
				t.Error("Click: ClickTarget 不应为空")
			}
		})
	}
}

// TestMiddlewareBatchedActions 测试批量动作的性能和一致性。
func TestMiddlewareBatchedActions(t *testing.T) {
	cfg := DefaultConfig()
	mw := NewBehavioralMutationMiddleware(cfg)

	bounds := &ElementBounds{X1: 0, Y1: 0, X2: 200, Y2: 100, Width: 200, Height: 100}

	// 100 个动作连续变异，确认无panic、结果一致
	for i := 0; i < 100; i++ {
		action := LlmAction{Type: ActionClick, Selector: "#item"}
		mutated := mw.Mutate(action, bounds)

		if mutated.ClickTarget == nil {
			t.Fatalf("第 %d 个: ClickTarget 为空", i)
		}
		if mutated.PreGapMs == 0 {
			t.Fatalf("第 %d 个: PreGapMs = 0", i)
		}
	}
}

// TestMiddlewareNilBoundsSafety 测试空边界的安全性。
func TestMiddlewareNilBoundsSafety(t *testing.T) {
	cfg := DefaultConfig()
	mw := NewBehavioralMutationMiddleware(cfg)

	// Click 无边界，应使用默认中心点
	action := LlmAction{Type: ActionClick, Selector: "#unknown"}
	mutated := mw.Mutate(action, nil)

	if mutated.ClickTarget == nil {
		t.Fatal("ClickTarget 不应为 nil，应使用默认坐标")
	}
}

// TestMiddlewareConfigImmutability 测试配置在变异过程中不被修改。
func TestMiddlewareConfigImmutability(t *testing.T) {
	cfg := DefaultConfig()
	originalLevel := cfg.Level
	originalTiming := cfg.Timing

	mw := NewBehavioralMutationMiddleware(cfg)

	// 执行多次变异
	for i := 0; i < 50; i++ {
		mw.Mutate(LlmAction{Type: ActionTypeText, Text: "test"}, nil)
		mw.Mutate(LlmAction{Type: ActionClick, Selector: "#btn"}, &ElementBounds{
			X1: 0, Y1: 0, X2: 100, Y2: 50, Width: 100, Height: 50,
		})
	}

	// 验证配置未被修改
	if cfg.Level != originalLevel {
		t.Errorf("Level 被修改: %v -> %v", originalLevel, cfg.Level)
	}
	if cfg.Timing != originalTiming {
		t.Error("Timing 配置被修改")
	}
}

// TestMiddlewareTimingConsistency 验证延迟的统计一致性。
func TestMiddlewareTimingConsistency(t *testing.T) {
	cfg := DefaultConfig()
	mw := NewBehavioralMutationMiddleware(cfg)

	var gaps []uint32
	for i := 0; i < 200; i++ {
		mutated := mw.Mutate(LlmAction{Type: ActionWait, DurationMs: 500}, nil)
		gaps = append(gaps, mutated.PreGapMs)
	}

	// 验证所有间隔都在合理范围内 (>0 且 < 5秒)
	zeroCount := 0
	for i, g := range gaps {
		if g == 0 {
			zeroCount++
			if zeroCount > 10 {
				t.Errorf("过多零间隔: 第 %d 个为0", i)
				break
			}
		}
		if g > 5000 {
			t.Errorf("第 %d 个: gap = %d 过大 (>5000ms)", i, g)
		}
	}

	// 计算均值，应在合理范围
	var sum uint64
	for _, g := range gaps {
		sum += uint64(g)
	}
	avg := sum / uint64(len(gaps))
	if avg > 2000 {
		t.Errorf("平均间隔 = %dms, 过长（>2000ms）", avg)
	}
}

// levelName 返回 HumanizationLevel 的可读名称。
func levelName(l HumanizationLevel) string {
	switch l {
	case LevelNone:
		return "None"
	case LevelMinimal:
		return "Minimal"
	case LevelMedium:
		return "Medium"
	case LevelHigh:
		return "High"
	default:
		return "Unknown"
	}
}

// TestMiddlewareGotoTiming 验证导航操作的延迟注入。
func TestMiddlewareGotoTiming(t *testing.T) {
	cfg := DefaultConfig()
	mw := NewBehavioralMutationMiddleware(cfg)

	mutated := mw.Mutate(LlmAction{Type: ActionGoto, URL: "https://example.com"}, nil)

	if mutated.URL != "https://example.com" {
		t.Errorf("URL = %q, want https://example.com", mutated.URL)
	}
	if mutated.PreGapMs == 0 {
		t.Error("导航前应有思考延迟")
	}
}

// TestHumanizedRetryDecisionFatalErrors 测试致命错误的放弃决策。
func TestHumanizedRetryDecisionFatalErrors(t *testing.T) {
	cfg := DefaultConfig()
	cfg.Failure.Type = FailureHuman

	fatalCodes := []ActionErrorCode{ErrSessionCrashed, ErrFingerprintRejected}
	for _, code := range fatalCodes {
		decision := HumanizedRetryDecision(code, 1, &cfg)
		if decision.Action.Type != RecoveryGiveUp {
			t.Errorf("致命错误 %v: actionType = %v, 期望 RecoveryGiveUp", code, decision.Action.Type)
		}
	}
}

// TestHumanizedRetryDecisionExhausted 测试重试次数耗尽。
func TestHumanizedRetryDecisionExhausted(t *testing.T) {
	cfg := DefaultConfig()
	cfg.Failure.Type = FailureHuman
	cfg.Failure.MaxRetries = 4
	cfg.Failure.GiveUpChance = 0

	// 超过最大重试次数
	decision := HumanizedRetryDecision(ErrElementNotFound, 5, &cfg)
	if decision.Action.Type != RecoveryGiveUp {
		t.Errorf("耗尽重试: actionType = %v, 期望 RecoveryGiveUp", decision.Action.Type)
	}
}

// TestHumanizedRetryDecisionInstantMode 测试无延迟模式的快速重试。
func TestHumanizedRetryDecisionInstantMode(t *testing.T) {
	cfg := DefaultConfig()
	cfg.Failure.Type = FailureInstant
	cfg.Failure.MaxRetries = 5

	// Instant 模式应直接重试
	decision := HumanizedRetryDecision(ErrElementNotFound, 1, &cfg)
	if decision.Action.Type != RecoveryRetry {
		t.Errorf("Instant: actionType = %v, 期望 RecoveryRetry", decision.Action.Type)
	}

	// 超过上限应放弃
	decision2 := HumanizedRetryDecision(ErrElementNotFound, 6, &cfg)
	if decision2.Action.Type != RecoveryGiveUp {
		t.Errorf("Instant耗尽: actionType = %v, 期望 RecoveryGiveUp", decision2.Action.Type)
	}
}

// TestSuggestedApproachCoversAllCodes 验证所有错误码都有建议方案。
func TestSuggestedApproachCoversAllCodes(t *testing.T) {
	codes := []ActionErrorCode{
		ErrElementNotFound, ErrClickFailed, ErrNavigationTimeout,
		ErrProxyDead, ErrFingerprintRejected, ErrSessionCrashed,
		ErrRateLimited, ErrContentBlinded, ErrUnknown,
	}

	for _, code := range codes {
		approach := SuggestedApproach(code)
		if approach == "" {
			t.Errorf("错误码 %v: SuggestedApproach 为空", code)
		}
	}
}
