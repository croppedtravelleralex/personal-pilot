package scheduler

import (
	"fmt"
	"sync"
	"testing"
	"time"
)

// ─── MemoryStore Tests ──────────────────────────────────────────────────────

func TestMemoryStore_ListEmpty(t *testing.T) {
	s := NewMemoryStore()
	tasks, err := s.List()
	if err != nil {
		t.Fatalf("List() 返回错误: %v", err)
	}
	if len(tasks) != 0 {
		t.Fatalf("期望空列表，得到 %d 个任务", len(tasks))
	}
}

func TestMemoryStore_SaveAndGet(t *testing.T) {
	s := NewMemoryStore()
	task := &TaskDef{
		ID:   "test-1",
		Name: "测试任务",
		Trigger: TaskTrigger{
			Type:     TriggerInterval,
			Interval: "5m",
		},
	}

	if err := s.Save(task); err != nil {
		t.Fatalf("Save() 返回错误: %v", err)
	}

	got, err := s.Get("test-1")
	if err != nil {
		t.Fatalf("Get() 返回错误: %v", err)
	}
	if got == nil {
		t.Fatal("Get() 返回 nil，期望找到任务")
	}
	if got.Name != "测试任务" {
		t.Fatalf("Name = %q, 期望 %q", got.Name, "测试任务")
	}
}

func TestMemoryStore_SaveUpdateTimestamp(t *testing.T) {
	s := NewMemoryStore()
	task := &TaskDef{ID: "t1", Name: "original"}
	if err := s.Save(task); err != nil {
		t.Fatalf("首次 Save 错误: %v", err)
	}
	originalUpdated := task.UpdatedAt

	time.Sleep(2 * time.Millisecond)
	task.Name = "updated"
	if err := s.Save(task); err != nil {
		t.Fatalf("第二次 Save 错误: %v", err)
	}
	if !task.UpdatedAt.After(originalUpdated) {
		t.Fatal("Save 后 UpdatedAt 应该更新")
	}
}

func TestMemoryStore_Delete(t *testing.T) {
	s := NewMemoryStore()
	if err := s.Save(&TaskDef{ID: "d1", Name: "待删除"}); err != nil {
		t.Fatalf("Save 错误: %v", err)
	}
	if err := s.Delete("d1"); err != nil {
		t.Fatalf("Delete 错误: %v", err)
	}
	got, err := s.Get("d1")
	if err != nil {
		t.Fatalf("Get 错误: %v", err)
	}
	if got != nil {
		t.Fatal("删除后 Get 应该返回 nil")
	}
}

func TestMemoryStore_GetNotFound(t *testing.T) {
	s := NewMemoryStore()
	got, err := s.Get("nonexistent")
	if err != nil {
		t.Fatalf("Get 错误: %v", err)
	}
	if got != nil {
		t.Fatal("不存在的任务应该返回 nil")
	}
}

// ─── ParseInterval Tests ────────────────────────────────────────────────────

func TestParseInterval_Valid(t *testing.T) {
	tests := []struct {
		input string
		want  time.Duration
	}{
		{"30s", 30 * time.Second},
		{"5m", 5 * time.Minute},
		{"1h", time.Hour},
		{"100ms", 100 * time.Millisecond},
	}
	for _, tc := range tests {
		d, err := ParseInterval(tc.input)
		if err != nil {
			t.Fatalf("ParseInterval(%q) 错误: %v", tc.input, err)
		}
		if d != tc.want {
			t.Fatalf("ParseInterval(%q) = %v, 期望 %v", tc.input, d, tc.want)
		}
	}
}

func TestParseInterval_Empty(t *testing.T) {
	d, err := ParseInterval("")
	if err != nil {
		t.Fatalf("ParseInterval(\"\") 错误: %v", err)
	}
	if d != 0 {
		t.Fatalf("期望 0，得到 %v", d)
	}
}

func TestParseInterval_Invalid(t *testing.T) {
	_, err := ParseInterval("abc")
	if err == nil {
		t.Fatal("期望错误，但没有返回")
	}
}

// ─── Scheduler Core Tests ───────────────────────────────────────────────────

func TestScheduler_StartStop(t *testing.T) {
	s := New(NewMemoryStore(), &NoopRunner{}, nil)
	s.Start(t.Context())
	// Should not panic when stopping
	s.Stop()
	// Double stop should be safe
	s.Stop()
}

func TestScheduler_DoubleStartIsSafe(t *testing.T) {
	s := New(NewMemoryStore(), &NoopRunner{}, nil)
	s.Start(t.Context())
	s.Start(t.Context()) // second start should be no-op
	s.Stop()
}

