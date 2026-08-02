package discovery

import (
	"fmt"

	"personal-pilot/backend/internal/workflow"
)

func FlowToWorkflowTemplate(id string, flow Flow) workflow.Workflow {
	steps := []workflow.Step{{ID: "step-001", Action: workflow.Action{Type: "browser:navigate"}}}
	idx := 2
	for _, form := range flow.Forms {
		for _, field := range form.Fields {
			steps = append(steps, workflow.Step{ID: workflowStepID(idx), Action: workflow.Action{Type: "dom:type", Params: map[string]any{"selector": field.Selector, "kind": field.Kind}}})
			idx++
		}
	}
	for _, challenge := range flow.Challenges {
		steps = append(steps, workflow.Step{ID: workflowStepID(idx), Action: workflow.Action{Type: "challenge:" + challenge.Kind, Params: map[string]any{"selector": challenge.Selector, "siteKey": challenge.SiteKey}}})
		idx++
	}
	return workflow.Workflow{ID: id, Name: string(flow.PageKind), Steps: steps}
}

func workflowStepID(i int) string {
	return fmt.Sprintf("step-%03d", i)
}
