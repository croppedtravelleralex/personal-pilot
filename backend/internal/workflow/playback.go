package workflow

import "time"

type PlaybackEvent struct {
	StepID string
	Action Action
	Delay  time.Duration
}

func BuildPlaybackPlan(workflow Workflow, gap time.Duration) []PlaybackEvent {
	if gap <= 0 {
		gap = 350 * time.Millisecond
	}
	events := make([]PlaybackEvent, 0, len(workflow.Steps))
	for i, step := range workflow.Steps {
		delay := gap
		if i == 0 {
			delay = 0
		}
		events = append(events, PlaybackEvent{StepID: step.ID, Action: step.Action, Delay: delay})
	}
	return events
}

func (e Engine) ExecuteReady(workflow Workflow, state ExecutionState) ExecutionState {
	if state.StepStatus == nil {
		state = e.Plan(workflow)
	}
	if state.Errors == nil {
		state.Errors = map[string]string{}
	}
	for _, step := range e.RunnableSteps(workflow, state) {
		state.StepStatus[step.ID] = StepRunning
		if e.Executor == nil {
			state.StepStatus[step.ID] = StepSkipped
			state.Errors[step.ID] = "executor_not_configured"
			continue
		}
		vars, err := e.Executor.Execute(step.Action, state.Variables)
		if err != nil {
			state.StepStatus[step.ID] = StepFailed
			state.Errors[step.ID] = err.Error()
			continue
		}
		for key, value := range vars {
			state.Variables[key] = value
		}
		state.StepStatus[step.ID] = StepPassed
	}
	return state
}
