//go:build !windows
// +build !windows

package backend

import "fmt"

func externalWindowWorkArea() (workbenchWindowRect, error) {
	return workbenchWindowRect{}, fmt.Errorf("外部窗口控制仅支持 Windows")
}

func activateExternalWindowByPID(pid int) error {
	return fmt.Errorf("外部窗口控制仅支持 Windows")
}

func moveExternalWindowByPID(pid int, rect workbenchWindowRect) (bool, error) {
	return false, fmt.Errorf("外部窗口控制仅支持 Windows")
}
