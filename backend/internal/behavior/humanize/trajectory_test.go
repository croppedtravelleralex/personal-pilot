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

func TestComputeClickTargetForElement_ControlKindsStayInConservativeSafeZone(t *testing.T) {
	kinds := []ClickElementType{
		ClickElementButton,
		ClickElementInput,
		ClickElementMenu,
		ClickElementCheckbox,
		ClickElementTab,
	}

	for i, kind := range kinds {
		t.Run(string(kind), func(t *testing.T) {
			cfg := ConfigForLevel(LevelMedium)
			cfg.Click.Seed = uint64(100 + i)

			target := ComputeClickTargetForElement(200, 300, 32, 18, kind, &cfg)
			assertWithinTargetBox(t, target, 200, 300, 32, 18)
			assertWithinOffset(t, target, 200, 300, 4, 3)
		})
	}
}

func TestComputeClickTargetForElement_TextAndIconStayNearCenter(t *testing.T) {
	kinds := []ClickElementType{
		ClickElementTextLink,
		ClickElementIconButton,
	}

	for i, kind := range kinds {
		t.Run(string(kind), func(t *testing.T) {
			cfg := ConfigForLevel(LevelHigh)
			cfg.Click.Seed = uint64(200 + i)

			target := ComputeClickTargetForElement(300, 120, 120, 24, kind, &cfg)
			assertWithinTargetBox(t, target, 300, 120, 120, 24)
			assertWithinOffset(t, target, 300, 120, 12, 3)
		})
	}
}

func TestComputeClickTargetForElement_LargeContentCanUseWiderSafeArea(t *testing.T) {
	cfg := ConfigForLevel(LevelMedium)
	const cx, cy int32 = 400, 300
	const w, h uint32 = 400, 240

	sawWideOffset := false
	for seed := uint64(1); seed <= 64; seed++ {
		cfg.Click.Seed = seed
		target := ComputeClickTargetForElement(cx, cy, w, h, ClickElementContent, &cfg)

		assertWithinTargetBox(t, target, cx, cy, w, h)
		assertWithinSafeMargin(t, target, cx, cy, w, h, 48, 29)

		dx := absInt32(target.X - cx)
		dy := absInt32(target.Y - cy)
		if dx > int32(cfg.Click.RadiusPx) || dy > int32(cfg.Click.RadiusPx) {
			sawWideOffset = true
		}
	}

	if !sawWideOffset {
		t.Fatal("large content never exceeded RadiusPx; want wider safe-area offsets")
	}
}

func TestComputeClickTargetForElement_UnknownFallbackIsConservativeAndCompatible(t *testing.T) {
	cfg := ConfigForLevel(LevelMedium)
	cfg.Click.Seed = 42

	legacy := ComputeClickTarget(200, 300, 200, 100, &cfg)
	unknown := ComputeClickTargetForElement(200, 300, 200, 100, ClickElementUnknown, &cfg)

	if legacy.X != unknown.X || legacy.Y != unknown.Y {
		t.Fatalf("legacy target = (%d,%d), unknown = (%d,%d)", legacy.X, legacy.Y, unknown.X, unknown.Y)
	}
	assertWithinTargetBox(t, unknown, 200, 300, 200, 100)
	assertWithinOffset(t, unknown, 200, 300, int32(cfg.Click.RadiusPx), int32(cfg.Click.RadiusPx))
}

func TestComputeClickTargetForElement_TinyElementNeverOutOfBounds(t *testing.T) {
	kinds := []ClickElementType{
		ClickElementCheckbox,
		ClickElementContent,
		ClickElementUnknown,
	}

	for i, kind := range kinds {
		t.Run(string(kind), func(t *testing.T) {
			cfg := ConfigForLevel(LevelHigh)
			cfg.Click.Seed = uint64(300 + i)

			target := ComputeClickTargetForElement(10, 20, 3, 2, kind, &cfg)
			assertWithinTargetBox(t, target, 10, 20, 3, 2)
		})
	}
}

func TestComputeClickTargetForElement_StableForSameSeedAndConfig(t *testing.T) {
	cfg := ConfigForLevel(LevelMedium)
	cfg.Click.Seed = 99

	first := ComputeClickTargetForElement(100, 80, 160, 90, ClickElementImage, &cfg)
	second := ComputeClickTargetForElement(100, 80, 160, 90, ClickElementImage, &cfg)

	if first.X != second.X || first.Y != second.Y {
		t.Fatalf("targets differ for same seed/config: first=(%d,%d), second=(%d,%d)",
			first.X, first.Y, second.X, second.Y)
	}
	if len(first.Trajectory) != len(second.Trajectory) {
		t.Fatalf("trajectory lengths differ: first=%d second=%d", len(first.Trajectory), len(second.Trajectory))
	}
	for i := range first.Trajectory {
		if first.Trajectory[i] != second.Trajectory[i] {
			t.Fatalf("trajectory[%d] differs: first=%v second=%v", i, first.Trajectory[i], second.Trajectory[i])
		}
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

func assertWithinTargetBox(t *testing.T, target ClickTarget, centerX, centerY int32, width, height uint32) {
	t.Helper()
	halfW := int32(width / 2)
	halfH := int32(height / 2)
	if target.X < centerX-halfW || target.X > centerX+halfW {
		t.Fatalf("X=%d outside [%d,%d]", target.X, centerX-halfW, centerX+halfW)
	}
	if target.Y < centerY-halfH || target.Y > centerY+halfH {
		t.Fatalf("Y=%d outside [%d,%d]", target.Y, centerY-halfH, centerY+halfH)
	}
}

func assertWithinSafeMargin(t *testing.T, target ClickTarget, centerX, centerY int32, width, height uint32, marginX, marginY int32) {
	t.Helper()
	halfW := int32(width / 2)
	halfH := int32(height / 2)
	if target.X < centerX-halfW+marginX || target.X > centerX+halfW-marginX {
		t.Fatalf("X=%d outside safe margin [%d,%d]", target.X, centerX-halfW+marginX, centerX+halfW-marginX)
	}
	if target.Y < centerY-halfH+marginY || target.Y > centerY+halfH-marginY {
		t.Fatalf("Y=%d outside safe margin [%d,%d]", target.Y, centerY-halfH+marginY, centerY+halfH-marginY)
	}
}

func assertWithinOffset(t *testing.T, target ClickTarget, centerX, centerY int32, maxDX, maxDY int32) {
	t.Helper()
	if dx := absInt32(target.X - centerX); dx > maxDX {
		t.Fatalf("dx=%d, want <= %d", dx, maxDX)
	}
	if dy := absInt32(target.Y - centerY); dy > maxDY {
		t.Fatalf("dy=%d, want <= %d", dy, maxDY)
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
