//go:build windows

package inputplane

import (
	"fmt"
	"time"

	"personal-pilot/backend/internal/behavior"
	"personal-pilot/backend/internal/behavior/humanize"
	"personal-pilot/backend/internal/wininput"

	"golang.org/x/sys/windows"
)

// OSAvailable reports whether OS-level input injection is supported on this platform.
func OSAvailable() bool { return true }

// ExecuteClickAt performs a headed OS click at viewport coordinates without a selector.
func ExecuteClickAt(executor *behavior.CDPExecutor, pid int, x, y float64) error {
	return ExecuteClickAtWithSeed(executor, pid, x, y, "")
}

// ExecuteClickAtWithSeed applies BioNoise-derived mouse trajectory from humanize seed.
func ExecuteClickAtWithSeed(executor *behavior.CDPExecutor, pid int, x, y float64, seed string) error {
	mouse, _, err := newOSMouse(executor, pid, seed)
	if err != nil {
		return err
	}
	return mouse.ClickAtWithSeed(x, y, seed)
}

func osClick(executor *behavior.CDPExecutor, pid int, selector string, offsetX, offsetY int, seed string) error {
	mouse, _, err := newOSMouse(executor, pid, seed)
	if err != nil {
		return err
	}
	x, y, err := elementPoint(executor, selector, offsetX, offsetY)
	if err != nil {
		return err
	}
	return mouse.ClickAtWithSeed(x, y, seed)
}

func osDoubleClick(executor *behavior.CDPExecutor, pid int, selector, seed string) error {
	mouse, _, err := newOSMouse(executor, pid, seed)
	if err != nil {
		return err
	}
	x, y, err := elementPoint(executor, selector, 0, 0)
	if err != nil {
		return err
	}
	if err := mouse.MoveToHumanizedWithSeed(x, y, seed); err != nil {
		return err
	}
	humanize.NaturalDelay(humanize.BioNoiseConfigFromSeed(seed + "|dblclick"))
	if err := mouse.Click(); err != nil {
		return err
	}
	time.Sleep(120 * time.Millisecond)
	return mouse.Click()
}

func osRightClick(executor *behavior.CDPExecutor, pid int, selector, seed string) error {
	mouse, _, err := newOSMouse(executor, pid, seed)
	if err != nil {
		return err
	}
	x, y, err := elementPoint(executor, selector, 0, 0)
	if err != nil {
		return err
	}
	if err := mouse.MoveToHumanizedWithSeed(x, y, seed); err != nil {
		return err
	}
	humanize.NaturalDelay(humanize.BioNoiseConfigFromSeed(seed + "|rclick"))
	return mouse.RightClick()
}

func osType(executor *behavior.CDPExecutor, pid int, selector, text string, clearFirst, submitOnEnter bool, seed string) error {
	mouse, kbd, err := newOSMouse(executor, pid, seed)
	if err != nil {
		return err
	}
	x, y, err := elementPoint(executor, selector, 0, 0)
	if err != nil {
		return err
	}
	if err := mouse.ClickAtWithSeed(x, y, seed); err != nil {
		return fmt.Errorf("focus click: %w", err)
	}
	humanize.NaturalDelay(humanize.BioNoiseConfigFromSeed(seed + "|type-focus"))

	if clearFirst {
		if err := kbd.Combo(wininput.VK_CTRL, uint16('A')); err != nil {
			return fmt.Errorf("select all: %w", err)
		}
		time.Sleep(60 * time.Millisecond)
		if err := kbd.PressKey(windows.VK_BACK); err != nil {
			return fmt.Errorf("clear field: %w", err)
		}
		time.Sleep(80 * time.Millisecond)
	}

	if text != "" {
		if err := kbd.TypeString(text); err != nil {
			return fmt.Errorf("type text: %w", err)
		}
	}
	if submitOnEnter {
		time.Sleep(100 * time.Millisecond)
		if err := kbd.PressKey(wininput.VK_ENTER); err != nil {
			return fmt.Errorf("submit enter: %w", err)
		}
	}
	return nil
}

func newOSMouse(executor *behavior.CDPExecutor, pid int, seed string) (*wininput.MouseSender, *wininput.KeyboardSender, error) {
	if pid <= 0 {
		return nil, nil, fmt.Errorf("os input: invalid pid %d", pid)
	}
	vw, vh, dpr, err := executor.GetViewportMetrics()
	if err != nil {
		return nil, nil, fmt.Errorf("viewport metrics: %w", err)
	}
	hwnd, err := wininput.FindChromeWindow(pid)
	if err != nil {
		return nil, nil, fmt.Errorf("find chrome window: %w", err)
	}
	geo, err := wininput.MeasureGeometry(hwnd, vw, vh, dpr)
	if err != nil {
		return nil, nil, fmt.Errorf("measure geometry: %w", err)
	}
	mouse := wininput.NewMouseSender(hwnd, geo.ToolbarHeight, geo.DPIScale)
	_ = seed // trajectory seed applied at MoveToHumanizedWithSeed call sites
	kbd := wininput.NewKeyboardSender(45)
	return mouse, kbd, nil
}

func elementPoint(executor *behavior.CDPExecutor, selector string, offsetX, offsetY int) (float64, float64, error) {
	if offsetX == 0 && offsetY == 0 {
		return executor.GetElementCenter(selector)
	}
	bounds, err := executor.GetElementBounds(selector)
	if err != nil {
		return 0, 0, err
	}
	return float64(bounds.X1) + float64(offsetX), float64(bounds.Y1) + float64(offsetY), nil
}
