package humanize

import (
	"math"
	"math/rand/v2"
)

type PointerPoint struct {
	X  float64
	Y  float64
	Ms uint32
}

type PointerTrajectoryPlan struct {
	Points         []PointerPoint
	MovementTimeMs uint32
	DistancePx     float64
	TargetWidthPx  float64
	NoiseAmplitude float64
}

type ClickPhaseType string

const (
	ClickPhaseMove        ClickPhaseType = "move"
	ClickPhasePrePress    ClickPhaseType = "pre_press"
	ClickPhasePressPeak   ClickPhaseType = "press_peak"
	ClickPhasePostPress   ClickPhaseType = "post_press"
	ClickPhaseRelease     ClickPhaseType = "release"
	ClickPhaseInterClick  ClickPhaseType = "inter_click"
	ClickPhaseContextMenu ClickPhaseType = "context_menu"
	ClickPhaseDragHold    ClickPhaseType = "drag_hold"
)

type ClickPhase struct {
	Type       ClickPhaseType
	DurationMs uint32
	X          float64
	Y          float64
}

type PointerActionPlan struct {
	Trajectory PointerTrajectoryPlan
	Phases     []ClickPhase
}

func BuildFittsTrajectory(fromX, fromY, toX, toY float64, targetWidthPx uint32, seed uint64) PointerTrajectoryPlan {
	dx := toX - fromX
	dy := toY - fromY
	distance := math.Hypot(dx, dy)
	width := math.Max(1, float64(targetWidthPx))
	movementTime := uint32(math.Round(80 + 120*math.Log2(distance/width+1)))
	if movementTime < 120 {
		movementTime = 120
	}
	steps := int(math.Ceil(float64(movementTime) / 16.0))
	if steps < 8 {
		steps = 8
	}
	if steps > 96 {
		steps = 96
	}
	rng := rand.New(rand.NewPCG(seed^0x873c4d2d0a6f12af, splitMix64(seed^0x51ed2705)))
	noise := math.Min(6, math.Max(0.5, distance/220))
	points := make([]PointerPoint, 0, steps+1)
	for i := 0; i <= steps; i++ {
		t := float64(i) / float64(steps)
		eased := easeInOutCubic(t)
		amp := math.Sin(math.Pi*t) * noise
		nx := pinkNoise(rng, 10) * amp
		ny := pinkNoise(rng, 10) * amp
		points = append(points, PointerPoint{
			X:  fromX + dx*eased + nx,
			Y:  fromY + dy*eased + ny,
			Ms: uint32(math.Round(float64(movementTime) * t)),
		})
	}
	points[len(points)-1] = PointerPoint{X: toX, Y: toY, Ms: movementTime}
	return PointerTrajectoryPlan{Points: points, MovementTimeMs: movementTime, DistancePx: distance, TargetWidthPx: width, NoiseAmplitude: noise}
}

func BuildFourPhaseClickPlan(fromX, fromY, toX, toY float64, targetWidthPx uint32, seed uint64) PointerActionPlan {
	trajectory := BuildFittsTrajectory(fromX, fromY, toX, toY, targetWidthPx, seed)
	return PointerActionPlan{Trajectory: trajectory, Phases: []ClickPhase{
		{Type: ClickPhaseMove, DurationMs: trajectory.MovementTimeMs, X: toX, Y: toY},
		{Type: ClickPhasePrePress, DurationMs: 20, X: toX, Y: toY},
		{Type: ClickPhasePressPeak, DurationMs: 12, X: toX, Y: toY},
		{Type: ClickPhasePostPress, DurationMs: 50, X: toX, Y: toY},
		{Type: ClickPhaseRelease, DurationMs: 8, X: toX, Y: toY},
	}}
}

func BuildDoubleClickPlan(fromX, fromY, toX, toY float64, targetWidthPx uint32, seed uint64) PointerActionPlan {
	base := BuildFourPhaseClickPlan(fromX, fromY, toX, toY, targetWidthPx, seed)
	rng := rand.New(rand.NewPCG(seed^0x44ab13, splitMix64(seed^0x880355)))
	interval := clampPointerUint32(uint32(math.Round(300+normalLike(rng)*50)), 180, 450)
	microX := toX + (rng.Float64()*10 - 5)
	microY := toY + (rng.Float64()*10 - 5)
	base.Phases = append(base.Phases,
		ClickPhase{Type: ClickPhaseInterClick, DurationMs: interval, X: microX, Y: microY},
		ClickPhase{Type: ClickPhasePrePress, DurationMs: 20, X: microX, Y: microY},
		ClickPhase{Type: ClickPhasePressPeak, DurationMs: 12, X: microX, Y: microY},
		ClickPhase{Type: ClickPhasePostPress, DurationMs: 50, X: microX, Y: microY},
		ClickPhase{Type: ClickPhaseRelease, DurationMs: 8, X: microX, Y: microY},
	)
	return base
}

