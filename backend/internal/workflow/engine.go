package workflow

type Executor interface {
	Execute(action Action, vars map[string]any) (map[string]any, error)
}

type Engine struct {
	Executor Executor
}

func (e Engine) Plan(workflow Workflow) ExecutionState {
	status := make(map[string]StepStatus, len(workflow.Steps))
	for _, step := range workflow.Steps {
		status[step.ID] = StepPending
	}
	return ExecutionState{WorkflowID: workflow.ID, StepStatus: status, Variables: cloneVars(workflow.Variables), Errors: map[string]string{}}
}

func (e Engine) RunnableSteps(workflow Workflow, state ExecutionState) []Step {
	var runnable []Step
	for _, step := range workflow.Steps {
		if state.StepStatus[step.ID] != StepPending {
			continue
		}
		ready := true
		for _, dep := range step.DependsOn {
			if state.StepStatus[dep] != StepPassed {
				ready = false
				break
			}
		}
		if ready && EvaluateCondition(step.Condition, state.Variables) {
			runnable = append(runnable, step)
		}
	}
	return runnable
}

func cloneVars(in map[string]any) map[string]any {
	out := map[string]any{}
	for key, value := range in {
		out[key] = value
	}
	return out
}
