//go:build windows
// +build windows

package backend

import (
	"fmt"
	"syscall"
	"unsafe"
)

var (
	user32                    = syscall.NewLazyDLL("user32.dll")
	procEnumWindows           = user32.NewProc("EnumWindows")
	procGetWindow             = user32.NewProc("GetWindow")
	procGetWindowThreadProcID = user32.NewProc("GetWindowThreadProcessId")
	procIsWindowVisible       = user32.NewProc("IsWindowVisible")
	procMoveWindow            = user32.NewProc("MoveWindow")
	procSetForegroundWindow   = user32.NewProc("SetForegroundWindow")
	procShowWindow            = user32.NewProc("ShowWindow")
	procSystemParametersInfoW = user32.NewProc("SystemParametersInfoW")
)

const (
	gwOwner          = 4
	swRestore        = 9
	swShowNormal     = 1
	spiGetWorkArea   = 0x0030
	defaultWorkAreaX = 0
	defaultWorkAreaY = 0
	defaultWorkAreaW = 1280
	defaultWorkAreaH = 720
)

type winRect struct {
	left   int32
	top    int32
	right  int32
	bottom int32
}

func externalWindowWorkArea() (workbenchWindowRect, error) {
	rect := winRect{}
	ok, _, err := procSystemParametersInfoW.Call(
		uintptr(spiGetWorkArea),
		0,
		uintptr(unsafe.Pointer(&rect)),
		0,
	)
	if ok == 0 {
		if err != syscall.Errno(0) {
			return workbenchWindowRect{}, fmt.Errorf("读取 Windows 工作区失败: %w", err)
		}
		return workbenchWindowRect{}, fmt.Errorf("读取 Windows 工作区失败")
	}
	width := int(rect.right - rect.left)
	height := int(rect.bottom - rect.top)
	if width <= 0 || height <= 0 {
		return workbenchWindowRect{
			x:      defaultWorkAreaX,
			y:      defaultWorkAreaY,
			width:  defaultWorkAreaW,
			height: defaultWorkAreaH,
		}, nil
	}
	return workbenchWindowRect{
		x:      int(rect.left),
		y:      int(rect.top),
		width:  width,
		height: height,
	}, nil
}

func activateExternalWindowByPID(pid int) error {
	hwnd, err := findExternalWindowByPID(pid)
	if err != nil {
		return err
	}
	procShowWindow.Call(hwnd, uintptr(swRestore))
	ok, _, callErr := procSetForegroundWindow.Call(hwnd)
	if ok == 0 {
		if callErr != syscall.Errno(0) {
			return fmt.Errorf("激活窗口失败: %w", callErr)
		}
		return fmt.Errorf("Windows 阻止了窗口置前")
	}
	return nil
}

func moveExternalWindowByPID(pid int, rect workbenchWindowRect) (bool, error) {
	hwnd, err := findExternalWindowByPID(pid)
	if err != nil {
		return false, err
	}
	procShowWindow.Call(hwnd, uintptr(swShowNormal))
	ok, _, callErr := procMoveWindow.Call(
		hwnd,
		uintptr(rect.x),
		uintptr(rect.y),
		uintptr(rect.width),
		uintptr(rect.height),
		1,
	)
	if ok == 0 {
		if callErr != syscall.Errno(0) {
			return true, fmt.Errorf("移动窗口失败: %w", callErr)
		}
		return true, fmt.Errorf("移动窗口失败")
	}
	return true, nil
}

func findExternalWindowByPID(pid int) (uintptr, error) {
	if pid <= 0 {
		return 0, fmt.Errorf("进程 PID 无效")
	}

	var found uintptr
	callback := syscall.NewCallback(func(hwnd uintptr, _ uintptr) uintptr {
		if found != 0 {
			return 0
		}

		visible, _, _ := procIsWindowVisible.Call(hwnd)
		if visible == 0 {
			return 1
		}
		owner, _, _ := procGetWindow.Call(hwnd, uintptr(gwOwner))
		if owner != 0 {
			return 1
		}

		var windowPID uint32
		procGetWindowThreadProcID.Call(hwnd, uintptr(unsafe.Pointer(&windowPID)))
		if int(windowPID) != pid {
			return 1
		}

		found = hwnd
		return 0
	})

	procEnumWindows.Call(callback, 0)
	if found == 0 {
		return 0, fmt.Errorf("未找到 PID %d 对应的可见顶层窗口", pid)
	}
	return found, nil
}
