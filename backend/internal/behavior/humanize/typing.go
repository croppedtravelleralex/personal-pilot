package humanize

import "math/rand/v2"

// TypingEventType distinguishes keys, backspaces, and pauses in a typing plan.
type TypingEventType int

const (
	TypingEventKey       TypingEventType = 0
	TypingEventBackspace TypingEventType = 1
	TypingEventPause     TypingEventType = 2
	TypingEventModifier  TypingEventType = 3
	TypingEventIME       TypingEventType = 4
	TypingEventTab       TypingEventType = 5
	TypingEventClipboard TypingEventType = 6
)

// TypingEvent is one step in a typing plan.
type TypingEvent struct {
	Type       TypingEventType
	Ch         rune   // only for Key
	IntervalMs uint32 // for Key and Backspace
	DurationMs uint32 // for Pause
	Action     string // for Modifier/IME/Clipboard
}

// TypingPlan is a complete sequence of typing events for a text string.
type TypingPlan struct {
	Events  []TypingEvent
	TotalMs uint32
}

// BuildTypingPlan builds a complete humanized typing plan for the given text.
// Includes per-character intervals, typos with backspace corrections,
// mid-text thinking pauses, and word-boundary pauses.
func BuildTypingPlan(text string, config *HumanizationConfig) TypingPlan {
	events := make([]TypingEvent, 0, len(text)*2)
	var totalMs uint32

	if len(text) == 0 {
		return TypingPlan{Events: events, TotalMs: 0}
	}

	cfg := &config.Typing
	runes := []rune(text)

	for i, ch := range runes {
		// 1. Pre-character "reaction" jitter
		preDelayMax := min(30, cfg.SpeedVariancePercent)
		if preDelayMax > 0 {
			preMs := randRangeUint32(0, preDelayMax)
			if preMs > 0 {
				totalMs += preMs
				events = append(events, TypingEvent{Type: TypingEventPause, DurationMs: preMs})
			}
		}

		// 2. Occasional mid-text thinking pause
		if cfg.PauseChance > 0 && rand.Float64() < cfg.PauseChance {
			totalMs += cfg.PauseDurationMs
			events = append(events, TypingEvent{Type: TypingEventPause, DurationMs: cfg.PauseDurationMs})
		}

		// 3. Compute base interval for this character
		baseInterval := uint32(60000 / max(1, cfg.BaseWPM))
		variance := uint32(float64(baseInterval) * float64(cfg.SpeedVariancePercent) / 100.0)
		var interval uint32
		if variance > 0 {
			interval = randRangeUint32(baseInterval-variance, baseInterval+variance)
		} else {
			interval = baseInterval
		}

		// 4. Shift/caps/punctuation penalty
		adjustedInterval := interval
		if (ch >= 'A' && ch <= 'Z') || isASCIISymbol(ch) {
			adjustedInterval = interval + 25
		}

		// 5. Typo simulation with retry
		if cfg.ErrorRetryChance > 0 && rand.Float64() < cfg.ErrorRetryChance {
			// Type wrong character (half interval)
			wrongCh := adjacentKey(ch)
			events = append(events, TypingEvent{Type: TypingEventKey, Ch: wrongCh, IntervalMs: adjustedInterval / 2})
			totalMs += adjustedInterval / 2

			// "Notice" pause
			noticePause := randRangeUint32(150, 350)
			totalMs += noticePause
			events = append(events, TypingEvent{Type: TypingEventPause, DurationMs: noticePause})

			// Backspace
			events = append(events, TypingEvent{Type: TypingEventBackspace, IntervalMs: 80})
			totalMs += 80

			// Retype correctly (full interval)
			events = append(events, TypingEvent{Type: TypingEventKey, Ch: ch, IntervalMs: adjustedInterval})
			totalMs += adjustedInterval
		} else {
			// Normal keystroke
			events = append(events, TypingEvent{Type: TypingEventKey, Ch: ch, IntervalMs: adjustedInterval})
			totalMs += adjustedInterval
		}

		// 6. End-of-word pause (after whitespace, unless last char)
		if ch == ' ' && i < len(runes)-1 {
			wordPause := randRangeUint32(30, 80)
			totalMs += wordPause
			events = append(events, TypingEvent{Type: TypingEventPause, DurationMs: wordPause})
		}
	}

	return TypingPlan{Events: events, TotalMs: totalMs}
}

// CharInterval returns the typing interval for a single character (without typo logic).
// Useful for inline estimates when a full plan is not needed.
func CharInterval(config *HumanizationConfig, ch rune) uint32 {
	cfg := &config.Typing
	base := uint32(60000 / max(1, cfg.BaseWPM))
	variance := uint32(float64(base) * float64(cfg.SpeedVariancePercent) / 100.0)
	var interval uint32
	if variance > 0 {
		interval = randRangeUint32(base-variance, base+variance)
	} else {
		interval = base
	}
	if (ch >= 'A' && ch <= 'Z') || isASCIISymbol(ch) {
		interval += 25
	}
	return interval
}

