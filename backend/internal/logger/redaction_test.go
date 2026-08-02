package logger

import (
	"encoding/json"
	"fmt"
	"strings"
	"testing"
	"time"
)

func TestM4SafetyLoggingContract_RedactsCredentialFields(t *testing.T) {
	mockWriter := NewMockWriter()
	log := createTestLogger(DEBUG, mockWriter)

	log.Info(
		"provider config loaded api_key=plain-api-key",
		F("password", "plain-password"),
		F("api_key", "plain-api-key"),
		F("Authorization", "Bearer plain-bearer-token"),
		F("metadata", map[string]interface{}{
			"token":  "plain-nested-token",
			"normal": "kept",
		}),
		F("error", `upstream rejected Authorization: Bearer plain-error-token`),
		F("proxy_url", "socks5://user:plain-proxy-password@example.test:1080"),
	)

	entries := mockWriter.GetEntries()
	if len(entries) != 1 {
		t.Fatalf("expected 1 entry, got %d", len(entries))
	}

	entry := entries[0]
	assertNoSecretText(t, "message", entry.Message, "plain-api-key")
	assertNoSecretText(t, "password", asString(entry.Fields["password"]), "plain-password")
	assertNoSecretText(t, "api_key", asString(entry.Fields["api_key"]), "plain-api-key")
	assertNoSecretText(t, "Authorization", asString(entry.Fields["Authorization"]), "plain-bearer-token")
	assertNoSecretText(t, "error", asString(entry.Fields["error"]), "plain-error-token")
	assertNoSecretText(t, "proxy_url", asString(entry.Fields["proxy_url"]), "plain-proxy-password")

	metadata, ok := entry.Fields["metadata"].(map[string]interface{})
	if !ok {
		t.Fatalf("metadata = %T, want map[string]interface{}", entry.Fields["metadata"])
	}
	if metadata["token"] != SensitiveLogValue {
		t.Fatalf("metadata token = %v, want %q", metadata["token"], SensitiveLogValue)
	}
	if metadata["normal"] != "kept" {
		t.Fatalf("metadata normal = %v, want kept", metadata["normal"])
	}
}

func TestM4SafetyLoggingContract_FormattersDoNotEmitSecrets(t *testing.T) {
	entry := &LogEntry{
		Timestamp: time.Date(2026, 6, 1, 10, 0, 0, 0, time.UTC),
		Level:     INFO,
		Component: "safety",
		Message:   "load failed password=plain-message-password",
		Error:     "request failed with Authorization: Bearer plain-error-token",
		Fields: map[string]interface{}{
			"password":  "plain-password",
			"body":      `{"token":"plain-json-token","nested":{"client_secret":"plain-client-secret"}}`,
			"proxy_url": "http://proxy-user:plain-proxy-password@127.0.0.1:8080",
			"normal":    "kept",
		},
	}

	textData, err := NewTextFormatter().Format(entry)
	if err != nil {
		t.Fatalf("text format failed: %v", err)
	}
	textOutput := string(textData)
	for _, secret := range []string{"plain-message-password", "plain-error-token", "plain-password", "plain-json-token", "plain-client-secret", "plain-proxy-password"} {
		assertNoSecretText(t, "text formatter", textOutput, secret)
	}
	if !strings.Contains(textOutput, SensitiveLogValue) {
		t.Fatalf("text output missing redaction marker: %s", textOutput)
	}

	jsonData, err := NewJSONFormatter().Format(entry)
	if err != nil {
		t.Fatalf("json format failed: %v", err)
	}
	for _, secret := range []string{"plain-message-password", "plain-error-token", "plain-password", "plain-json-token", "plain-client-secret", "plain-proxy-password"} {
		assertNoSecretText(t, "json formatter", string(jsonData), secret)
	}
	var parsed map[string]interface{}
	if err := json.Unmarshal(jsonData, &parsed); err != nil {
		t.Fatalf("json formatter emitted invalid JSON: %v", err)
	}
	fields, ok := parsed["fields"].(map[string]interface{})
	if !ok {
		t.Fatalf("fields = %T, want map[string]interface{}", parsed["fields"])
	}
	if fields["password"] != SensitiveLogValue {
		t.Fatalf("json password = %v, want %q", fields["password"], SensitiveLogValue)
	}
	if fields["normal"] != "kept" {
		t.Fatalf("json normal = %v, want kept", fields["normal"])
	}
}

func asString(value interface{}) string {
	if value == nil {
		return ""
	}
	return fmt.Sprint(value)
}

func assertNoSecretText(t *testing.T, label string, text string, secret string) {
	t.Helper()
	if strings.Contains(text, secret) {
		t.Fatalf("%s leaked secret %q in %q", label, secret, text)
	}
}
