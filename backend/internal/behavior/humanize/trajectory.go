package humanize

import (
	"math"
	"math/rand/v2"
)

// ClickTarget describes where and with what trajectory a click should land.
type ClickTarget struct {
	X             int32      // absolute X on page
	Y             int32      // absolute Y on page
	HoverBeforeMs *uint32    // nil = no hover; value = hover duration before click
	Trajectory    [][2]int32 // waypoints: [(hoverX, hoverY), (targetX, targetY)]
}

// ComputeClickTarget computes a humanized click position within an element.
// Applies a polar-coordinate offset from the element center with optional directional bias.
func ComputeClickTarget(centerX, centerY int32, width, height uint32, config *HumanizationConfig) ClickTarget {
	if config.Click.RadiusPx == 0 {
		return ClickTarget{
			X: centerX, Y: centerY,
			HoverBeforeMs: nil,
			Trajectory:    nil,
		}
	}

	dx, dy := computeOffsetVector(config.Click.RadiusPx, config.Click.BiasDirection)
	targetX := centerX + dx
	targetY := centerY + dy

	// Hover waypoint: slightly different from final click point
	jitterRange := int32(config.Click.RadiusPx) + 5
	hoverX := targetX + int32(randRangeUint32(0, uint32(jitterRange*2))) - jitterRange
	hoverY := targetY + int32(randRangeUint32(0, uint32(jitterRange*2))) - jitterRange

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

// computeOffsetVector returns (dx, dy) within the given radius using polar coordinates.
// Directional bias constrains the angle to a specific quadrant.
func computeOffsetVector(radiusPx uint32, bias *BiasDirection) (int32, int32) {
	radius := randRangeUint32(0, radiusPx)
	angleRad := randomAngle(bias)

	dx := int32(float64(radius) * math.Cos(angleRad))
	dy := int32(float64(radius) * math.Sin(angleRad))
	return dx, dy
}

// randomAngle returns a random angle in radians, optionally biased to a quadrant.
// Uses screen coordinates: Y increases downward.
// Angles in degrees with mathematical convention: 0=right, 90=up, 180=left, 270=down.
func randomAngle(bias *BiasDirection) float64 {
	var degrees float64
	if bias == nil {
		degrees = rand.Float64() * 360.0
	} else {
		switch *bias {
		case BiasTopLeft:
			degrees = randRangeFloat64(135.0, 225.0)
		case BiasTopRight:
			degrees = randRangeFloat64(225.0, 315.0)
		case BiasBottomLeft:
			degrees = randRangeFloat64(45.0, 135.0)
		case BiasBottomRight:
			// Match original Rust behavior: max of two independent samples from disjoint ranges
			a := randRangeFloat64(315.0, 360.0)
			b := randRangeFloat64(0.0, 45.0)
			degrees = math.Max(a, b)
		case BiasCenter:
			degrees = rand.Float64() * 360.0
		}
	}
	return degrees * math.Pi / 180.0
}
