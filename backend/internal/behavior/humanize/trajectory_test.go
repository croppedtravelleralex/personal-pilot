package humanize

import "testing"

func TestComputeClickTarget_NoRadius(t *testing.T) {
	cfg := ConfigForLevel(LevelNone)
	target := ComputeClickTarget(200, 300, 50, 30, &cfg)

	if target.X != 200 || target.Y != 300 {
		t.Fatalf("Target = (%d,%d), want (200,300)", target.X, target.Y)
	}
	if target.HoverBeforeMs != nil {
		t.Fatal("HoverBeforeMs should be nil for LevelNone")
	}
}

func TestComputeClickTarget_WithRadius(t *testing.T) {
	cfg := ConfigForLevel(LevelMedium)
	const cx, cy int32 = 200, 300
	const w, h uint32 = 200, 100

	// Run multiple times to verify the offset is within bounds
	for i := 0; i < 50; i++ {
		target := ComputeClickTarget(cx, cy, w, h, &cfg)

		// Check offset from center
		dx := target.X - cx
		dy := target.Y - cy

		// Max offset is RadiusPx (8) truncated by cos/sin conversion to int
		// cos/sin ∈ [-1, 1], so dx,dy ∈ [-RadiusPx, RadiusPx]
		if dx < -int32(cfg.Click.RadiusPx) || dx > int32(cfg.Click.RadiusPx) {
			t.Fatalf("dx=%d, want [%d,%d]", dx, -int32(cfg.Click.RadiusPx), cfg.Click.RadiusPx)
		}
		if dy < -int32(cfg.Click.RadiusPx) || dy > int32(cfg.Click.RadiusPx) {
			t.Fatalf("dy=%d, want [%d,%d]", dy, -int32(cfg.Click.RadiusPx), cfg.Click.RadiusPx)
		}
	}
}

func TestComputeClickTarget_HasTrajectory(t *testing.T) {
	cfg := ConfigForLevel(LevelMedium)
	target := ComputeClickTarget(200, 300, 100, 50, &cfg)

	if len(target.Trajectory) != 2 {
		t.Fatalf("Trajectory length = %d, want 2", len(target.Trajectory))
	}
}

func TestComputeOffsetVector_WithBias(t *testing.T) {
	bias := BiasTopLeft
	for i := 0; i < 50; i++ {
		dx, dy := computeOffsetVector(20, &bias)
		// TopLeft: angle 135-225 deg. cos in [-1, -0.707], sin in [0.707, -0.707]
		// So dx should be <= 0 (left side)
		if dx > 0 {
			t.Fatalf("TopLeft bias: dx=%d should be <= 0", dx)
		}
		_ = dy
	}
}

func TestComputeOffsetVector_Center(t *testing.T) {
	bias := BiasCenter
	for i := 0; i < 50; i++ {
		dx, dy := computeOffsetVector(20, &bias)
		// Center = uniform, range should be within [-20, 20]
		if dx < -20 || dx > 20 {
			t.Fatalf("dx=%d out of range", dx)
		}
		if dy < -20 || dy > 20 {
			t.Fatalf("dy=%d out of range", dy)
		}
	}
}
