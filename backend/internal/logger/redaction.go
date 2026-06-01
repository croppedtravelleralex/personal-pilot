package logger

import (
	"fmt"
	"reflect"
	"regexp"
	"strings"
)

const SensitiveLogValue = "***"

var (
	sensitiveAssignmentPattern = regexp.MustCompile(`(?i)(["']?(?:password|passwd|pwd|token|access[_-]?token|refresh[_-]?token|id[_-]?token|api[_-]?key|apikey|authorization|credential|credentials|secret|client[_-]?secret|private[_-]?key|cookie|set-cookie)["']?\s*[:=]\s*)(?:"[^"]*"|'[^']*'|[^\s,;}&\]]+)`)
	authorizationHeaderPattern = regexp.MustCompile(`(?i)\b(authorization\s*[:=]\s*)(?:"[^"]*"|'[^']*'|(?:Bearer|Basic)\s+[A-Za-z0-9._~+/=-]+|[^\s,;}&\]]+)`)
	authorizationValuePattern  = regexp.MustCompile(`(?i)\b(Bearer|Basic)\s+[A-Za-z0-9._~+/=-]+`)
	urlUserInfoPattern         = regexp.MustCompile(`(?i)([a-z][a-z0-9+.-]*://)([^/\s:@]+):([^@\s/]+)@`)
)

// DefaultSensitiveFieldNames is the source-level safety contract for logs.
func DefaultSensitiveFieldNames() []string {
	return []string{
		"password",
		"passwd",
		"pwd",
		"token",
		"access_token",
		"refresh_token",
		"id_token",
		"api_key",
		"apikey",
		"authorization",
		"credential",
		"credentials",
		"secret",
		"client_secret",
		"private_key",
		"cookie",
		"cookies",
		"set_cookie",
		"set-cookie",
	}
}

// IsSensitiveLogField returns true for fields whose values must never be logged.
func IsSensitiveLogField(key string) bool {
	normalized := normalizeSensitiveKey(key)
	if normalized == "" {
		return false
	}

	for _, field := range DefaultSensitiveFieldNames() {
		if normalized == normalizeSensitiveKey(field) {
			return true
		}
	}

	return false
}

func normalizeSensitiveKey(key string) string {
	key = strings.TrimSpace(strings.ToLower(key))
	if key == "" {
		return ""
	}

	var b strings.Builder
	lastUnderscore := false
	for _, r := range key {
		if (r >= 'a' && r <= 'z') || (r >= '0' && r <= '9') {
			b.WriteRune(r)
			lastUnderscore = false
			continue
		}
		if !lastUnderscore {
			b.WriteByte('_')
			lastUnderscore = true
		}
	}

	return strings.Trim(b.String(), "_")
}

// RedactText removes common inline secret forms from free-form log strings.
func RedactText(text string) string {
	if text == "" {
		return text
	}

	redacted := authorizationHeaderPattern.ReplaceAllString(text, `${1}"`+SensitiveLogValue+`"`)
	redacted = authorizationValuePattern.ReplaceAllString(redacted, `${1} `+SensitiveLogValue)
	redacted = sensitiveAssignmentPattern.ReplaceAllString(redacted, `${1}"`+SensitiveLogValue+`"`)
	redacted = urlUserInfoPattern.ReplaceAllString(redacted, `${1}${2}:`+SensitiveLogValue+`@`)
	return redacted
}

func RedactValueForKey(key string, value interface{}) interface{} {
	if IsSensitiveLogField(key) {
		return SensitiveLogValue
	}
	return RedactValue(value)
}

func RedactValue(value interface{}) interface{} {
	if value == nil {
		return nil
	}

	switch v := value.(type) {
	case string:
		return RedactText(v)
	case error:
		return RedactText(v.Error())
	case fmt.Stringer:
		return RedactText(v.String())
	}

	return redactReflectValue(reflect.ValueOf(value))
}

func redactReflectValue(value reflect.Value) interface{} {
	if !value.IsValid() {
		return nil
	}

	if value.Kind() == reflect.Interface {
		if value.IsNil() {
			return nil
		}
		return redactReflectValue(value.Elem())
	}

	switch value.Kind() {
	case reflect.Ptr:
		if value.IsNil() {
			return nil
		}
		return redactReflectValue(value.Elem())
	case reflect.Map:
		result := make(map[string]interface{}, value.Len())
		iter := value.MapRange()
		for iter.Next() {
			key := fmt.Sprintf("%v", iter.Key().Interface())
			result[key] = RedactValueForKey(key, iter.Value().Interface())
		}
		return result
	case reflect.Slice, reflect.Array:
		result := make([]interface{}, value.Len())
		for i := 0; i < value.Len(); i++ {
			result[i] = RedactValue(value.Index(i).Interface())
		}
		return result
	case reflect.Struct:
		result := make(map[string]interface{}, value.NumField())
		valueType := value.Type()
		for i := 0; i < value.NumField(); i++ {
			field := valueType.Field(i)
			if !field.IsExported() {
				continue
			}
			key := logFieldName(field)
			result[key] = RedactValueForKey(key, value.Field(i).Interface())
		}
		return result
	default:
		return value.Interface()
	}
}

func logFieldName(field reflect.StructField) string {
	for _, tagName := range []string{"json", "yaml"} {
		tag := field.Tag.Get(tagName)
		if tag == "" {
			continue
		}
		name := strings.Split(tag, ",")[0]
		if name != "" && name != "-" {
			return name
		}
	}
	return field.Name
}

func RedactFields(fields map[string]interface{}) map[string]interface{} {
	if len(fields) == 0 {
		return nil
	}

	result := make(map[string]interface{}, len(fields))
	for key, value := range fields {
		result[key] = RedactValueForKey(key, value)
	}
	return result
}

func RedactLogEntry(entry *LogEntry) *LogEntry {
	if entry == nil {
		return nil
	}

	redacted := *entry
	redacted.Message = RedactText(entry.Message)
	redacted.Method = RedactText(entry.Method)
	redacted.Error = RedactText(entry.Error)
	redacted.Fields = RedactFields(entry.Fields)
	return &redacted
}
