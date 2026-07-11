//go:build windows

package wininput

import (
	"fmt"
	"hash/fnv"
	"math"
	"strings"
	"time"

	"golang.org/x/sys/windows"
)

const (
	wmMousemove    = 0x0200
	wmLbuttondown  = 0x0201
	wmLbuttonup    = 0x0202
	wmRbuttondown  = 0x0204
	wmRbuttonup    = 0x0205
	wmMbuttonup    = 0x0208
	wmMousewheel   = 0x020A
	wmSetfocus     = 0x0007
	mkLbutton      = 0x0001
	wheelDelta     = 120
)

// MouseSender injects mouse events into a target window via SendMessage.
// Events generated via SendMessage produce isTrusted: true in Chromium.
type MouseSender struct {
	hwnd     windows.HWND
	chromeY  int32
	dpiScale float64
	curX     int32
	curY     int32
}

// NewMouseSender creates a MouseSender targeting the given window.
// chromeY is the toolbar height in physical pixels (client area height minus viewport height).
// dpiScale is window.devicePixelRatio from the page.
func NewMouseSender(hwnd windows.HWND, chromeY int32, dpiScale float64) *MouseSender {
	return &MouseSender{
		hwnd:     hwnd,
		chromeY:  chromeY,
		dpiScale: dpiScale,
	}
}

// WindowHandle returns the underlying window handle.
func (m *MouseSender) WindowHandle() windows.HWND { return m.hwnd }

// MoveTo moves the mouse to viewport CSS coordinates (x, y) with human-like interpolation.
func (m *MouseSender) MoveTo(viewportX, viewportY float64) error {
	clientX := int32(viewportX * m.dpiScale)
	clientY := m.chromeY + int32(viewportY*m.dpiScale)

	if err := m.sendMouseMove(clientX, clientY); err != nil {
		return err
	}
	m.curX = clientX
	m.curY = clientY
	return nil
}

// MoveToHumanized moves the mouse along a Bezier trajectory with human-like timing.
func (m *MouseSender) MoveToHumanized(viewportX, viewportY float64) error {
	return m.MoveToHumanizedWithSeed(viewportX, viewportY, "")
}

// MoveToHumanizedWithSeed applies BioNoise-derived jitter from a stable humanize seed (docs/53 L5).
func (m *MouseSender) MoveToHumanizedWithSeed(viewportX, viewportY float64, seed string) error {
	targetX := int32(viewportX * m.dpiScale)
	targetY := m.chromeY + int32(viewportY*m.dpiScale)
	fromX := float64(m.curX)
	fromY := float64(m.curY)
	toX := float64(targetX)
	toY := float64(targetY)

	dist := math.Sqrt(math.Pow(toX-fromX, 2) + math.Pow(toY-fromY, 2))
	steps := int(math.Max(10, dist/15))
	if steps > 60 {
		steps = 60
	}
	if steps < 3 {
		steps = 3
	}

	// Seed-stable control-point jitter (same seed → same arc family).
	j1, j2, speedMul := bioNoiseMouseParams(seed)
	cp1x := fromX + (toX-fromX)*0.3 + j1
	cp1y := fromY - 40 + j2
	cp2x := fromX + (toX-fromX)*0.7 - j2
	cp2y := toY + 40 + j1

	for i := 0; i <= steps; i++ {
		t := float64(i) / float64(steps)
		u := 1 - t
		x := int32(u*u*u*fromX + 3*u*u*t*cp1x + 3*u*t*t*cp2x + t*t*t*toX)
		y := int32(u*u*u*fromY + 3*u*u*t*cp1y + 3*u*t*t*cp2y + t*t*t*toY)

		if err := m.sendMouseMove(x, y); err != nil {
			return err
		}

		delay := time.Duration(1000.0/400.0*(1.0+math.Sin(math.Pi*t)*0.5)*speedMul) * time.Millisecond
		time.Sleep(delay)
	}

	m.curX = targetX
	m.curY = targetY
	return nil
}

