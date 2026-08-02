package workflow

import "strings"

func EvaluateCondition(expr string, vars map[string]any) bool {
	expr = strings.TrimSpace(expr)
	if expr == "" || expr == "true" {
		return true
	}
	if expr == "false" {
		return false
	}
	parts := strings.Split(expr, "==")
	if len(parts) == 2 {
		left := strings.TrimSpace(parts[0])
		right := strings.Trim(strings.TrimSpace(parts[1]), "'")
		return strings.TrimSpace(toString(vars[left])) == right
	}
	value, ok := vars[expr]
	return ok && value == true
}

func toString(value any) string {
	if s, ok := value.(string); ok {
		return s
	}
	return ""
}
