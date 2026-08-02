package workflow

import (
	"testing"
	"time"
)

func TestWorkflowContract(t *testing.T) {
	wf := Workflow{ID: "wf1", Variables: map[string]any{"ready": true, "name": "Pilot"}, Steps: []Step{{ID: "a", Condition: "ready"}, {ID: "b", DependsOn: []string{"a"}, Condition: "name == 'Pilot'"}}}
	engine := Engine{}
	state := engine.Plan(wf)
	runnable := engine.RunnableSteps(wf, state)
	if len(runnable) != 1 || runnable[0].ID != "a" {
		t.Fatalf("runnable = %+v", runnable)
	}
	state.StepStatus["a"] = StepPassed
	if len(engine.RunnableSteps(wf, state)) != 1 {
		t.Fatal("dependency step should become runnable")
	}
	if ResolveVariables("hi ${name}", wf.Variables) != "hi Pilot" {
		t.Fatal("variable resolution failed")
	}
	if delay, ok := NextRetryDelay(RetryPolicy{MaxAttempts: 2, Backoff: time.Second}, 1); !ok || delay != 2*time.Second {
		t.Fatalf("retry = %v %v", delay, ok)
	}
}
