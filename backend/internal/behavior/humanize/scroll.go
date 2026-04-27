package humanize

import "math"

// ScrollStepType enumerates the kinds of steps in a scroll plan.
type ScrollStepType int

const (
	ScrollStepBy    ScrollStepType = 0 // ScrollBy a delta
	ScrollStepPause ScrollStepType = 1 // Pause for a duration
	ScrollStepTo    ScrollStepType = 2 // ScrollTo absolute position
)

// ScrollStep is one step in a scroll plan.
type ScrollStep struct {
	Type       ScrollStepType
	DeltaPx    int32  // positive=down, negative=up (for ScrollBy)
	Y          int32  // absolute position (for ScrollTo)
	Speed      ScrollSpeed
	DurationMs uint32 // for Pause
}

// ScrollPlan is a complete humanized scroll sequence.
type ScrollPlan struct {
	Steps           []ScrollStep
	TotalDistancePx uint32
	TotalMs         uint32
}

// BuildScrollPlan builds a humanized scroll plan for the given target distance.
// Implements a 4-phase overshoot-return pattern:
//  1. Overshoot — scroll past the target (fast)
//  2. Pause — "reading/orienting" pause at overshoot
//  3. Return — scroll back to the target (slow)
//  4. Micro-adjust — tiny corrections (±30px)
func BuildScrollPlan(targetDistancePx uint32, config *HumanizationConfig) ScrollPlan {
	// Fast path: no humanization
	if config.Level == LevelNone || config.Scroll.OvershootRatio == nil {
		return ScrollPlan{
			Steps: []ScrollStep{{
				Type:    ScrollStepBy,
				DeltaPx: int32(targetDistancePx),
				Speed:   ScrollFast,
			}},
			TotalDistancePx: targetDistancePx,
			TotalMs:         0,
		}
	}

	ratio := *config.Scroll.OvershootRatio
	speed := config.Scroll.Speed

	overshootPx := uint32(float64(targetDistancePx) * ratio)
	totalDistance := targetDistancePx + overshootPx*2

	var steps []ScrollStep
	var totalMs uint32

	// Phase 0: Hover before scrolling
	if hms := config.Scroll.HoverBeforeScrollMs; hms != nil {
		jitter := randRangeUint32(0, 30)
		dur := *hms + jitter
		steps = append(steps, ScrollStep{Type: ScrollStepPause, DurationMs: dur})
		totalMs += dur
	}

	// Phase 1: Overshoot
	overshootDur := scrollStepDuration(overshootPx, speed)
	steps = append(steps, ScrollStep{Type: ScrollStepBy, DeltaPx: int32(overshootPx), Speed: speed})
	totalMs += overshootDur

	// Phase 2: Pause at overshoot
	pause1 := randRangeUint32(200, 450)
	steps = append(steps, ScrollStep{Type: ScrollStepPause, DurationMs: pause1})
	totalMs += pause1

	// Phase 3: Scroll back to target (slower)
	backDur := scrollStepDuration(overshootPx, ScrollSlow)
	steps = append(steps, ScrollStep{Type: ScrollStepBy, DeltaPx: -int32(overshootPx), Speed: ScrollSlow})
	totalMs += backDur

	// Phase 4: Micro-adjustments
	microAdjust := int32(randRangeUint32(0, 60)) - 30 // range [-30, +30]
	if absInt32(microAdjust) > 5 {
		adjustDur := scrollStepDuration(uint32(absInt32(microAdjust)), ScrollSlow)
		steps = append(steps, ScrollStep{Type: ScrollStepBy, DeltaPx: microAdjust, Speed: ScrollSlow})
		totalMs += adjustDur
	}

	return ScrollPlan{
		Steps:           steps,
		TotalDistancePx: totalDistance,
		TotalMs:         totalMs,
	}
}

// BuildElementScrollPlan computes a scroll plan that positions an element
// at roughly 1/3 from the top of the viewport.
func BuildElementScrollPlan(elementYPosition, viewportHeight int32, config *HumanizationConfig) ScrollPlan {
	targetPx := int32(math.Max(0, float64(elementYPosition-viewportHeight/3)))
	return BuildScrollPlan(uint32(targetPx), config)
}

// scrollStepDuration converts scroll distance to time based on speed.
func scrollStepDuration(px uint32, speed ScrollSpeed) uint32 {
	absPx := px
	if absPx == 0 {
		return 0
	}
	switch speed {
	case ScrollFast:
		return absPx / 3
	case ScrollNormal:
		return absPx / 2
	default: // ScrollSlow
		return absPx
	}
}

func absInt32(x int32) int32 {
	if x < 0 {
		return -x
	}
	return x
}