func bioNoiseMouseParams(seed string) (j1, j2, speedMul float64) {
	speedMul = 1.0
	if strings.TrimSpace(seed) == "" {
		return 0, 0, speedMul
	}
	h := fnv.New32a()
	_, _ = h.Write([]byte(seed + "|os-mouse"))
	v := h.Sum32()
	j1 = float64(int(v%61) - 30)
	j2 = float64(int((v>>8)%61) - 30)
	speedMul = 0.85 + float64((v>>16)%30)/100.0
	return j1, j2, speedMul
}

// Click performs a left mouse click at the current position.
func (m *MouseSender) Click() error {
	if err := m.sendMouseButton(wmLbuttondown, m.curX, m.curY, mkLbutton); err != nil {
		return err
	}
	time.Sleep(50 * time.Millisecond)
	return m.sendMouseButton(wmLbuttonup, m.curX, m.curY, mkLbutton)
}

// ClickAt moves to viewport coordinates and clicks.
func (m *MouseSender) ClickAt(viewportX, viewportY float64) error {
	return m.ClickAtWithSeed(viewportX, viewportY, "")
}

// ClickAtWithSeed moves with seed-stable BioNoise trajectory then clicks.
func (m *MouseSender) ClickAtWithSeed(viewportX, viewportY float64, seed string) error {
	if err := m.MoveToHumanizedWithSeed(viewportX, viewportY, seed); err != nil {
		return err
	}
	time.Sleep(100 * time.Millisecond)
	return m.Click()
}

// RightClick performs a right mouse click at the current position.
func (m *MouseSender) RightClick() error {
	if err := m.sendMouseButton(wmRbuttondown, m.curX, m.curY, 0); err != nil {
		return err
	}
	time.Sleep(50 * time.Millisecond)
	return m.sendMouseButton(wmRbuttonup, m.curX, m.curY, 0)
}

// Scroll performs a vertical scroll at the current position.
// Positive delta scrolls up, negative scrolls down.
func (m *MouseSender) Scroll(delta int) error {
	wParam := uintptr(delta * wheelDelta / 120)
	lParam := uintptr(m.curY)<<16 | uintptr(m.curX)&0xFFFF
	ret, _, err := windows.NewLazyDLL("user32.dll").NewProc("SendMessageW").Call(
		uintptr(m.hwnd), wmMousewheel, wParam, lParam,
	)
	if ret == 0 && err != nil {
		return fmt.Errorf("SendMessage WM_MOUSEWHEEL: %w", err)
	}
	return nil
}

// ScrollAt viewport coordinates scrolls by delta.
func (m *MouseSender) ScrollAt(viewportX, viewportY float64, delta int) error {
	if err := m.MoveTo(viewportX, viewportY); err != nil {
		return err
	}
	time.Sleep(40 * time.Millisecond)
	return m.Scroll(delta)
}

func (m *MouseSender) sendMouseMove(x, y int32) error {
	lParam := uintptr(y)<<16 | uintptr(x)&0xFFFF
	ret, _, err := windows.NewLazyDLL("user32.dll").NewProc("SendMessageW").Call(
		uintptr(m.hwnd), wmMousemove, 0, lParam,
	)
	if ret == 0 && err != nil {
		return fmt.Errorf("SendMessage WM_MOUSEMOVE: %w", err)
	}
	return nil
}

func (m *MouseSender) sendMouseButton(msg uint32, x, y int32, wParam uintptr) error {
	lParam := uintptr(y)<<16 | uintptr(x)&0xFFFF
	ret, _, err := windows.NewLazyDLL("user32.dll").NewProc("SendMessageW").Call(
		uintptr(m.hwnd), uintptr(msg), wParam, lParam,
	)
	if ret == 0 && err != nil {
		return fmt.Errorf("SendMessage mouse button %#x: %w", msg, err)
	}
	return nil
}