func BuildDragPlan(fromX, fromY, toX, toY float64, targetWidthPx uint32, seed uint64) PointerActionPlan {
	trajectory := BuildFittsTrajectory(fromX, fromY, toX, toY, targetWidthPx, seed)
	return PointerActionPlan{Trajectory: trajectory, Phases: []ClickPhase{
		{Type: ClickPhasePrePress, DurationMs: 40, X: fromX, Y: fromY},
		{Type: ClickPhaseDragHold, DurationMs: 90, X: fromX, Y: fromY},
		{Type: ClickPhaseMove, DurationMs: trajectory.MovementTimeMs, X: toX, Y: toY},
		{Type: ClickPhaseRelease, DurationMs: 30, X: toX, Y: toY},
	}}
}

func BuildContextMenuPlan(fromX, fromY, toX, toY float64, targetWidthPx uint32, seed uint64) PointerActionPlan {
	trajectory := BuildFittsTrajectory(fromX, fromY, toX, toY, targetWidthPx, seed)
	return PointerActionPlan{Trajectory: trajectory, Phases: []ClickPhase{
		{Type: ClickPhaseMove, DurationMs: trajectory.MovementTimeMs, X: toX, Y: toY},
		{Type: ClickPhasePrePress, DurationMs: 120, X: toX, Y: toY},
		{Type: ClickPhaseContextMenu, DurationMs: 20, X: toX, Y: toY},
		{Type: ClickPhaseRelease, DurationMs: 20, X: toX, Y: toY},
	}}
}

// ClickElementType describes the element family used to choose a safe click area.
type ClickElementType string

const (
	ClickElementUnknown    ClickElementType = "unknown"
	ClickElementButton     ClickElementType = "button"
	ClickElementInput      ClickElementType = "input"
	ClickElementMenu       ClickElementType = "menu"
	ClickElementCheckbox   ClickElementType = "checkbox"
	ClickElementTab        ClickElementType = "tab"
	ClickElementTextLink   ClickElementType = "text-link"
	ClickElementIconButton ClickElementType = "icon-button"
	ClickElementCard       ClickElementType = "card"
	ClickElementImage      ClickElementType = "image"
	ClickElementContent    ClickElementType = "content"
)

// ClickTarget describes where and with what trajectory a click should land.
type ClickTarget struct {
	X             int32      // absolute X on page
	Y             int32      // absolute Y on page
	HoverBeforeMs *uint32    // nil = no hover; value = hover duration before click
	Trajectory    [][2]int32 // waypoints: [(hoverX, hoverY), (targetX, targetY)]
}

// ComputeClickTarget computes a humanized click position within an element.
// Existing callers use the conservative unknown-element fallback.
func ComputeClickTarget(centerX, centerY int32, width, height uint32, config *HumanizationConfig) ClickTarget {
	return ComputeClickTargetForElement(centerX, centerY, width, height, ClickElementUnknown, config)
}

// ComputeClickTargetForElement computes a deterministic safe click point for an element type.
func ComputeClickTargetForElement(centerX, centerY int32, width, height uint32, elementType ClickElementType, config *HumanizationConfig) ClickTarget {
	if config == nil || config.Click.RadiusPx == 0 {
		return ClickTarget{
			X: centerX, Y: centerY,
			HoverBeforeMs: nil,
			Trajectory:    nil,
		}
	}

	profile := clickSafetyProfileFor(elementType)
	safeHalfW := safeHalfExtent(width, profile)
	safeHalfH := safeHalfExtent(height, profile)
	maxDX := clickAxisMaxOffset(width, config.Click.RadiusPx, safeHalfW, profile)
	maxDY := clickAxisMaxOffset(height, config.Click.RadiusPx, safeHalfH, profile)

	rng := newClickRand(centerX, centerY, width, height, elementType, config)
	dx := sampleSignedAxis(rng, maxDX)
	dy := sampleSignedAxis(rng, maxDY)
	dx, dy = applyClickBias(dx, dy, config.Click.BiasDirection)

	targetX := clampInt32(centerX+dx, centerX-safeHalfW, centerX+safeHalfW)
	targetY := clampInt32(centerY+dy, centerY-safeHalfH, centerY+safeHalfH)

	// Hover waypoint: slightly different from final click point
	hoverMaxX := minInt32(maxDX, int32(config.Click.RadiusPx)+5)
	hoverMaxY := minInt32(maxDY, int32(config.Click.RadiusPx)+5)
	hoverX := clampInt32(targetX+sampleSignedAxis(rng, hoverMaxX), centerX-safeHalfW, centerX+safeHalfW)
	hoverY := clampInt32(targetY+sampleSignedAxis(rng, hoverMaxY), centerY-safeHalfH, centerY+safeHalfH)

	var hoverBeforeMs *uint32
	if hms := config.Scroll.HoverBeforeScrollMs; hms != nil {
		hoverBeforeMs = hms
	}

	return ClickTarget{
		X:             targetX,
		Y:             targetY,
		HoverBeforeMs: hoverBeforeMs,
		Trajectory:    [][2]int32{{hoverX, hoverY}, {targetX, targetY}},
	}
}

