package backend

import (
	"context"

	"github.com/wailsapp/wails/v2/pkg/runtime"
)

func quitWailsRuntime(ctx context.Context) {
	runtime.Quit(ctx)
}

func logFatalWailsRuntime(ctx context.Context, message string) {
	runtime.LogFatal(ctx, message)
}
