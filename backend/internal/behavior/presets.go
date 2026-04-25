package behavior

// BuiltinPresets returns the standard set of behavioral personas.
func BuiltinPresets() []Profile {
	return []Profile{
		{
			ID:          "office-worker",
			Name:        "办公室职员",
			Description: "中等速度，标准键盘节奏，偶尔走神滚动，最常用的画像",
			Mouse: MouseProfile{
				Enabled: true, IdleMoveInterval: "5s-20s",
				CurveStyle: "bezier2", SpeedMean: 400, SpeedStdDev: 100,
				JitterPx: 2, PauseProb: 0.1, PauseMaxMs: 150,
			},
			Keyboard: KeyboardProfile{
				Enabled: true, BaseDelayMs: 120, DelayStdDevMs: 40,
				BurstProb: 0.05, BurstKeys: 2,
				TypoProb: 0.01, BigramDelays: true,
			},
			Scroll: ScrollProfile{
				Enabled: true, IdleScrollProb: 0.15,
				ScrollStepPx: 120, ScrollStepStdDev: 40,
				PauseBetweenMs: 800, PauseStdDevMs: 400,
				OverscrollProb: 0.05, ReverseProb: 0.03,
			},
			Idle: IdleProfile{MouseWander: true, TabSwitch: false, FocusLoss: false},
		},
		{
			ID:          "student",
			Name:        "学生",
			Description: "快速打字，快速滚动，偶尔急躁操作",
			Mouse: MouseProfile{
				Enabled: true, IdleMoveInterval: "3s-10s",
				CurveStyle: "bezier2", SpeedMean: 600, SpeedStdDev: 150,
				JitterPx: 3, PauseProb: 0.05, PauseMaxMs: 80,
			},
			Keyboard: KeyboardProfile{
				Enabled: true, BaseDelayMs: 80, DelayStdDevMs: 30,
				BurstProb: 0.1, BurstKeys: 3,
				TypoProb: 0.02, BigramDelays: false,
			},
			Scroll: ScrollProfile{
				Enabled: true, IdleScrollProb: 0.25,
				ScrollStepPx: 200, ScrollStepStdDev: 80,
				PauseBetweenMs: 400, PauseStdDevMs: 200,
				OverscrollProb: 0.1, ReverseProb: 0.05,
			},
			Idle: IdleProfile{MouseWander: true, TabSwitch: true, FocusLoss: false},
		},
		{
			ID:          "senior-executive",
			Name:        "高管",
			Description: "慢速、深思熟虑、极少打字错误，鼠标移动沉稳",
			Mouse: MouseProfile{
				Enabled: true, IdleMoveInterval: "10s-30s",
				CurveStyle: "bezier3", SpeedMean: 250, SpeedStdDev: 60,
				JitterPx: 1, PauseProb: 0.2, PauseMaxMs: 300,
			},
			Keyboard: KeyboardProfile{
				Enabled: true, BaseDelayMs: 180, DelayStdDevMs: 50,
				BurstProb: 0.02, BurstKeys: 2,
				TypoProb: 0.003, BigramDelays: true,
			},
			Scroll: ScrollProfile{
				Enabled: true, IdleScrollProb: 0.08,
				ScrollStepPx: 80, ScrollStepStdDev: 30,
				PauseBetweenMs: 1500, PauseStdDevMs: 800,
				OverscrollProb: 0.02, ReverseProb: 0.01,
			},
			Idle: IdleProfile{MouseWander: false, TabSwitch: false, FocusLoss: true},
		},
		{
			ID:          "gamer",
			Name:        "游戏玩家",
			Description: "极快速鼠标移动，精准点击，键盘高速连击",
			Mouse: MouseProfile{
				Enabled: true, IdleMoveInterval: "1s-5s",
				CurveStyle: "natural", SpeedMean: 800, SpeedStdDev: 200,
				JitterPx: 1, PauseProb: 0.02, PauseMaxMs: 50,
			},
			Keyboard: KeyboardProfile{
				Enabled: true, BaseDelayMs: 50, DelayStdDevMs: 20,
				BurstProb: 0.15, BurstKeys: 4,
				TypoProb: 0.005, BigramDelays: false,
			},
			Scroll: ScrollProfile{
				Enabled: true, IdleScrollProb: 0.3,
				ScrollStepPx: 250, ScrollStepStdDev: 100,
				PauseBetweenMs: 200, PauseStdDevMs: 100,
				OverscrollProb: 0.15, ReverseProb: 0.08,
			},
			Idle: IdleProfile{MouseWander: true, TabSwitch: true, FocusLoss: true},
		},
		{
			ID:          "elderly",
			Name:        "年长用户",
			Description: "慢速操作，长停顿，偶尔误触，滚动犹豫",
			Mouse: MouseProfile{
				Enabled: true, IdleMoveInterval: "15s-45s",
				CurveStyle: "bezier2", SpeedMean: 180, SpeedStdDev: 80,
				JitterPx: 5, PauseProb: 0.3, PauseMaxMs: 500,
			},
			Keyboard: KeyboardProfile{
				Enabled: true, BaseDelayMs: 200, DelayStdDevMs: 80,
				BurstProb: 0.01, BurstKeys: 2,
				TypoProb: 0.03, BigramDelays: true,
			},
			Scroll: ScrollProfile{
				Enabled: true, IdleScrollProb: 0.05,
				ScrollStepPx: 60, ScrollStepStdDev: 25,
				PauseBetweenMs: 2000, PauseStdDevMs: 1000,
				OverscrollProb: 0.08, ReverseProb: 0.04,
			},
			Idle: IdleProfile{MouseWander: false, TabSwitch: false, FocusLoss: false},
		},
		{
			ID:          "bot-minimal",
			Name:        "隐身（最小注入）",
			Description: "几乎无行为注入，仅偶尔鼠标微动防止绝对静止检测",
			Mouse: MouseProfile{
				Enabled: true, IdleMoveInterval: "30s-90s",
				CurveStyle: "bezier2", SpeedMean: 100, SpeedStdDev: 30,
				JitterPx: 1, PauseProb: 0.0, PauseMaxMs: 0,
			},
			Keyboard: KeyboardProfile{
				Enabled: false, BaseDelayMs: 0, DelayStdDevMs: 0,
				BurstProb: 0, BurstKeys: 0,
				TypoProb: 0, BigramDelays: false,
			},
			Scroll: ScrollProfile{
				Enabled: false, IdleScrollProb: 0,
				ScrollStepPx: 0, ScrollStepStdDev: 0,
				PauseBetweenMs: 0, PauseStdDevMs: 0,
				OverscrollProb: 0, ReverseProb: 0,
			},
			Idle: IdleProfile{MouseWander: true, TabSwitch: false, FocusLoss: false},
		},
	}
}

// GetPreset returns a preset by ID, or nil if not found.
func GetPreset(id string) *Profile {
	for _, p := range BuiltinPresets() {
		if p.ID == id {
			return &p
		}
	}
	return nil
}

// PresetIDs returns all preset IDs.
func PresetIDs() []string {
	presets := BuiltinPresets()
	ids := make([]string, len(presets))
	for i, p := range presets {
		ids[i] = p.ID
	}
	return ids
}
