package scheduler

import (
	"ant-chrome/backend/internal/logger"
	"context"
	"sync"
	"time"
)

// EmitFn is the function signature for emitting events (matches runtime.EventsEmit usage).
type EmitFn func(eventName string, data ...interface{})

// TaskRunner executes a task's actions. Implementations can use CDP, shell, etc.
type TaskRunner interface {
	Run(task *TaskDef) error
}

// Scheduler manages task scheduling with cron/interval/event triggers.
type Scheduler struct {
	store    TaskStore
	runner   TaskRunner
	emitFn   EmitFn
	log      *logger.Logger

	ctx       context.Context
	cancel    context.CancelFunc
	wg        sync.WaitGroup
	started   bool
	mu        sync.Mutex

	// Event channel for event-triggered tasks
	eventCh chan string

	// Track currently running tasks
	running map[string]bool
	runningMu sync.Mutex
}

// New creates a new Scheduler.
func New(store TaskStore, runner TaskRunner, emitFn EmitFn) *Scheduler {
	return &Scheduler{
		store:   store,
		runner:  runner,
		emitFn:  emitFn,
		log:     logger.New("Scheduler"),
		eventCh: make(chan string, 256),
		running: make(map[string]bool),
	}
}

// Start begins the scheduler loop. Call once.
func (s *Scheduler) Start(ctx context.Context) {
	s.mu.Lock()
	if s.started {
		s.mu.Unlock()
		return
	}
	s.started = true
	s.ctx, s.cancel = context.WithCancel(ctx)
	s.mu.Unlock()

	s.log.Info("任务调度器已启动")
	s.wg.Add(2)
	go s.cronLoop()
	go s.eventLoop()
}

// Stop gracefully shuts down the scheduler.
func (s *Scheduler) Stop() {
	s.mu.Lock()
	if !s.started {
		s.mu.Unlock()
		return
	}
	s.cancel()
	s.started = false
	s.mu.Unlock()

	close(s.eventCh)
	s.wg.Wait()
	s.log.Info("任务调度器已停止")
}

// AddTask registers a new task and starts tracking it.
func (s *Scheduler) AddTask(task *TaskDef) error {
	if task.CreatedAt.IsZero() {
		task.CreatedAt = time.Now()
	}
	if err := s.store.Save(task); err != nil {
		return err
	}
	s.emit("automation:task:created", map[string]interface{}{
		"taskId":      task.ID,
		"taskName":    task.Name,
		"triggerType": task.Trigger.Type,
		"profileId":   task.ProfileID,
	})
	return nil
}

// RemoveTask deletes a task by ID.
func (s *Scheduler) RemoveTask(id string) error {
	return s.store.Delete(id)
}

// ListTasks returns all registered tasks.
func (s *Scheduler) ListTasks() ([]*TaskDef, error) {
	return s.store.List()
}

// GetTask returns a task by ID.
func (s *Scheduler) GetTask(id string) (*TaskDef, error) {
	return s.store.Get(id)
}

// RunTaskNow triggers immediate execution of a task regardless of its trigger.
func (s *Scheduler) RunTaskNow(id string) {
	task, err := s.store.Get(id)
	if err != nil || task == nil {
		return
	}
	s.executeTask(task)
}

// TriggerEvent injects an event for event-triggered tasks.
func (s *Scheduler) TriggerEvent(eventName string) {
	select {
	case s.eventCh <- eventName:
	default:
		// channel full, drop
	}
}

func (s *Scheduler) cronLoop() {
	defer s.wg.Done()
	ticker := time.NewTicker(10 * time.Second)
	defer ticker.Stop()

	for {
		select {
		case <-s.ctx.Done():
			return
		case <-ticker.C:
			s.checkCronTasks()
		}
	}
}

func (s *Scheduler) eventLoop() {
	defer s.wg.Done()
	for {
		select {
		case <-s.ctx.Done():
			return
		case eventName, ok := <-s.eventCh:
			if !ok {
				return
			}
			s.checkEventTasks(eventName)
		}
	}
}

func (s *Scheduler) checkCronTasks() {
	tasks, err := s.store.List()
	if err != nil {
		s.log.Error("获取任务列表失败", logger.F("error", err))
		return
	}

	now := time.Now()
	for _, task := range tasks {
		if !task.Enabled {
			continue
		}
		if task.Trigger.Type == TriggerEvent {
			continue // handled by event loop
		}
		if task.Status() == StatusRunning {
			continue
		}
		if !s.canRun(task) {
			continue
		}

		switch task.Trigger.Type {
		case TriggerInterval:
			interval, err := ParseInterval(task.Trigger.Interval)
			if err != nil || interval <= 0 {
				continue
			}
			lastRun := task.LastRunAt()
			if !lastRun.IsZero() && now.Sub(lastRun) < interval {
				continue
			}
			s.executeTask(task)

		case TriggerCron:
			if s.cronMatches(task.Trigger.Cron, now, task.LastRunAt()) {
				s.executeTask(task)
			}
		}
	}
}