func TestScheduler_AddTask(t *testing.T) {
	s := New(NewMemoryStore(), &NoopRunner{}, nil)

	task := &TaskDef{
		ID:   "add-1",
		Name: "新增任务",
		Trigger: TaskTrigger{
			Type:     TriggerInterval,
			Interval: "5m",
		},
		Actions:    []TaskAction{},
		MaxRetries: 1,
		Enabled:    true,
	}

	if err := s.AddTask(task); err != nil {
		t.Fatalf("AddTask 错误: %v", err)
	}

	got, err := s.GetTask("add-1")
	if err != nil {
		t.Fatalf("GetTask 错误: %v", err)
	}
	if got == nil {
		t.Fatal("AddTask 后任务应该存在")
	}
	if got.Name != "新增任务" {
		t.Fatalf("Name = %q, 期望 %q", got.Name, "新增任务")
	}
}

func TestScheduler_RemoveTask(t *testing.T) {
	s := New(NewMemoryStore(), &NoopRunner{}, nil)
	if err := s.AddTask(&TaskDef{ID: "rm-1", Name: "待删除"}); err != nil {
		t.Fatalf("AddTask 错误: %v", err)
	}
	if err := s.RemoveTask("rm-1"); err != nil {
		t.Fatalf("RemoveTask 错误: %v", err)
	}
	got, _ := s.GetTask("rm-1")
	if got != nil {
		t.Fatal("RemoveTask 后任务应该已被删除")
	}
}

func TestScheduler_ListTasks(t *testing.T) {
	s := New(NewMemoryStore(), &NoopRunner{}, nil)
	s.AddTask(&TaskDef{ID: "l1", Name: "任务1"})
	s.AddTask(&TaskDef{ID: "l2", Name: "任务2"})
	s.AddTask(&TaskDef{ID: "l3", Name: "任务3"})

	tasks, err := s.ListTasks()
	if err != nil {
		t.Fatalf("ListTasks 错误: %v", err)
	}
	if len(tasks) != 3 {
		t.Fatalf("期望 3 个任务，得到 %d", len(tasks))
	}
}

// ─── Cron Match Tests ───────────────────────────────────────────────────────

func TestCronMatches_EmptyReturnsFalse(t *testing.T) {
	s := New(NewMemoryStore(), &NoopRunner{}, nil)
	now := time.Date(2026, 4, 27, 10, 0, 0, 0, time.UTC)
	lastRun := time.Time{}

	if s.cronMatches("", now, lastRun) {
		t.Fatal("空 cron 应该返回 false")
	}
}

func TestCronMatches_Every5Min(t *testing.T) {
	s := New(NewMemoryStore(), &NoopRunner{}, nil)
	now := time.Date(2026, 4, 27, 10, 0, 0, 0, time.UTC)

	// First run - should match (lastRun is zero)
	if !s.cronMatches("*/5m", now, time.Time{}) {
		t.Fatal("lastRun 为零时 */5m 应该匹配")
	}

	// 4 minutes later - should NOT match
	now4 := time.Date(2026, 4, 27, 10, 4, 0, 0, time.UTC)
	lastRun := time.Date(2026, 4, 27, 10, 0, 0, 0, time.UTC)
	if s.cronMatches("*/5m", now4, lastRun) {
		t.Fatal("4分钟后 */5m 不应该匹配")
	}

	// 5 minutes later - should match
	now5 := time.Date(2026, 4, 27, 10, 5, 0, 0, time.UTC)
	if !s.cronMatches("*/5m", now5, lastRun) {
		t.Fatal("5分钟后 */5m 应该匹配")
	}
}

func TestCronMatches_Hourly(t *testing.T) {
	s := New(NewMemoryStore(), &NoopRunner{}, nil)
	now := time.Date(2026, 4, 27, 10, 0, 0, 0, time.UTC)

	// First run
	if !s.cronMatches("@hourly", now, time.Time{}) {
		t.Fatal("lastRun 为零时 @hourly 应该匹配")
	}
	if !s.cronMatches("hourly", now, time.Time{}) {
		t.Fatal("lastRun 为零时 hourly 应该匹配")
	}

	// 30 minutes later
	now30 := time.Date(2026, 4, 27, 10, 30, 0, 0, time.UTC)
	lastRun := time.Date(2026, 4, 27, 10, 0, 0, 0, time.UTC)
	if s.cronMatches("@hourly", now30, lastRun) {
		t.Fatal("30分钟后 @hourly 不应该匹配")
	}

	// 1 hour later
	now1h := time.Date(2026, 4, 27, 11, 0, 0, 0, time.UTC)
	if !s.cronMatches("@hourly", now1h, lastRun) {
		t.Fatal("1小时后 @hourly 应该匹配")
	}
}

