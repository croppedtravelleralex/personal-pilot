package humanize

import "math"

// ScrollStepType enumerates the kinds of steps in a scroll plan.
type ScrollStepType int

const (
	ScrollStepBy          ScrollStepType = 0 // ScrollBy a delta
	ScrollStepPause       ScrollStepType = 1 // Pause for a duration
	ScrollStepTo          ScrollStepType = 2 // ScrollTo absolute position
	ScrollStepMicroJitter ScrollStepType = 3
	ScrollStepLoadMore    ScrollStepType = 4
)

// ScrollStep is one step in a scroll plan.
type ScrollStep struct {
	Type       ScrollStepType
	DeltaPx    int32 // positive=down, negative=up (for ScrollBy)
	Y          int32 // absolute position (for ScrollTo)
	Speed      ScrollSpeed
	DurationMs uint32 // for Pause
	Reason     string
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

// ContentLandmark identifies page structure that changes scroll cadence.
type ContentLandmark struct {
	Y    int32
	Kind string
}

func BuildInertialScrollPlan(initialVelocityPx uint32, friction float64, config *HumanizationConfig) ScrollPlan {
	if friction <= 0 || friction >= 1 {
		friction = 0.82
	}
	velocity := float64(initialVelocityPx)
	var steps []ScrollStep
	var totalDistance uint32
	var totalMs uint32
	for velocity >= 8 {
		delta := int32(math.Round(velocity / 6))
		steps = append(steps, ScrollStep{Type: ScrollStepBy, DeltaPx: delta, Speed: ScrollNormal})
		totalDistance += uint32(absInt32(delta))
		totalMs += 16
		velocity *= friction
	}
	if config != nil && config.Level >= LevelMedium {
		steps = append(steps, ScrollStep{Type: ScrollStepBy, DeltaPx: -12, Speed: ScrollSlow, Reason: "elastic_return"})
		totalDistance += 12
		totalMs += 80
	}
	return ScrollPlan{Steps: steps, TotalDistancePx: totalDistance, TotalMs: totalMs}
}

func BuildContentAwareScrollPlan(targetDistancePx uint32, landmarks []ContentLandmark, config *HumanizationConfig) ScrollPlan {
	plan := BuildScrollPlan(targetDistancePx, config)
	for _, landmark := range landmarks {
		if landmark.Y < 0 || uint32(landmark.Y) > targetDistancePx {
			continue
		}
		switch landmark.Kind {
		case "paragraph_end":
			plan.Steps = append(plan.Steps, ScrollStep{Type: ScrollStepPause, DurationMs: randRangeUint32(220, 520), Reason: "paragraph_end"})
		case "image":
			plan.Steps = append(plan.Steps, ScrollStep{Type: ScrollStepPause, DurationMs: randRangeUint32(450, 1100), Reason: "image_inspect"})
		case "heading":
			plan.Steps = append(plan.Steps, ScrollStep{Type: ScrollStepBy, DeltaPx: 80, Speed: ScrollFast, Reason: "heading_skip"})
		}
	}
	plan.TotalMs = sumScrollMs(plan.Steps)
	return plan
}

func MaybeAddReread(plan ScrollPlan, probability float64) ScrollPlan {
	if probability <= 0 {
		return plan
	}
	if probability > 1 || randRangeFloat64(0, 1) <= probability {
		delta := int32(randRangeUint32(100, 300))
		plan.Steps = append(plan.Steps,
			ScrollStep{Type: ScrollStepBy, DeltaPx: -delta, Speed: ScrollSlow, Reason: "reread_backtrack"},
			ScrollStep{Type: ScrollStepPause, DurationMs: randRangeUint32(350, 900), Reason: "reread_pause"},
		)
		plan.TotalDistancePx += uint32(delta)
		plan.TotalMs = sumScrollMs(plan.Steps)
	}
	return plan
}

func BuildInfiniteScrollPlan(viewportDistancePx uint32, loadCycles uint32, config *HumanizationConfig) ScrollPlan {
	plan := BuildScrollPlan(viewportDistancePx, config)
	for i := uint32(0); i < loadCycles; i++ {
		plan.Steps = append(plan.Steps,
			ScrollStep{Type: ScrollStepPause, DurationMs: randRangeUint32(800, 1800), Reason: "load_more_wait"},
			ScrollStep{Type: ScrollStepLoadMore, DurationMs: randRangeUint32(200, 500), Reason: "load_more"},
			ScrollStep{Type: ScrollStepBy, DeltaPx: int32(viewportDistancePx / 2), Speed: ScrollNormal, Reason: "continue_after_load"},
		)
		plan.TotalDistancePx += viewportDistancePx / 2
	}
	plan.TotalMs = sumScrollMs(plan.Steps)
	return plan
}

func AddMicroJitter(plan ScrollPlan, count uint32) ScrollPlan {
	for i := uint32(0); i < count; i++ {
		delta := int32(randRangeUint32(1, 20))
		if i%2 == 0 {
			delta = -delta
		}
		plan.Steps = append(plan.Steps, ScrollStep{Type: ScrollStepMicroJitter, DeltaPx: delta, Speed: ScrollSlow, Reason: "micro_jitter"})
		plan.TotalDistancePx += uint32(absInt32(delta))
	}
	plan.TotalMs = sumScrollMs(plan.Steps)
	return plan
}

func sumScrollMs(steps []ScrollStep) uint32 {
	var total uint32
	for _, step := range steps {
		if step.DurationMs > 0 {
			total += step.DurationMs
			continue
		}
		if step.Type == ScrollStepBy || step.Type == ScrollStepMicroJitter {
			total += scrollStepDuration(uint32(absInt32(step.DeltaPx)), step.Speed)
		}
	}
	return total
}
