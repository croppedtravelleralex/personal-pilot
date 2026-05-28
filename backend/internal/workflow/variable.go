package workflow

import "strings"

func ResolveVariables(input string, vars map[string]any) string {
	out := input
	for key, value := range vars {
		if text, ok := value.(string); ok {
			out = strings.ReplaceAll(out, "${"+key+"}", text)
		}
	}
	return out
}