func TestCronMatches_Daily(t *testing.T) {
	s := New(NewMemoryStore(), &NoopRunner{}, nil)
	now := time.Date(2026, 4, 27, 10, 0, 0, 0, time.UTC)

	// First run
	if !s.cronMatches("@daily", now, time.Time{}) {
		t.Fatal("lastRun 为零时 @daily 应该匹配")
	}

	// 12 hours later
	now12h := time.Date(2026, 4, 27, 22, 0, 0, 0, time.UTC)
	lastRun := time.Date(2026, 4, 27, 10, 0, 0, 0, time.UTC)
	if s.cronMatches("@daily", now12h, lastRun) {
		t.Fatal("12小时后 @daily 不应该匹配")
	}

	// Next day
	nextDay := time.Date(2026, 4, 28, 10, 0, 0, 0, time.UTC)
	if !s.cronMatches("@daily", nextDay, lastRun) {
		t.Fatal("第二天 @daily 应该匹配")
	}
}

func TestCronMatches_DailyAtSpecificTime(t *testing.T) {
	s := New(NewMemoryStore(), &NoopRunner{}, nil)
	now := time.Date(2026, 4, 27, 9, 0, 0, 0, time.UTC)

	// Match at 09:00
	if !s.cronMatches("daily@09:00", now, time.Time{}) {
		t.Fatal("daily@09:00 在 09:00 应该匹配")
	}

	// Not match at different time
	now10 := time.Date(2026, 4, 27, 10, 0, 0, 0, time.UTC)
	lastRun := time.Date(2026, 4, 27, 9, 0, 0, 0, time.UTC)
	if s.cronMatches("daily@09:00", now10, lastRun) {
		t.Fatal("daily@09:00 在 10:00 不应该匹配")
	}

	// Next day same time should match
	nextDay := time.Date(2026, 4, 28, 9, 0, 0, 0, time.UTC)
	if !s.cronMatches("daily@09:00", nextDay, lastRun) {
		t.Fatal("第二天 daily@09:00 应该匹配")
	}
}

// ─── Dependency Check Tests ─────────────────────────────────────────────────

func TestCanRun_NoDependencies(t *testing.T) {
	s := New(NewMemoryStore(), &NoopRunner{}, nil)
	task := &TaskDef{ID: "no-dep", Name: "无依赖"}
	if !s.canRun(task) {
		t.Fatal("无依赖任务应该可以运行")
	}
}

func TestCanRun_DependencyNotDone(t *testing.T) {
	s := New(NewMemoryStore(), &NoopRunner{}, nil)
	dep := &TaskDef{ID: "dep-1", Name: "前置任务"}
	dep.SetStatus(StatusIdle)
	s.AddTask(dep)

	task := &TaskDef{ID: "main", Name: "主任务", DependsOn: []string{"dep-1"}}
	if s.canRun(task) {
		t.Fatal("前置任务未完成时主任务不应该可以运行")
	}
}

func TestCanRun_DependencyDone(t *testing.T) {
	s := New(NewMemoryStore(), &NoopRunner{}, nil)
	dep := &TaskDef{ID: "dep-2", Name: "前置任务"}
	dep.SetStatus(StatusDone)
	s.AddTask(dep)

	task := &TaskDef{ID: "main2", Name: "主任务", DependsOn: []string{"dep-2"}}
	if !s.canRun(task) {
		t.Fatal("前置任务完成后主任务应该可以运行")
	}
}

func TestCanRun_MultipleDependencies(t *testing.T) {
	s := New(NewMemoryStore(), &NoopRunner{}, nil)
	dep1 := &TaskDef{ID: "d1", Name: "前置1"}
	dep1.SetStatus(StatusDone)
	dep2 := &TaskDef{ID: "d2", Name: "前置2"}
	dep2.SetStatus(StatusDone)
	s.AddTask(dep1)
	s.AddTask(dep2)

	task := &TaskDef{
		ID:        "multi-dep",
		Name:      "多依赖",
		DependsOn: []string{"d1", "d2"},
	}
	if !s.canRun(task) {
		t.Fatal("所有前置完成时应该可以运行")
	}
}

// ─── Retry Logic Tests ──────────────────────────────────────────────────────

// recordingRunner captures execution attempts for verification.
type recordingRunner struct {
	mu   sync.Mutex
	runs []string // task IDs of executed tasks
	err  error    // error to return on Run
}

func (r *recordingRunner) Run(task *TaskDef) error {
	r.mu.Lock()
	r.runs = append(r.runs, task.ID)
	r.mu.Unlock()
	return r.err
}

