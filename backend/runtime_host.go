//go:build ignore

package backend

import (
	"context"
	"log"
	"os"

	"personal-pilot/backend/internal/events"
	"personal-pilot/backend/internal/logger"
)

func (a *App) emit(eventName string, data ...interface{}) {
	if a == nil {
		return
	}
	events.EmitFrontend(a.ctx, eventName, data...)
}

func (a *App) quitHostRuntime() {
	if a == nil {
		return
	}
	os.Exit(0)
}

func (a *App) logHostFatal(ctx context.Context, message string) {
	logger.New("App").Error(message)
	log.Fatalf("FATAL: %s", message)
}