type clickSafetyProfile struct {
	insetRatio  float64
	minInsetPx  uint32
	offsetRatio float64
	allowWide   bool
}

func clickSafetyProfileFor(elementType ClickElementType) clickSafetyProfile {
	switch elementType {
	case ClickElementButton, ClickElementInput, ClickElementMenu, ClickElementCheckbox, ClickElementTab:
		return clickSafetyProfile{insetRatio: 0.25, minInsetPx: 2, offsetRatio: 0.12}
	case ClickElementTextLink, ClickElementIconButton:
		return clickSafetyProfile{insetRatio: 0.30, minInsetPx: 2, offsetRatio: 0.10}
	case ClickElementCard, ClickElementImage, ClickElementContent:
		return clickSafetyProfile{insetRatio: 0.12, minInsetPx: 4, offsetRatio: 0.35, allowWide: true}
	default:
		return clickSafetyProfile{insetRatio: 0.25, minInsetPx: 3, offsetRatio: 0.20}
	}
}

func safeHalfExtent(size uint32, profile clickSafetyProfile) int32 {
	half := int32(size / 2)
	if half <= 0 {
		return 0
	}
	inset := int32(math.Round(float64(size) * profile.insetRatio))
	if minInset := int32(profile.minInsetPx); inset < minInset {
		inset = minInset
	}
	if inset > half {
		inset = half
	}
	return half - inset
}

func clickAxisMaxOffset(size, radiusPx uint32, safeHalf int32, profile clickSafetyProfile) int32 {
	if safeHalf <= 0 || radiusPx == 0 {
		return 0
	}

	kindMax := int32(math.Round(float64(size) * profile.offsetRatio))
	if kindMax < 1 {
		kindMax = 1
	}

	radiusMax := int32(radiusPx)
	var wanted int32
	if profile.allowWide {
		wanted = maxInt32(radiusMax, kindMax)
	} else {
		wanted = minInt32(radiusMax, kindMax)
	}
	return minInt32(wanted, safeHalf)
}

func newClickRand(centerX, centerY int32, width, height uint32, elementType ClickElementType, config *HumanizationConfig) *rand.Rand {
	seed := uint64(0x9e3779b97f4a7c15) ^ config.Click.Seed
	seed = mixClickSeed(seed, uint64(uint32(centerX)))
	seed = mixClickSeed(seed, uint64(uint32(centerY)))
	seed = mixClickSeed(seed, uint64(width))
	seed = mixClickSeed(seed, uint64(height))
	seed = mixClickSeed(seed, uint64(config.Click.RadiusPx))
	if config.Click.BiasDirection != nil {
		seed = mixClickSeed(seed, uint64(*config.Click.BiasDirection)+1)
	}
	for _, b := range []byte(elementType) {
		seed = mixClickSeed(seed, uint64(b))
	}
	return rand.New(rand.NewPCG(seed, splitMix64(seed^0xd1b54a32d192ed03)))
}

func mixClickSeed(seed, value uint64) uint64 {
	return splitMix64(seed ^ (value + 0x9e3779b97f4a7c15 + (seed << 6) + (seed >> 2)))
}

func splitMix64(v uint64) uint64 {
	v += 0x9e3779b97f4a7c15
	v = (v ^ (v >> 30)) * 0xbf58476d1ce4e5b9
	v = (v ^ (v >> 27)) * 0x94d049bb133111eb
	return v ^ (v >> 31)
}

