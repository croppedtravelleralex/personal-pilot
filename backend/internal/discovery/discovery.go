package discovery

import "strings"

type Field struct {
	Name     string
	Kind     string
	Selector string
}
type Form struct {
	Selector string
	Fields   []Field
}
type PageKind string

const (
	PageKindForm    PageKind = "form"
	PageKindAuth    PageKind = "auth"
	PageKindGeneric PageKind = "generic"
)

func ClassifyPage(url, title string, forms []Form) PageKind {
	text := strings.ToLower(url + " " + title)
	if strings.Contains(text, "login") || strings.Contains(text, "signin") || strings.Contains(text, "register") {
		return PageKindAuth
	}
	if len(forms) > 0 {
		return PageKindForm
	}
	return PageKindGeneric
}

func InferFieldKind(name string) string {
	name = strings.ToLower(name)
	switch {
	case strings.Contains(name, "email"):
		return "email"
	case strings.Contains(name, "phone"):
		return "phone"
	case strings.Contains(name, "pass"):
		return "password"
	case strings.Contains(name, "captcha") || strings.Contains(name, "code"):
		return "verification_code"
	default:
		return "text"
	}
}
