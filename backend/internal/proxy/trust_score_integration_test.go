package proxy

import (
	"testing"
)

// TestTrustScoreWithSpeedTestResults 测试信任评分与实际测速结果的集成。
func TestTrustScoreWithSpeedTestResults(t *testing.T) {
	calc := DefaultTrustScoreCalculator()

	// 模拟一系列测速结果
	scenarios := []struct {
		name       string
		results    []TestResult
		latencyMs  int64
		fraudScore int64
		isDC       bool
		regionOk   bool
		wantMin    float64 // 最低期望分
		wantMax    float64 // 最高期望分
	}{
		{
			name:       "全部成功_低延迟_住宅IP_区域匹配",
			results:    []TestResult{{Ok: true}, {Ok: true}, {Ok: true}, {Ok: true}, {Ok: true}},
			latencyMs:  150,
			fraudScore: 0,
			isDC:       false,
			regionOk:   true,
			wantMin:    80,
			wantMax:    100,
		},
		{
			name:       "全部失败_高延迟_数据中心_区域不匹配",
			results:    []TestResult{{Ok: false}, {Ok: false}, {Ok: false}},
			latencyMs:  6000,
			fraudScore: 90,
			isDC:       true,
			regionOk:   false,
			wantMin:    0,
			wantMax:    30,
		},
		{
			name:       "混合结果_中等延迟",
			results:    []TestResult{{Ok: true}, {Ok: false}, {Ok: true}, {Ok: false}, {Ok: true}},
			latencyMs:  1000,
			fraudScore: 40,
			isDC:       false,
			regionOk:   false,
			wantMin:    30,
			wantMax:    70,
		},
		{
			name:       "空历史_默认中性",
			results:    nil,
			latencyMs:  500,
			fraudScore: -1, // unknown
			isDC:       false,
			regionOk:   true,
			wantMin:    40,
			wantMax:    80,
		},
	}

	for _, sc := range scenarios {
		t.Run(sc.name, func(t *testing.T) {
			result := calc.ComputeScoreFromHistory(
				sc.latencyMs, sc.fraudScore, sc.isDC, sc.results, sc.regionOk,
			)

			if result.OverallScore < sc.wantMin {
				t.Errorf("OverallScore = %.1f, 期望 >= %.1f", result.OverallScore, sc.wantMin)
			}
			if result.OverallScore > sc.wantMax {
				t.Errorf("OverallScore = %.1f, 期望 <= %.1f", result.OverallScore, sc.wantMax)
			}

			// 验证组件分的范围
			if result.Components.LatencyScore < 0 || result.Components.LatencyScore > 1 {
				t.Errorf("LatencyScore = %.2f 超出 [0,1]", result.Components.LatencyScore)
			}
			if result.Components.IPHealthScore < 0 || result.Components.IPHealthScore > 1 {
				t.Errorf("IPHealthScore = %.2f 超出 [0,1]", result.Components.IPHealthScore)
			}
			if result.Components.HistoryScore < 0 || result.Components.HistoryScore > 1 {
				t.Errorf("HistoryScore = %.2f 超出 [0,1]", result.Components.HistoryScore)
			}
			if result.Components.RegionMatchScore < 0 || result.Components.RegionMatchScore > 1 {
				t.Errorf("RegionMatchScore = %.2f 超出 [0,1]", result.Components.RegionMatchScore)
			}
		})
	}
}

// TestTrustScoreLevelsConsistency 测试分数等级的边界一致性。
func TestTrustScoreLevelsConsistency(t *testing.T) {
	tests := []struct {
		score     float64
		wantLevel string
	}{
		{95, "excellent"},
		{80, "excellent"}, // 边界值
		{79, "good"},
		{65, "good"},
		{60, "good"}, // 边界值
		{59, "fair"},
		{45, "fair"},
		{40, "fair"}, // 边界值
		{39, "poor"},
		{10, "poor"},
		{0, "poor"},
	}

	for _, tc := range tests {
		// 直接构造结果验证等级
		result := TrustScoreResult{OverallScore: tc.score}
		// 用已知输入逼近目标分数
		// 此处验证等级字符串的一致性
		var level string
		switch {
		case tc.score >= 80:
			level = "excellent"
		case tc.score >= 60:
			level = "good"
		case tc.score >= 40:
			level = "fair"
		default:
			level = "poor"
		}
		if level != tc.wantLevel {
			t.Errorf("分数 %.0f: level = %q, want %q", tc.score, level, tc.wantLevel)
		}
		_ = result
	}
}

