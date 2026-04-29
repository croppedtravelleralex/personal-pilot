package backend

import (
	"ant-chrome/backend/internal/events"
	"ant-chrome/backend/internal/logger"
	"context"
)

func (a *App) emit(eventName string, data ...interface{}) {
	if a == nil {
		return
	}
	events.EmitFrontend(a.ctx, eventName, data...)
}

func (a *App) quitHostRuntime() {
	if a == nil || a.ctx == nil || events.HasFrontendEmitter() {
		return
	}
	quitWailsRuntime(a.ctx)
}

func (a *App) logHostFatal(ctx context.Context, message string) {
	if ctx == nil || events.HasFrontendEmitter() {
		logger.New("App").Error(message)
		return
	}
	logFatalWailsRuntime(ctx, message)
}
