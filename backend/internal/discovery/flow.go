package discovery

import "strings"

type Challenge struct {
	Kind     string
	Selector string
	SiteKey  string
}

type Flow struct {
	PageKind   PageKind
	Forms      []Form
	Challenges []Challenge
	Actions    []string
}

func DetectForms(inputs []Field) []Form {
	if len(inputs) == 0 {
		return nil
	}
	return []Form{{Selector: "form", Fields: inputs}}
}

func RecognizeChallenges(html string) []Challenge {
	lower := strings.ToLower(html)
	var out []Challenge
	if strings.Contains(lower, "turnstile") {
		out = append(out, Challenge{Kind: "turnstile", Selector: "[data-sitekey]"})
	}
	if strings.Contains(lower, "recaptcha") || strings.Contains(lower, "g-recaptcha") {
		out = append(out, Challenge{Kind: "recaptcha", Selector: ".g-recaptcha"})
	}
	return out
}

func InferFlow(url, title, html string, fields []Field) Flow {
	forms := DetectForms(fields)
	challenges := RecognizeChallenges(html)
	kind := ClassifyPage(url, title, forms)
	actions := []string{"browser:navigate"}
	if len(forms) > 0 {
		actions = append(actions, "form:fill")
	}
	if len(challenges) > 0 {
		actions = append(actions, "challenge:solve")
	}
	return Flow{PageKind: kind, Forms: forms, Challenges: challenges, Actions: actions}
}
