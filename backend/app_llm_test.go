package backend

import (
	"strings"
	"testing"
)

func TestLlmActionCommandsUseRecordableCDPInput(t *testing.T) {
	click := buildLlmClickCommands(10.2, 20.8)
	if len(click) != 3 {
		t.Fatalf("click commands = %d, want 3", len(click))
	}
	for _, cmd := range click {
		if cmd.Method != "Input.dispatchMouseEvent" {
			t.Fatalf("click method = %s, want Input.dispatchMouseEvent", cmd.Method)
		}
	}
	if click[1].Params["type"] != "mousePressed" || click[2].Params["type"] != "mouseReleased" {
		t.Fatalf("click should dispatch press/release, got %v / %v", click[1].Params["type"], click[2].Params["type"])
	}

	scroll := buildLlmScrollCommands(100, 200, 500)
	if len(scroll) != 1 {
		t.Fatalf("scroll commands = %d, want 1", len(scroll))
	}
	if scroll[0].Method != "Input.dispatchMouseEvent" || scroll[0].Params["type"] != "mouseWheel" {
		t.Fatalf("scroll should dispatch mouseWheel, got %s %v", scroll[0].Method, scroll[0].Params["type"])
	}

	typing := buildLlmTypeKeyCommands("ab")
	if len(typing) != 6 {
		t.Fatalf("typing commands = %d, want 6", len(typing))
	}
	for _, cmd := range typing {
		if cmd.Method != "Input.dispatchKeyEvent" {
			t.Fatalf("typing method = %s, want Input.dispatchKeyEvent", cmd.Method)
		}
	}
	if typing[1].Params["type"] != "char" || typing[1].Params["text"] != "a" {
		t.Fatalf("typing char command mismatch: %#v", typing[1].Params)
	}
}

func TestLlmSensitiveTypingUsesRedactedRecordingEvents(t *testing.T) {
	targets := []llmElementInfo{
		{InputType: "password"},
		{Placeholder: "请输入验证码"},
		{Name: "otp_code"},
	}
	for _, target := range targets {
		if !isSensitiveLlmTarget(target) {
			t.Fatalf("target should be sensitive: %#v", target)
		}
	}

	secret := "s3cret-验证码"
	expr, err := buildAppendRecordedEventsExpression(buildRedactedTypingEvents(secret))
	if err != nil {
		t.Fatalf("build expression: %v", err)
	}
	if strings.Contains(expr, secret) || strings.Contains(expr, "验证码") || strings.Contains(expr, "s3cret") {
		t.Fatalf("redacted recording expression leaked secret: %s", expr)
	}
	if !strings.Contains(expr, "[redacted]") {
		t.Fatalf("redacted recording expression missing marker: %s", expr)
	}
}