func TestExecuteTask_Success(t *testing.T) {
	runner := &recordingRunner{}
	emitEvents := make([]string, 0, 10)
	emitFn := func(eventName string, data ...interface{}) {
		emitEvents = append(emitEvents, eventName)
	}

	s := New(NewMemoryStore(), runner, emitFn)
	task := &TaskDef{ID: "exec-ok", Name: "正常执行"}

	// Execute directly
	before := task.LastRunAt()
	s.executeTask(task)
	after := task.LastRunAt()

	if after.IsZero() || !after.After(before) {
		t.Fatal("执行后 LastRunAt 应该更新")
	}
	if task.Status() != StatusDone {
		t.Fatalf("状态 = %s, 期望 %s", task.Status(), StatusDone)
	}

	// Check events
	foundStart := false
	foundComplete := false
	for _, e := range emitEvents {
		if e == "automation:task:started" {
			foundStart = true
		}
		if e == "automation:task:completed" {
			foundComplete = true
		}
	}
	if !foundStart {
		t.Fatal("应该发射 started 事件")
	}
	if !foundComplete {
		t.Fatal("应该发射 completed 事件")
	}
}

func TestExecuteTask_FailureWithRetry(t *testing.T) {
	runner := &recordingRunner{err: fmt.Errorf("模拟错误")}
	emitEvents := make([]string, 0, 10)
	emitFn := func(eventName string, data ...interface{}) {
		emitEvents = append(emitEvents, eventName)
	}

	s := New(NewMemoryStore(), runner, emitFn)
	task := &TaskDef{
		ID:         "exec-fail",
		Name:       "失败任务",
		MaxRetries: 1,
		RetryDelay: "1s",
	}

	s.executeTask(task)

	if task.Status() != StatusFailed {
		t.Fatalf("状态 = %s, 期望 %s", task.Status(), StatusFailed)
	}
	if task.retryCount != 1 {
		t.Fatalf("retryCount = %d, 期望 3", task.retryCount)
	}
	if task.lastError == "" {
		t.Fatal("lastError 不应该为空")
	}

	foundFailed := false
	for _, e := range emitEvents {
		if e == "automation:task:failed" {
			foundFailed = true
		}
	}
	if !foundFailed {
		t.Fatal("应该发射 failed 事件")
	}
}

func TestExecuteTask_FailureNoRetry(t *testing.T) {
	runner := &recordingRunner{err: fmt.Errorf("模拟错误")}
	s := New(NewMemoryStore(), runner, nil)
	task := &TaskDef{
		ID:         "no-retry",
		Name:       "不重试",
		MaxRetries: 0,
	}

	s.executeTask(task)
	if task.Status() != StatusFailed {
		t.Fatalf("状态 = %s, 期望 %s", task.Status(), StatusFailed)
	}
}

// ─── Event-Driven Task Tests ────────────────────────────────────────────────

func TestTriggerEvent(t *testing.T) {
	runner := &recordingRunner{}
	s := New(NewMemoryStore(), runner, nil)
	s.Start(t.Context())
	defer s.Stop()

	s.AddTask(&TaskDef{
		ID:      "evt-1",
		Name:    "事件任务",
		Enabled: true,
		Trigger: TaskTrigger{Type: TriggerEvent, Event: "test:event"},
	})

	s.TriggerEvent("test:event")
	time.Sleep(200 * time.Millisecond)

	runner.mu.Lock()
	count := len(runner.runs)
	runner.mu.Unlock()
	if count != 1 {
		t.Fatalf("期望执行 1 次，得到 %d 次", count)
	}
}

func TestTriggerEvent_UnrelatedEvent(t *testing.T) {
	runner := &recordingRunner{}
	s := New(NewMemoryStore(), runner, nil)
	s.Start(t.Context())
	defer s.Stop()

	s.AddTask(&TaskDef{
		ID:      "evt-2",
		Name:    "事件任务",
		Enabled: true,
		Trigger: TaskTrigger{Type: TriggerEvent, Event: "expected:event"},
	})

	s.TriggerEvent("unrelated:event")
	time.Sleep(200 * time.Millisecond)

	runner.mu.Lock()
	count := len(runner.runs)
	runner.mu.Unlock()
	if count != 0 {
		t.Fatalf("不相关事件不应该触发任务，执行了 %d 次", count)
	}
}

func TestTriggerEvent_DisabledTask(t *testing.T) {
	runner := &recordingRunner{}
	s := New(NewMemoryStore(), runner, nil)
	s.Start(t.Context())
	defer s.Stop()

	s.AddTask(&TaskDef{
		ID:      "evt-3",
		Name:    "已禁用",
		Enabled: false,
		Trigger: TaskTrigger{Type: TriggerEvent, Event: "test:event"},
	})

	s.TriggerEvent("test:event")
	time.Sleep(200 * time.Millisecond)

	runner.mu.Lock()
	count := len(runner.runs)
	runner.mu.Unlock()
	if count != 0 {
		t.Fatal("已禁用的任务不应该被执行")
	}
}