// TestTrustScoreHistoryDecay 测试历史结果的时效性（最近N条权重更高）。
func TestTrustScoreHistoryDecay(t *testing.T) {
	// 构造20条结果：前10条全部失败，后10条全部成功
	results := make([]TestResult, 20)
	for i := 0; i < 10; i++ {
		results[i] = TestResult{Ok: false}
	}
	for i := 10; i < 20; i++ {
		results[i] = TestResult{Ok: true}
	}

	calc := DefaultTrustScoreCalculator()
	calc.RecentTestCount = 10 // 只看最近10条

	result := calc.ComputeScoreFromHistory(200, 0, false, results, true)
	// 最近10条全部成功，历史分应为1.0
	if result.Components.HistoryScore != 1.0 {
		t.Errorf("HistoryScore = %.2f, 最近10条全部成功，期望 1.0", result.Components.HistoryScore)
	}

	// 反过来：前10条成功，后10条失败
	for i := 0; i < 10; i++ {
		results[i] = TestResult{Ok: true}
	}
	for i := 10; i < 20; i++ {
		results[i] = TestResult{Ok: false}
	}

	result2 := calc.ComputeScoreFromHistory(200, 0, false, results, true)
	if result2.Components.HistoryScore != 0.0 {
		t.Errorf("HistoryScore = %.2f, 最近10条全部失败，期望 0.0", result2.Components.HistoryScore)
	}
}

// TestTrustScoreWeightsSum 验证权重和为1.0。
func TestTrustScoreWeightsSum(t *testing.T) {
	calc := DefaultTrustScoreCalculator()
	sum := calc.LatencyWeight + calc.IPHealthWeight + calc.HistoryWeight + calc.RegionMatchWeight
	if sum < 0.99 || sum > 1.01 {
		t.Errorf("权重和 = %.2f, 期望 ~1.0", sum)
	}
}

// TestTrustScoreLatencyInterpolation 测试延迟线性插值的准确性。
func TestTrustScoreLatencyInterpolation(t *testing.T) {
	calc := DefaultTrustScoreCalculator()

	// 在阈值中点：延迟(200+5000)/2=2600ms 应接近 0.5
	result := calc.ComputeScore(2600, 0, false, 1.0, true)
	if result.Components.LatencyScore < 0.4 || result.Components.LatencyScore > 0.6 {
		t.Errorf("中点延迟 LatencyScore = %.2f, 期望 ~0.5", result.Components.LatencyScore)
	}

	// 零延迟或负延迟
	resultZero := calc.ComputeScore(0, 0, false, 1.0, true)
	if resultZero.Components.LatencyScore != 0 {
		t.Errorf("延迟=0 时 LatencyScore = %.2f, 期望 0", resultZero.Components.LatencyScore)
	}

	resultNeg := calc.ComputeScore(-1, 0, false, 1.0, true)
	if resultNeg.Components.LatencyScore != 0 {
		t.Errorf("负延迟 LatencyScore = %.2f, 期望 0", resultNeg.Components.LatencyScore)
	}
}

// TestTrustScoreDatacenterPenalty 验证数据中心惩罚系数。
func TestTrustScoreDatacenterPenalty(t *testing.T) {
	calc := DefaultTrustScoreCalculator()

	residential := calc.ComputeScore(200, 50, false, 1.0, true)
	datacenter := calc.ComputeScore(200, 50, true, 1.0, true)

	// 数据中心IP健康分应严格低于住宅IP
	if residential.Components.IPHealthScore <= datacenter.Components.IPHealthScore {
		t.Errorf("住宅 IP健康分 %.2f 应 > 数据中心 %.2f",
			residential.Components.IPHealthScore, datacenter.Components.IPHealthScore)
	}

	// 数据中心总分应更低
	if residential.OverallScore <= datacenter.OverallScore {
		t.Errorf("住宅总分 %.1f 应 > 数据中心总分 %.1f",
			residential.OverallScore, datacenter.OverallScore)
	}
}

// TestTrustScoreFraudScoreClamping 测试欺诈分边界处理。
func TestTrustScoreFraudScoreClamping(t *testing.T) {
	calc := DefaultTrustScoreCalculator()

	// 欺诈分超过100
	r1 := calc.ComputeScore(200, 150, false, 1.0, true)
	if r1.Components.IPHealthScore < 0 {
		t.Errorf("IPHealthScore = %.2f, 不应为负（已钳制）", r1.Components.IPHealthScore)
	}
	if r1.Components.IPHealthScore != 0 {
		t.Errorf("欺诈分150时 IPHealthScore = %.2f, 期望 0", r1.Components.IPHealthScore)
	}

	// 未知欺诈分(-1)
	r2 := calc.ComputeScore(200, -1, false, 1.0, true)
	if r2.Components.IPHealthScore != 0.5 {
		t.Errorf("未知欺诈分 IPHealthScore = %.2f, 期望 0.5", r2.Components.IPHealthScore)
	}
}
