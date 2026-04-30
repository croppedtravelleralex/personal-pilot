package behavior

import (
	"encoding/json"
	"testing"
)

func TestDefaultExecutionPolicyDefaultsToAskEachTime(t *testing.T) {
	policy := DefaultExecutionPolicy("")

	if policy.PermissionMode != PermissionAskEachTime {
		t.Fatalf("permission mode = %q, want %q", policy.PermissionMode, PermissionAskEachTime)
	}
}

func TestDefaultExecutionPolicyKeepsHumanBoundariesAndReasonOnlyPause(t *testing.T) {
	policy := DefaultExecutionPolicy(PermissionFullAccess)

	if policy.PermissionMode != PermissionFullAccess {
		t.Fatalf("permission mode = %q, want %q", policy.PermissionMode, PermissionFullAccess)
	}
	assertHumanBoundaries(t, policy.HumanBoundaries)

	pause := policy.LowConfidencePause
	if !pause.Enabled {
		t.Fatal("low-confidence pause must be enabled")
	}
	if pause.RevealTargetScreenshot || pause.RevealCandidateElements || pause.RevealRecommendedPoint {
		t.Fatalf("low-confidence pause must hide target evidence: %#v", pause)
	}
	if len(pause.PromptFields) != 2 || pause.PromptFields[0] != "reason" || pause.PromptFields[1] != "action" {
		t.Fatalf("prompt fields = %#v, want reason/action only", pause.PromptFields)
	}
}

func TestDefaultTemplateSemanticsDisablesDetectionAndScreenshotCandidates(t *testing.T) {
	policy := DefaultExecutionPolicy(PermissionAutoReview)
	semantics := DefaultTemplateSemantics(policy)

	if semantics.SchemaVersion != 1 {
		t.Fatalf("schema version = %d, want 1", semantics.SchemaVersion)
	}
	if semantics.Source != "recording_template" || semantics.Intent != "unknown" {
		t.Fatalf("unexpected semantics source/intent: %#v", semantics)
	}
	if semantics.ThirdPartyDetection || semantics.ScreenshotCandidates {
		t.Fatalf("MVP must not enable detection or screenshot candidates: %#v", semantics)
	}
	assertHumanBoundaries(t, semantics.HumanBoundaries)
}

func TestVariationConfigDecodesExecutionPolicyEnvelope(t *testing.T) {
	payload := []byte(`{
		"intensity": 0.3,
		"executionPolicy": {
			"permissionMode": "auto_review",
			"humanBoundaries": ["captcha", "password"],
			"lowConfidencePause": {
				"enabled": true,
				"revealTargetScreenshot": false,
				"revealCandidateElements": false,
				"revealRecommendedPoint": false,
				"promptFields": ["reason", "action"]
			}
		},
		"templateSemantics": {
			"schemaVersion": 1,
			"source": "recording_template",
			"intent": "unknown",
			"thirdPartyDetection": false,
			"screenshotCandidates": false,
			"humanBoundaries": ["captcha", "password"]
		}
	}`)

	var cfg VariationConfig
	if err := json.Unmarshal(payload, &cfg); err != nil {
		t.Fatalf("unmarshal variation: %v", err)
	}
	if cfg.ExecutionPolicy == nil || cfg.ExecutionPolicy.PermissionMode != PermissionAutoReview {
		t.Fatalf("execution policy was not decoded: %#v", cfg.ExecutionPolicy)
	}
	if cfg.TemplateSemantics == nil || cfg.TemplateSemantics.ThirdPartyDetection {
		t.Fatalf("template semantics was not decoded safely: %#v", cfg.TemplateSemantics)
	}
}

func assertHumanBoundaries(t *testing.T, got []HumanBoundary) {
	t.Helper()

	want := MandatoryHumanBoundaries()
	if len(got) != len(want) {
		t.Fatalf("human boundaries = %#v, want %#v", got, want)
	}
	for i := range want {
		if got[i] != want[i] {
			t.Fatalf("human boundaries = %#v, want %#v", got, want)
		}
	}
}
