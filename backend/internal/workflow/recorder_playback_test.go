package workflow

import (
	"errors"
	"testing"
	"time"
)

type fakeExecutor struct{ fail bool }

func (f fakeExecutor) Execute(action Action, vars map[string]any) (map[string]any, error) {
	if f.fail {
		return nil, errors.New("boom")
	}
	return map[string]any{"lastAction": action.Type}, nil
}

func TestRecorderPlaybackAndFormMapper(t *testing.T) {
	steps := StepsFromCDPEvents([]CDPEvent{{Method: "Page.navigate", URL: "https://example.test"}, {Method: "Input.type", Selector: "#email", Text: "a@example.test"}})
	if len(steps) != 2 || steps[0].Action.Type != "browser:navigate" || steps[1].ID != "step-002" {
		t.Fatalf("steps = %+v", steps)
	}
	workflow := Workflow{ID: "wf", Variables: map[string]any{}, Steps: steps}
	playback := BuildPlaybackPlan(workflow, 250*time.Millisecond)
	if len(playback) != 2 || playback[0].Delay != 0 || playback[1].Delay != 250*time.Millisecond {
		t.Fatalf("playback = %+v", playback)
	}
	state := Engine{Executor: fakeExecutor{}}.ExecuteReady(workflow, ExecutionState{})
	if state.StepStatus["step-001"] != StepPassed || state.Variables["lastAction"] == "" {
		t.Fatalf("state = %+v", state)
	}
	formSteps := BuildFormSteps(FormPlan{Fields: []FieldMapping{{Selector: "#email", Kind: "email", ValueKey: "email"}}, SubmitSelector: "button"}, map[string]string{})
	if len(formSteps) != 2 || formSteps[0].Action.Params["text"] == "" {
		t.Fatalf("formSteps = %+v", formSteps)
	}
}