// adjacentKey returns a key "near" the given key on a QWERTY layout
// to simulate a common typo.
func adjacentKey(ch rune) rune {
	neighbors := map[rune]rune{
		'a': 's', 's': 'a', 'd': 's', 'f': 'd', 'g': 'f',
		'h': 'g', 'j': 'h', 'k': 'j', 'l': 'k',
		'q': 'w', 'w': 'q', 'e': 'w', 'r': 'e', 't': 'r',
		'y': 't', 'u': 'y', 'i': 'u', 'o': 'i', 'p': 'o',
		'z': 'x', 'x': 'z', 'c': 'x', 'v': 'c', 'b': 'v',
		'n': 'b', 'm': 'n',
	}
	lower := ch
	if ch >= 'A' && ch <= 'Z' {
		lower = ch + 32
	}
	if n, ok := neighbors[lower]; ok {
		if ch >= 'A' && ch <= 'Z' {
			return n - 32
		}
		return n
	}
	return ch
}

// TypingContext tunes intervals for field-level typing semantics.
type TypingContext struct {
	FieldKind        string
	MinutesElapsed   uint32
	CaptchaCellCount uint32
}

func BuildContextTypingPlan(text string, context TypingContext, config *HumanizationConfig) TypingPlan {
	plan := BuildTypingPlan(text, config)
	multiplier := 1.0 + float64(context.MinutesElapsed)*0.025
	if context.FieldKind == "password" {
		multiplier *= 1.4
	}
	if multiplier != 1.0 {
		for i := range plan.Events {
			if plan.Events[i].IntervalMs > 0 {
				plan.Events[i].IntervalMs = uint32(float64(plan.Events[i].IntervalMs) * multiplier)
			}
			if plan.Events[i].DurationMs > 0 {
				plan.Events[i].DurationMs = uint32(float64(plan.Events[i].DurationMs) * multiplier)
			}
		}
		plan.TotalMs = sumTypingPlanMs(plan.Events)
	}
	if context.FieldKind == "captcha" && context.CaptchaCellCount > 1 {
		plan = insertCaptchaCellPauses(plan, context.CaptchaCellCount)
	}
	return plan
}

func BuildIMEPlan(pinyin string, candidateMoves uint32, config *HumanizationConfig) TypingPlan {
	plan := BuildTypingPlan(pinyin, config)
	for i := uint32(0); i < candidateMoves; i++ {
		plan.Events = append(plan.Events, TypingEvent{Type: TypingEventIME, Action: "candidate_arrow", IntervalMs: randRangeUint32(120, 260)})
	}
	plan.Events = append(plan.Events, TypingEvent{Type: TypingEventIME, Action: "candidate_enter", IntervalMs: randRangeUint32(120, 220)})
	plan.TotalMs = sumTypingPlanMs(plan.Events)
	return plan
}

func BuildModifierKeyPlan(sequence string, config *HumanizationConfig) TypingPlan {
	interval := uint32(90)
	if config != nil {
		interval = max(60, config.Timing.PreActionDelayMs+40)
	}
	return TypingPlan{Events: []TypingEvent{
		{Type: TypingEventModifier, Action: "ctrl_down", IntervalMs: interval},
		{Type: TypingEventModifier, Action: sequence, IntervalMs: randRangeUint32(45, 90)},
		{Type: TypingEventModifier, Action: "ctrl_up", IntervalMs: randRangeUint32(30, 70)},
	}, TotalMs: interval + 90 + 70}
}

func BuildTabSwitchPlan(fields uint32) TypingPlan {
	if fields == 0 {
		return TypingPlan{}
	}
	events := make([]TypingEvent, 0, fields)
	var total uint32
	for i := uint32(0); i < fields; i++ {
		interval := uint32(sampleNormal(500, 200, 700))
		if interval < 120 {
			interval = 120
		}
		events = append(events, TypingEvent{Type: TypingEventTab, Action: "tab", IntervalMs: interval})
		total += interval
	}
	return TypingPlan{Events: events, TotalMs: total}
}

func BuildClipboardPlan(config *HumanizationConfig) TypingPlan {
	plan := TypingPlan{}
	for _, action := range []string{"ctrl+a", "ctrl+c", "ctrl+v"} {
		part := BuildModifierKeyPlan(action, config)
		plan.Events = append(plan.Events, part.Events...)
	}
	plan.TotalMs = sumTypingPlanMs(plan.Events)
	return plan
}

func FingerSpeedRatio(ch rune) float64 {
	switch ch {
	case 'q', 'a', 'z', 'p':
		return 0.40
	case 'w', 's', 'x', 'o', 'l':
		return 0.55
	case 'e', 'd', 'c', 'i', 'k':
		return 0.70
	case 'r', 't', 'f', 'g', 'v', 'b', 'y', 'u', 'h', 'j', 'n', 'm':
		return 0.80
	default:
		return 0.65
	}
}

func sumTypingPlanMs(events []TypingEvent) uint32 {
	var total uint32
	for _, event := range events {
		total += event.IntervalMs + event.DurationMs
	}
	return total
}

func insertCaptchaCellPauses(plan TypingPlan, cells uint32) TypingPlan {
	if cells <= 1 {
		return plan
	}
	keysSeen := uint32(0)
	next := make([]TypingEvent, 0, len(plan.Events)+int(cells))
	for _, event := range plan.Events {
		next = append(next, event)
		if event.Type == TypingEventKey {
			keysSeen++
			if keysSeen < cells {
				next = append(next, TypingEvent{Type: TypingEventPause, DurationMs: randRangeUint32(180, 420)})
			}
		}
	}
	return TypingPlan{Events: next, TotalMs: sumTypingPlanMs(next)}
}
