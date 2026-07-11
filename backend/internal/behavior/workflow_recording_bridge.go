package behavior

import "personal-pilot/backend/internal/workflow"

// BridgeWorkflowSteps converts a behavior recording into workflow steps.
func BridgeWorkflowSteps(rec *Recording) []workflow.Step {
	if rec == nil {
		return nil
	}
	events := make([]workflow.CDPEvent, 0, len(rec.Events))
	for _, ev := range rec.Events {
		event := workflow.CDPEvent{}
		switch ev.Type {
		case "down", "click":
			event.Method = "Input.dispatchMouseEvent.click"
			event.Selector = ev.TargetPath
		case "key", "input", "change", "paste":
			event.Method = "Input.dispatchKeyEvent"
			event.Text = ev.Text
			event.Selector = ev.TargetPath
		case "scroll":
			event.Method = "Input.dispatchMouseEvent.scroll"
		case "move":
			event.Method = "Input.dispatchMouseEvent.move"
		default:
			continue
		}
		events = append(events, event)
	}
	steps := workflow.StepsFromCDPEvents(events)
	if rec.StartURL != "" {
		steps = append([]workflow.Step{{ID: "step-000", Action: workflow.Action{Type: "browser:navigate", Params: map[string]any{"url": rec.StartURL}}}}, steps...)
	}
	return steps
}

// BridgeRecordingToWorkflowPlan builds an executable workflow plan name from recording metadata.
func BridgeRecordingToWorkflowPlan(rec *Recording) workflow.Workflow {
	steps := BridgeWorkflowSteps(rec)
	return workflow.Workflow{
		ID:    rec.ID,
		Name:  rec.Name,
		Steps: steps,
	}
}
