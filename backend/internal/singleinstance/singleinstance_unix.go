//go:build !windows

package singleinstance

import (
	"fmt"
	"os"
	"path/filepath"
	"strconv"
	"syscall"
)

// Acquire 使用 flock 保证单实例（非 Windows）。
func Acquire(appRoot string) (func(), error) {
	lockDir := filepath.Join(appRoot, "data", "runtime")
	if err := os.MkdirAll(lockDir, 0o755); err != nil {
		return nil, err
	}
	lockPath := filepath.Join(lockDir, "personal-pilot-core.lock")
	f, err := os.OpenFile(lockPath, os.O_CREATE|os.O_RDWR, 0o644)
	if err != nil {
		return nil, err
	}
	if err := syscall.Flock(int(f.Fd()), syscall.LOCK_EX|syscall.LOCK_NB); err != nil {
		_ = f.Close()
		return nil, fmt.Errorf("personal-pilot-core 已在运行，请勿重复启动")
	}
	_ = f.Truncate(0)
	_, _ = f.WriteString(strconv.Itoa(os.Getpid()))
	return func() {
		_ = syscall.Flock(int(f.Fd()), syscall.LOCK_UN)
		_ = f.Close()
	}, nil
}
