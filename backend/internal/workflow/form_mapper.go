package workflow

import "strings"

type FieldMapping struct {
	Selector string
	Kind     string
	ValueKey string
}

type FormPlan struct {
	Fields         []FieldMapping
	SubmitSelector string
}

func BuildFormSteps(plan FormPlan, data map[string]string) []Step {
	steps := make([]Step, 0, len(plan.Fields)+1)
	for i, field := range plan.Fields {
		value := data[field.ValueKey]
		if value == "" {
			value = GenerateFieldValue(field.Kind)
		}
		steps = append(steps, Step{ID: stepID(i), Action: Action{Type: "dom:type", Params: map[string]any{"selector": field.Selector, "text": value}}})
	}
	if plan.SubmitSelector != "" {
		steps = append(steps, Step{ID: stepID(len(steps)), Action: Action{Type: "dom:click", Params: map[string]any{"selector": plan.SubmitSelector}}})
	}
	return steps
}

func GenerateFieldValue(kind string) string {
	switch strings.ToLower(kind) {
	case "name":
		return "Alex Chen"
	case "email":
		return "alex.chen@example.test"
	case "phone":
		return "+12025550123"
	case "password":
		return "Pp!2026-Example"
	case "address":
		return "100 Market Street"
	default:
		return "test"
	}
}
