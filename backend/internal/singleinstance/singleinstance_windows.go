//go:build windows

package singleinstance

import (
	"fmt"

	"golang.org/x/sys/windows"
)

const mutexName = `Global\PersonalPilotCore_SingleInstance_v1`

var mutexHandle windows.Handle

// Acquire 确保本机仅一个 personal-pilot-core 进程。
func Acquire(_ string) (func(), error) {
	name, err := windows.UTF16PtrFromString(mutexName)
	if err != nil {
		return nil, err
	}
	h, err := windows.CreateMutex(nil, false, name)
	if err != nil {
		return nil, fmt.Errorf("create mutex: %w", err)
	}
	if err := windows.GetLastError(); err == windows.ERROR_ALREADY_EXISTS {
		windows.CloseHandle(h)
		return nil, fmt.Errorf("personal-pilot-core 已在运行，请勿重复启动")
	}
	mutexHandle = h
	return func() {
		if mutexHandle != 0 {
			_ = windows.CloseHandle(mutexHandle)
			mutexHandle = 0
		}
	}, nil
}
