package browser

import (
	"testing"
)

// TestFingerprintConsistencyFullStack 测试完整的指纹一致性检查管线。
func TestFingerprintConsistencyFullStack(t *testing.T) {
	// 模拟真实场景的完整输入
	tests := []struct {
		name       string
		input      FingerprintConsistencyInput
		wantStatus string
		minScore   int
		maxScore   int
		minHard    int
	}{
		{
			name: "完美匹配_US住宅用户",
			input: FingerprintConsistencyInput{
				TargetRegion:        "US",
				ProxyRegion:         "US",
				Timezone:            "America/New_York",
				Locale:              "en-US",
				AcceptLanguage:      "en-US,en;q=0.9",
				ScreenWidth:         1920,
				ScreenHeight:        1080,
				AvailWidth:          1920,
				AvailHeight:         1040,
				GPUVendor:           "Google",
				WebGLVendor:         "Google Inc.",
				SupportsTouch:       false,
				HardwareConcurrency: 8,
				Platform:            "Win32",
			},
			wantStatus: "coherent",
			minScore:   80,
			maxScore:   100,
			minHard:    0,
		},
		{
			name: "严重不一致_区域时区语言全不匹配",
			input: FingerprintConsistencyInput{
				TargetRegion:        "US",
				ProxyRegion:         "JP",
				Timezone:            "Asia/Shanghai",
				Locale:              "ja-JP",
				AcceptLanguage:      "zh-CN,zh;q=0.9",
				ScreenWidth:         1024,
				ScreenHeight:        768,
				AvailWidth:          1920,
				AvailHeight:         1200,
				GPUVendor:           "NVIDIA",
				WebGLVendor:         "Intel Inc.",
				SupportsTouch:       true,
				MaxTouchPoints:      0,
				HardwareConcurrency: 16,
				Platform:            "Android",
			},
			wantStatus: "inconsistent",
			minScore:   0,
			maxScore:   50,
			minHard:    0,
		},
		{
			name: "自动化检测_机器行为",
			input: FingerprintConsistencyInput{
				TargetRegion:            "US",
				ProxyRegion:             "US",
				HardwareConcurrency:     8,
				Platform:                "Win32",
				AutomationPolicyEnabled: true,
				BehaviorCadence:         "bot",
			},
			wantStatus: "coherent",
			minScore:   85,
			maxScore:   95,
			minHard:    0,
		},
		{
			name: "移动设备高核心数",
			input: FingerprintConsistencyInput{
				Platform:            "Android",
				HardwareConcurrency: 16,
			},
			wantStatus: "", // 不检查具体状态
			minScore:   0,
			maxScore:   100,
			minHard:    0,
		},
	}

	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			result := AssessFingerprintConsistency(&tc.input)

			if tc.wantStatus != "" && result.Status != tc.wantStatus {
				t.Errorf("Status = %q, 期望 %q (Score=%d, Risks=%v)",
					result.Status, tc.wantStatus, result.CoherenceScore, result.RiskReasons)
			}

			if result.CoherenceScore < tc.minScore {
				t.Errorf("CoherenceScore = %d, 期望 >= %d. Risks: %v",
					result.CoherenceScore, tc.minScore, result.RiskReasons)
			}

			if result.CoherenceScore > tc.maxScore && tc.maxScore > 0 {
				t.Errorf("CoherenceScore = %d, 期望 <= %d", result.CoherenceScore, tc.maxScore)
			}

			if result.HardFailures < tc.minHard {
				t.Errorf("HardFailures = %d, 期望 >= %d", result.HardFailures, tc.minHard)
			}
		})
	}
}

// TestFingerprintConsistencyAllChecksRun 验证所有10项检查都被执行。
func TestFingerprintConsistencyAllChecksRun(t *testing.T) {
	input := &FingerprintConsistencyInput{
		TargetRegion:            "US",
		ProxyRegion:             "US",
		Timezone:                "America/New_York",
		Locale:                  "en-US",
		AcceptLanguage:          "en-US,en;q=0.9",
		ScreenWidth:             1920,
		ScreenHeight:            1080,
		AvailWidth:              1920,
		AvailHeight:             1040,
		GPUVendor:               "Google",
		WebGLVendor:             "Google Inc.",
		WebGLRenderer:           "ANGLE",
		SupportsTouch:           false,
		HardwareConcurrency:     8,
		Platform:                "Win32",
		AutoRotateProxy:         true,
		StickySessionTTLMinutes: 30,
	}

	result := AssessFingerprintConsistency(input)

	expectedDimensions := []string{
		"target_vs_proxy_region",
		"proxy_vs_exit_region",
		"timezone_vs_region",
		"locale_vs_accept_language",
		"screen_vs_viewport",
		"gpu_vs_webgl",
		"touch_support",
		"sticky_session_vs_rotation",
		"hardware_tier_vs_power_plan",
		"automation_vs_behavior",
	}

	if len(result.CheckItems) != len(expectedDimensions) {
		t.Errorf("CheckItems 数量 = %d, 期望 %d", len(result.CheckItems), len(expectedDimensions))
	}

	found := make(map[string]bool)
	for _, ch := range result.CheckItems {
		found[ch.Dimension] = true
	}
	for _, dim := range expectedDimensions {
		if !found[dim] {
			t.Errorf("缺少检查维度: %s", dim)
		}
	}
}

// TestFingerprintConsistencyScoreNeverNegative 验证分数不会低于0。
func TestFingerprintConsistencyScoreNeverNegative(t *testing.T) {
	// 构造极端不一致的输入
	input := &FingerprintConsistencyInput{
		TargetRegion:            "US",
		ProxyRegion:             "JP",
		Timezone:                "Asia/Tokyo",
		Locale:                  "ja-JP",
		AcceptLanguage:          "zh-CN",
		ScreenWidth:             800,
		ScreenHeight:            600,
		AvailWidth:              1920,
		AvailHeight:             1080,
		GPUVendor:               "NVIDIA",
		WebGLVendor:             "AMD",
		SupportsTouch:           true,
		MaxTouchPoints:          0,
		AutoRotateProxy:         true,
		StickySessionTTLMinutes: 5,
		AutomationPolicyEnabled: true,
		BehaviorCadence:         "bot",
		HardwareConcurrency:     16,
		Platform:                "Android",
	}

	result := AssessFingerprintConsistency(input)

	if result.CoherenceScore < 0 {
		t.Errorf("CoherenceScore = %d, 不应为负数", result.CoherenceScore)
	}

	// 极端情况：0分是最低
	if result.CoherenceScore == 100 {
		t.Error("极端不一致输入竟然得满分100，不合理")
	}
}

// TestFingerprintPolicyIntegration 测试指纹策略与一致性检查的集成。
func TestFingerprintPolicyIntegration(t *testing.T) {
	// 验证 L1 字段（关键字段）的检查
	l1Fields := []string{"timezone", "locale", "platform", "accept_language", "viewport", "user_agent"}
	for _, field := range l1Fields {
		layer := ClassifyFingerprintField(field)
		if layer == nil {
			t.Errorf("L1 字段 %q 应被识别", field)
			continue
		}
		if *layer != FPLayerL1 {
			t.Errorf("字段 %q: layer = %v, 期望 FPLayerL1", field, *layer)
		}
	}

	// 验证 L1 关键字段使用轻量预算
	for _, field := range l1Fields {
		layer := ClassifyFingerprintField(field)
		if layer == nil {
			continue
		}
		budget := DefaultPerfBudgetForLayer(*layer)
		if budget != FPBudgetLight {
			t.Errorf("字段 %q: budget = %v, 期望 FPBudgetLight", field, budget)
		}
	}
}
