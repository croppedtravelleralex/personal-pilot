package plugin

import "personal-pilot/backend/internal/workflow"

type Template struct {
	ID          string
	Name        string
	Description string
	Steps       []workflow.Step
}

type Registry struct {
	templates map[string]Template
}

func NewRegistry() Registry {
	registry := Registry{templates: map[string]Template{}}
	for _, template := range BuiltIns() {
		registry.templates[template.ID] = template
	}
	return registry
}

func (r Registry) Get(id string) (Template, bool) {
	template, ok := r.templates[id]
	return template, ok
}

func BuiltIns() []Template {
	ids := []string{"auth:login", "auth:register", "form:contact", "provider:sms", "provider:email", "challenge:turnstile", "challenge:recaptcha"}
	out := make([]Template, 0, len(ids))
	for _, id := range ids {
		out = append(out, Template{ID: id, Name: id, Steps: []workflow.Step{{ID: id + ":step", Action: workflow.Action{Type: id}}}})
	}
	return out
}