func sampleSignedAxis(rng *rand.Rand, maxOffset int32) int32 {
	if maxOffset <= 0 {
		return 0
	}
	return int32(rng.Int64N(int64(maxOffset)*2+1)) - maxOffset
}

func applyClickBias(dx, dy int32, bias *BiasDirection) (int32, int32) {
	if bias == nil {
		return dx, dy
	}
	switch *bias {
	case BiasTopLeft:
		return -absInt32(dx), -absInt32(dy)
	case BiasTopRight:
		return absInt32(dx), -absInt32(dy)
	case BiasBottomLeft:
		return -absInt32(dx), absInt32(dy)
	case BiasBottomRight:
		return absInt32(dx), absInt32(dy)
	default:
		return dx, dy
	}
}

func clampInt32(v, lo, hi int32) int32 {
	if lo > hi {
		return v
	}
	if v < lo {
		return lo
	}
	if v > hi {
		return hi
	}
	return v
}

func minInt32(a, b int32) int32 {
	if a < b {
		return a
	}
	return b
}

func maxInt32(a, b int32) int32 {
	if a > b {
		return a
	}
	return b
}

// computeOffsetVector returns (dx, dy) within the given radius using polar coordinates.
// Directional bias constrains the angle to a specific quadrant.
func computeOffsetVector(radiusPx uint32, bias *BiasDirection) (int32, int32) {
	seed := mixClickSeed(uint64(0x517cc1b727220a95), uint64(radiusPx))
	if bias != nil {
		seed = mixClickSeed(seed, uint64(*bias)+1)
	}
	rng := rand.New(rand.NewPCG(seed, splitMix64(seed^0x94d049bb133111eb)))
	radius := rng.Uint32N(radiusPx + 1)
	angleRad := randomAngleFromRand(rng, bias)

	dx := int32(float64(radius) * math.Cos(angleRad))
	dy := int32(float64(radius) * math.Sin(angleRad))
	return dx, dy
}

// randomAngle returns a random angle in radians, optionally biased to a quadrant.
// Uses screen coordinates: Y increases downward.
// Angles in degrees with mathematical convention: 0=right, 90=up, 180=left, 270=down.
func randomAngle(bias *BiasDirection) float64 {
	seed := uint64(0x6a09e667f3bcc909)
	if bias != nil {
		seed = mixClickSeed(seed, uint64(*bias)+1)
	}
	return randomAngleFromRand(rand.New(rand.NewPCG(seed, splitMix64(seed))), bias)
}

func randomAngleFromRand(rng *rand.Rand, bias *BiasDirection) float64 {
	var degrees float64
	if bias == nil {
		degrees = rng.Float64() * 360.0
	} else {
		switch *bias {
		case BiasTopLeft:
			degrees = randRangeFloat64FromRand(rng, 135.0, 225.0)
		case BiasTopRight:
			degrees = randRangeFloat64FromRand(rng, 225.0, 315.0)
		case BiasBottomLeft:
			degrees = randRangeFloat64FromRand(rng, 45.0, 135.0)
		case BiasBottomRight:
			// Match original Rust behavior: max of two independent samples from disjoint ranges
			a := randRangeFloat64FromRand(rng, 315.0, 360.0)
			b := randRangeFloat64FromRand(rng, 0.0, 45.0)
			degrees = math.Max(a, b)
		case BiasCenter:
			degrees = rng.Float64() * 360.0
		}
	}
	return degrees * math.Pi / 180.0
}

func randRangeFloat64FromRand(rng *rand.Rand, min, max float64) float64 {
	return min + rng.Float64()*(max-min)
}

func easeInOutCubic(t float64) float64 {
	if t < 0.5 {
		return 4 * t * t * t
	}
	return 1 - math.Pow(-2*t+2, 3)/2
}

func pinkNoise(rng *rand.Rand, octaves int) float64 {
	if octaves <= 0 {
		return 0
	}
	var sum float64
	var weight float64
	for i := 0; i < octaves; i++ {
		w := 1.0 / float64(i+1)
		sum += (rng.Float64()*2 - 1) * w
		weight += w
	}
	return sum / weight
}

func normalLike(rng *rand.Rand) float64 {
	var sum float64
	for i := 0; i < 6; i++ {
		sum += rng.Float64()
	}
	return sum - 3
}

func clampPointerUint32(v, lo, hi uint32) uint32 {
	if v < lo {
		return lo
	}
	if v > hi {
		return hi
	}
	return v
}
