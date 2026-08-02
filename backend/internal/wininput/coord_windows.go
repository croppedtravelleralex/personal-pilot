//go:build windows

package wininput

import (
	"fmt"
	"syscall"
	"unsafe"

	"golang.org/x/sys/windows"
)

var (
	modUser32           = windows.NewLazyDLL("user32.dll")
	procClientToScreen  = modUser32.NewProc("ClientToScreen")
	procScreenToClient  = modUser32.NewProc("ScreenToClient")
	procGetWindowRect   = modUser32.NewProc("GetWindowRect")
	procGetClientRect   = modUser32.NewProc("GetClientRect")
	procEnumWindows     = modUser32.NewProc("EnumWindows")
	procGetWindowThreadProcessId = modUser32.NewProc("GetWindowThreadProcessId")
	procIsWindowVisible = modUser32.NewProc("IsWindowVisible")
	procGetWindow       = modUser32.NewProc("GetWindow")
)

// ChromeGeometry holds measured geometry of a Chrome browser window.
type ChromeGeometry struct {
	WindowLeft   int32
	WindowTop    int32
	ClientWidth  int32
	ClientHeight int32
	// ToolbarHeight is the vertical pixels from client-area top to viewport top.
	ToolbarHeight int32
	DPIScale      float64
	ViewportW     int32
	ViewportH     int32
}

// ViewportToClient converts viewport CSS coordinates to client-area coordinates
// suitable for SendMessage lParam encoding.
func (g *ChromeGeometry) ViewportToClient(viewportX, viewportY float64) (int32, int32) {
	cx := int32(viewportX * g.DPIScale)
	cy := g.ToolbarHeight + int32(viewportY*g.DPIScale)
	return cx, cy
}

// MeasureGeometry computes ChromeGeometry from a window handle and page metrics.
// viewportW/H are CSS pixels (window.innerWidth/innerHeight).
// dpiScale is window.devicePixelRatio.
func MeasureGeometry(hwnd windows.HWND, viewportW, viewportH int32, dpiScale float64) (*ChromeGeometry, error) {
	var wr rect
	ret, _, err := procGetWindowRect.Call(uintptr(hwnd), uintptr(unsafe.Pointer(&wr)))
	if ret == 0 {
		return nil, fmt.Errorf("GetWindowRect: %w", err)
	}

	var cr rect
	ret, _, err = procGetClientRect.Call(uintptr(hwnd), uintptr(unsafe.Pointer(&cr)))
	if ret == 0 {
		return nil, fmt.Errorf("GetClientRect: %w", err)
	}

	clientH := cr.bottom - cr.top
	viewportPhysicalH := int32(float64(viewportH) * dpiScale)
	tbH := clientH - viewportPhysicalH
	if tbH < 0 {
		tbH = 0
	}

	return &ChromeGeometry{
		WindowLeft:    wr.left,
		WindowTop:     wr.top,
		ClientWidth:   cr.right - cr.left,
		ClientHeight:  clientH,
		ToolbarHeight: tbH,
		DPIScale:      dpiScale,
		ViewportW:     viewportW,
		ViewportH:     viewportH,
	}, nil
}

// ClientToScreen converts client-area coordinates to screen coordinates.
func ClientToScreen(hwnd windows.HWND, x, y int32) (int32, int32, error) {
	pt := point{x: x, y: y}
	ret, _, err := procClientToScreen.Call(uintptr(hwnd), uintptr(unsafe.Pointer(&pt)))
	if ret == 0 {
		return 0, 0, fmt.Errorf("ClientToScreen: %w", err)
	}
	return pt.x, pt.y, nil
}

// FindChromeWindow finds the top-level visible Chrome window for the given process ID.
func FindChromeWindow(pid int) (windows.HWND, error) {
	if pid <= 0 {
		return 0, fmt.Errorf("invalid PID: %d", pid)
	}

	var found windows.HWND
	cb := syscall.NewCallback(func(hwnd uintptr, _ uintptr) uintptr {
		if found != 0 {
			return 0
		}
		vis, _, _ := procIsWindowVisible.Call(hwnd)
		if vis == 0 {
			return 1
		}
		owner, _, _ := procGetWindow.Call(hwnd, 4) // GW_OWNER
		if owner != 0 {
			return 1
		}
		var wpid uint32
		procGetWindowThreadProcessId.Call(hwnd, uintptr(unsafe.Pointer(&wpid)))
		if int(wpid) != pid {
			return 1
		}
		found = windows.HWND(hwnd)
		return 0
	})

	procEnumWindows.Call(cb, 0)
	if found == 0 {
		return 0, fmt.Errorf("no visible top-level window for PID %d", pid)
	}
	return found, nil
}

// ─── internal types ────────────────────────────────────────────────────────────────

type rect struct {
	left   int32
	top    int32
	right  int32
	bottom int32
}

type point struct {
	x int32
	y int32
}