func (s *Scheduler) checkEventTasks(eventName string) {
	tasks, err := s.store.List()
	if err != nil {
		return
	}

	for _, task := range tasks {
		if !task.Enabled {
			continue
		}
		if task.Trigger.Type != TriggerEvent {
			continue
		}
		if task.Trigger.Event != eventName {
			continue
		}
		if task.Status() == StatusRunning {
			continue
		}
		if !s.canRun(task) {
			continue
		}
		s.executeTask(task)
	}
}

// canRun checks if the task's dependencies are satisfied.
func (s *Scheduler) canRun(task *TaskDef) bool {
	for _, depID := range task.DependsOn {
		dep, err := s.store.Get(depID)
		if err != nil || dep == nil {
			return false
		}
		if dep.Status() != StatusDone {
			return false
		}
	}
	return true
}

func (s *Scheduler) executeTask(task *TaskDef) {
	s.runningMu.Lock()
	if s.running[task.ID] {
		s.runningMu.Unlock()
		return
	}
	s.running[task.ID] = true
	s.runningMu.Unlock()

	defer func() {
		s.runningMu.Lock()
		delete(s.running, task.ID)
		s.runningMu.Unlock()
	}()

	task.SetStatus(StatusRunning)

	s.emit("automation:task:started", map[string]interface{}{
		"taskId":    task.ID,
		"taskName":  task.Name,
		"profileId": task.ProfileID,
	})

	startTime := time.Now()

	// Execute via runner
	err := s.runner.Run(task)

	task.mu.Lock()
	task.lastRunAt = time.Now()
	duration := time.Since(startTime)
	task.mu.Unlock()

	if err != nil {
		task.mu.Lock()
		task.lastError = err.Error()
		task.retryCount++
		task.mu.Unlock()

		s.log.Warn("任务执行失败",
			logger.F("task_id", task.ID),
			logger.F("error", err),
			logger.F("retry", task.retryCount),
		)

		if task.MaxRetries > 0 && task.retryCount < task.MaxRetries {
			delay, _ := ParseInterval(task.RetryDelay)
			if delay <= 0 {
				delay = 10 * time.Second
			}
			time.AfterFunc(delay, func() {
				task.SetStatus(StatusIdle)
			})
			s.emit("automation:task:retried", map[string]interface{}{
				"taskId":     task.ID,
				"attempt":    task.retryCount,
				"maxRetries": task.MaxRetries,
			})
		} else {
			task.SetStatus(StatusFailed)
			s.emit("automation:task:failed", map[string]interface{}{
				"taskId": task.ID,
				"error":  err.Error(),
			})
		}
		return
	}

	task.SetStatus(StatusDone)

	s.emit("automation:task:completed", map[string]interface{}{
		"taskId":   task.ID,
		"duration": duration.Milliseconds(),
	})

	s.log.Info("任务执行完成",
		logger.F("task_id", task.ID),
		logger.F("duration_ms", duration.Milliseconds()),
	)
}

// cronMatches checks if a simple cron expression matches the current time.
// Supported: "*/5m" = every 5 min, "daily@09:00", "hourly" = every hour.
func (s *Scheduler) cronMatches(cron string, now time.Time, lastRun time.Time) bool {
	if cron == "" {
		return false
	}

	// Simple patterns
	switch {
	case cron == "@hourly" || cron == "hourly":
		return lastRun.IsZero() || now.Sub(lastRun) >= time.Hour
	case cron == "@daily" || cron == "daily":
		return lastRun.IsZero() || now.Sub(lastRun) >= 24*time.Hour
	case cron == "@weekly" || cron == "weekly":
		return lastRun.IsZero() || now.Sub(lastRun) >= 7*24*time.Hour
	}

	// "*/5m" pattern
	if len(cron) > 3 && cron[0:2] == "*/" {
		interval, err := ParseInterval(cron[2:])
		if err == nil && interval > 0 {
			return lastRun.IsZero() || now.Sub(lastRun) >= interval
		}
	}

	// "daily@09:00" pattern
	if len(cron) > 6 && cron[:6] == "daily@" {
		target := cron[6:]
		if len(target) == 5 && target[2] == ':' {
			currentHM := now.Format("15:04")
			if currentHM == target {
				lastHM := lastRun.Format("15:04")
				return lastHM != target
			}
		}
	}

	return false
}

func (s *Scheduler) emit(eventName string, data map[string]interface{}) {
	if s.emitFn == nil {
		return
	}
	s.emitFn(eventName, data)
}

// NoopRunner is a task runner that does nothing (for testing).
type NoopRunner struct{}

func (r *NoopRunner) Run(task *TaskDef) error {
	return nil
}
