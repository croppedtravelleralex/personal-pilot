package offsets

// BuiltinOffsetLibrary returns all 50 built-in offset variants.
func BuiltinOffsetLibrary() []OffsetVariant {
	variants := make([]OffsetVariant, 0, 50)

	// ── open (10) ──
	openVariants := []OffsetVariant{
		{ID: "open-001", Category: "open", Name: "快速启动", TimingOffset: &TimingVariant{PreDelayMs: 100, PostDelayMs: 200, JitterRatio: 0.1}, BehaviorPreset: "gamer", Description: "低延迟快速应用启动"},
		{ID: "open-002", Category: "open", Name: "标准启动", TimingOffset: &TimingVariant{PreDelayMs: 300, PostDelayMs: 500, JitterRatio: 0.15}, BehaviorPreset: "office-worker", Description: "正常节奏打开应用"},
		{ID: "open-003", Category: "open", Name: "延迟启动", TimingOffset: &TimingVariant{PreDelayMs: 800, PostDelayMs: 1200, JitterRatio: 0.2}, BehaviorPreset: "elderly", Description: "较慢速启动，模拟犹豫"},
		{ID: "open-004", Category: "open", Name: "寻找图标", TimingOffset: &TimingVariant{PreDelayMs: 500, PostDelayMs: 800, JitterRatio: 0.25}, ClickOffset: &ClickVariant{RadiusPx: 8, BiasDir: "random", HoverMs: 300}, BehaviorPreset: "student", Description: "在桌面上寻找并点击应用图标"},
		{ID: "open-005", Category: "open", Name: "双击启动", TimingOffset: &TimingVariant{PreDelayMs: 200, PostDelayMs: 400, JitterRatio: 0.1}, ClickOffset: &ClickVariant{RadiusPx: 2, BiasDir: "center", HoverMs: 0, DoubleClick: true}, BehaviorPreset: "gamer", Description: "快速双击启动"},
		{ID: "open-006", Category: "open", Name: "悬停确认", TimingOffset: &TimingVariant{PreDelayMs: 600, PostDelayMs: 900, JitterRatio: 0.2}, ClickOffset: &ClickVariant{RadiusPx: 3, BiasDir: "center", HoverMs: 500}, BehaviorPreset: "senior-executive", Description: "悬停确认后再点击"},
		{ID: "open-007", Category: "open", Name: "误触纠正", TimingOffset: &TimingVariant{PreDelayMs: 400, PostDelayMs: 700, JitterRatio: 0.3}, ClickOffset: &ClickVariant{RadiusPx: 12, BiasDir: "top-left", HoverMs: 200}, BehaviorPreset: "elderly", Description: "首次点击偏移较大，模拟误触后纠正"},
		{ID: "open-008", Category: "open", Name: "多任务切换", TimingOffset: &TimingVariant{PreDelayMs: 1000, PostDelayMs: 1500, JitterRatio: 0.25}, BehaviorPreset: "office-worker", Description: "从其他应用切换回来"},
		{ID: "open-009", Category: "open", Name: "通知栏启动", TimingOffset: &TimingVariant{PreDelayMs: 300, PostDelayMs: 600, JitterRatio: 0.15}, BehaviorPreset: "student", Description: "从通知栏点击进入"},
		{ID: "open-010", Category: "open", Name: "搜索框启动", TimingOffset: &TimingVariant{PreDelayMs: 500, PostDelayMs: 800, JitterRatio: 0.2}, BehaviorPreset: "office-worker", Description: "通过搜索找到应用"},
	}
	variants = append(variants, openVariants...)

	// ── browse (15) ──
	browseVariants := []OffsetVariant{
		{ID: "browse-001", Category: "browse", Name: "快速扫视", ScrollOffset: &ScrollVariant{StepPx: 250, OvershootRatio: 0.15, PauseMs: 300, SmoothSteps: 4}, TimingOffset: &TimingVariant{PreDelayMs: 100, PostDelayMs: 200, JitterRatio: 0.1}, BehaviorPreset: "student", Description: "快速上下扫视内容"},
		{ID: "browse-002", Category: "browse", Name: "深度阅读", ScrollOffset: &ScrollVariant{StepPx: 80, OvershootRatio: 0.05, PauseMs: 1500, SmoothSteps: 8}, TimingOffset: &TimingVariant{PreDelayMs: 300, PostDelayMs: 800, JitterRatio: 0.15}, BehaviorPreset: "senior-executive", Description: "慢速仔细阅读每一屏"},
		{ID: "browse-003", Category: "browse", Name: "反复回看", ScrollOffset: &ScrollVariant{StepPx: 120, OvershootRatio: 0.25, PauseMs: 500, SmoothSteps: 6}, TimingOffset: &TimingVariant{PreDelayMs: 200, PostDelayMs: 400, JitterRatio: 0.2}, BehaviorPreset: "elderly", Description: "下滚后经常回滚重新看"},
		{ID: "browse-004", Category: "browse", Name: "跳跃浏览", ScrollOffset: &ScrollVariant{StepPx: 350, OvershootRatio: 0.3, PauseMs: 200, SmoothSteps: 3}, TimingOffset: &TimingVariant{PreDelayMs: 50, PostDelayMs: 100, JitterRatio: 0.1}, BehaviorPreset: "gamer", Description: "大跨步跳跃式浏览"},
		{ID: "browse-005", Category: "browse", Name: "匀速滑动", ScrollOffset: &ScrollVariant{StepPx: 150, OvershootRatio: 0.1, PauseMs: 400, SmoothSteps: 5}, TimingOffset: &TimingVariant{PreDelayMs: 150, PostDelayMs: 250, JitterRatio: 0.1}, BehaviorPreset: "office-worker", Description: "匀速持续滑动浏览"},
		{ID: "browse-006", Category: "browse", Name: "停留阅读", ScrollOffset: &ScrollVariant{StepPx: 100, OvershootRatio: 0.0, PauseMs: 2000, SmoothSteps: 6}, TimingOffset: &TimingVariant{PreDelayMs: 400, PostDelayMs: 1000, JitterRatio: 0.15}, BehaviorPreset: "senior-executive", Description: "每屏长时间停留仔细阅读"},
		{ID: "browse-007", Category: "browse", Name: "图片浏览", ScrollOffset: &ScrollVariant{StepPx: 200, OvershootRatio: 0.2, PauseMs: 600, SmoothSteps: 5}, TimingOffset: &TimingVariant{PreDelayMs: 200, PostDelayMs: 300, JitterRatio: 0.15}, BehaviorPreset: "student", Description: "以图片为主的快速浏览"},
		{ID: "browse-008", Category: "browse", Name: "评论浏览", ScrollOffset: &ScrollVariant{StepPx: 60, OvershootRatio: 0.05, PauseMs: 800, SmoothSteps: 10}, TimingOffset: &TimingVariant{PreDelayMs: 300, PostDelayMs: 500, JitterRatio: 0.2}, BehaviorPreset: "office-worker", Description: "慢速阅读评论区"},
		{ID: "browse-009", Category: "browse", Name: "视频浏览", ScrollOffset: &ScrollVariant{StepPx: 500, OvershootRatio: 0.1, PauseMs: 3000, SmoothSteps: 2}, TimingOffset: &TimingVariant{PreDelayMs: 100, PostDelayMs: 5000, JitterRatio: 0.1}, BehaviorPreset: "student", Description: "每个视频停留观看"},
		{ID: "browse-010", Category: "browse", Name: "随机滑动", ScrollOffset: &ScrollVariant{StepPx: 180, OvershootRatio: 0.35, PauseMs: 350, SmoothSteps: 4}, TimingOffset: &TimingVariant{PreDelayMs: 100, PostDelayMs: 800, JitterRatio: 0.3}, BehaviorPreset: "elderly", Description: "方向不定的随机浏览"},
		{ID: "browse-011", Category: "browse", Name: "回到顶部", ScrollOffset: &ScrollVariant{StepPx: 400, OvershootRatio: 0.1, PauseMs: 200, SmoothSteps: 3}, TimingOffset: &TimingVariant{PreDelayMs: 300, PostDelayMs: 500, JitterRatio: 0.1}, BehaviorPreset: "office-worker", Description: "快速回到页面顶部"},
		{ID: "browse-012", Category: "browse", Name: "下拉刷新", ScrollOffset: &ScrollVariant{StepPx: 200, OvershootRatio: 0.4, PauseMs: 1000, SmoothSteps: 5}, TimingOffset: &TimingVariant{PreDelayMs: 200, PostDelayMs: 1500, JitterRatio: 0.15}, BehaviorPreset: "student", Description: "下拉刷新内容"},
		{ID: "browse-013", Category: "browse", Name: "横向滑动", ScrollOffset: &ScrollVariant{StepPx: 300, OvershootRatio: 0.1, PauseMs: 400, SmoothSteps: 4}, TimingOffset: &TimingVariant{PreDelayMs: 150, PostDelayMs: 300, JitterRatio: 0.1}, BehaviorPreset: "gamer", Description: "横向滑动浏览轮播"},
		{ID: "browse-014", Category: "browse", Name: "惯性滑动", ScrollOffset: &ScrollVariant{StepPx: 220, OvershootRatio: 0.5, PauseMs: 250, SmoothSteps: 3}, TimingOffset: &TimingVariant{PreDelayMs: 50, PostDelayMs: 150, JitterRatio: 0.05}, BehaviorPreset: "gamer", Description: "快速惯性滑动"},
		{ID: "browse-015", Category: "browse", Name: "边读边滑", ScrollOffset: &ScrollVariant{StepPx: 30, OvershootRatio: 0.02, PauseMs: 200, SmoothSteps: 12}, TimingOffset: &TimingVariant{PreDelayMs: 200, PostDelayMs: 300, JitterRatio: 0.15}, BehaviorPreset: "senior-executive", Description: "逐行阅读时微调滚动"},
	}
	variants = append(variants, browseVariants...)

	// ── click (15) ──
	clickVariants := []OffsetVariant{
		{ID: "click-001", Category: "click", Name: "精确点击", ClickOffset: &ClickVariant{RadiusPx: 1, BiasDir: "center", HoverMs: 50}, TimingOffset: &TimingVariant{PreDelayMs: 100, PostDelayMs: 200, JitterRatio: 0.05}, BehaviorPreset: "gamer", Description: "高精度快速点击"},
		{ID: "click-002", Category: "click", Name: "自然偏移", ClickOffset: &ClickVariant{RadiusPx: 5, BiasDir: "bottom-right", HoverMs: 150}, TimingOffset: &TimingVariant{PreDelayMs: 200, PostDelayMs: 400, JitterRatio: 0.15}, BehaviorPreset: "office-worker", Description: "略带偏移的自然点击"},
		{ID: "click-003", Category: "click", Name: "犹豫点击", ClickOffset: &ClickVariant{RadiusPx: 8, BiasDir: "random", HoverMs: 400}, TimingOffset: &TimingVariant{PreDelayMs: 500, PostDelayMs: 600, JitterRatio: 0.25}, BehaviorPreset: "elderly", Description: "长时间悬停后点击"},
		{ID: "click-004", Category: "click", Name: "双击", ClickOffset: &ClickVariant{RadiusPx: 3, BiasDir: "center", HoverMs: 80, DoubleClick: true}, TimingOffset: &TimingVariant{PreDelayMs: 150, PostDelayMs: 250, JitterRatio: 0.1}, BehaviorPreset: "student", Description: "快速双击"},
		{ID: "click-005", Category: "click", Name: "滑动点击", ClickOffset: &ClickVariant{RadiusPx: 6, BiasDir: "top-right", HoverMs: 100}, TimingOffset: &TimingVariant{PreDelayMs: 100, PostDelayMs: 300, JitterRatio: 0.2}, BehaviorPreset: "student", Description: "滑动后惯性点击"},
		{ID: "click-006", Category: "click", Name: "拇指点击", ClickOffset: &ClickVariant{RadiusPx: 10, BiasDir: "bottom-right", HoverMs: 200}, TimingOffset: &TimingVariant{PreDelayMs: 250, PostDelayMs: 350, JitterRatio: 0.2}, BehaviorPreset: "office-worker", Description: "模拟右手拇指点击（偏右下）"},
		{ID: "click-007", Category: "click", Name: "左手点击", ClickOffset: &ClickVariant{RadiusPx: 8, BiasDir: "bottom-left", HoverMs: 180}, TimingOffset: &TimingVariant{PreDelayMs: 300, PostDelayMs: 400, JitterRatio: 0.2}, BehaviorPreset: "student", Description: "模拟左手操作（偏左下）"},
		{ID: "click-008", Category: "click", Name: "笔直点击", ClickOffset: &ClickVariant{RadiusPx: 0, BiasDir: "center", HoverMs: 0}, TimingOffset: &TimingVariant{PreDelayMs: 80, PostDelayMs: 150, JitterRatio: 0.05}, BehaviorPreset: "bot-minimal", Description: "近乎完美的精确点击"},
		{ID: "click-009", Category: "click", Name: "误触边角", ClickOffset: &ClickVariant{RadiusPx: 15, BiasDir: "top-left", HoverMs: 50}, TimingOffset: &TimingVariant{PreDelayMs: 100, PostDelayMs: 500, JitterRatio: 0.3}, BehaviorPreset: "elderly", Description: "首次误触边角后纠正"},
		{ID: "click-010", Category: "click", Name: "连击", ClickOffset: &ClickVariant{RadiusPx: 4, BiasDir: "random", HoverMs: 60, DoubleClick: true}, TimingOffset: &TimingVariant{PreDelayMs: 50, PostDelayMs: 100, JitterRatio: 0.1}, BehaviorPreset: "gamer", Description: "快速连击模式"},
		{ID: "click-011", Category: "click", Name: "悬停预判", ClickOffset: &ClickVariant{RadiusPx: 5, BiasDir: "center", HoverMs: 300}, TimingOffset: &TimingVariant{PreDelayMs: 400, PostDelayMs: 200, JitterRatio: 0.2}, BehaviorPreset: "senior-executive", Description: "移动光标预判后再点"},
		{ID: "click-012", Category: "click", Name: "长按", ClickOffset: &ClickVariant{RadiusPx: 3, BiasDir: "center", HoverMs: 800}, TimingOffset: &TimingVariant{PreDelayMs: 300, PostDelayMs: 500, JitterRatio: 0.1}, BehaviorPreset: "elderly", Description: "长按后释放（模拟长按操作）"},
		{ID: "click-013", Category: "click", Name: "拖拽点击", ClickOffset: &ClickVariant{RadiusPx: 7, BiasDir: "top-right", HoverMs: 250}, TimingOffset: &TimingVariant{PreDelayMs: 200, PostDelayMs: 400, JitterRatio: 0.25}, BehaviorPreset: "office-worker", Description: "轻微拖拽后再点击"},
		{ID: "click-014", Category: "click", Name: "双手交替", ClickOffset: &ClickVariant{RadiusPx: 9, BiasDir: "random", HoverMs: 150}, TimingOffset: &TimingVariant{PreDelayMs: 350, PostDelayMs: 450, JitterRatio: 0.25}, BehaviorPreset: "student", Description: "双手交替操作的不规则点击"},
		{ID: "click-015", Category: "click", Name: "手抖点击", ClickOffset: &ClickVariant{RadiusPx: 12, BiasDir: "random", HoverMs: 350}, TimingOffset: &TimingVariant{PreDelayMs: 600, PostDelayMs: 700, JitterRatio: 0.35}, BehaviorPreset: "elderly", Description: "手抖导致的位移"},
	}
	variants = append(variants, clickVariants...)

	// ── type (5) ──
	typeVariants := []OffsetVariant{
		{ID: "type-001", Category: "type", Name: "快速输入", TimingOffset: &TimingVariant{PreDelayMs: 80, PostDelayMs: 100, JitterRatio: 0.05}, BehaviorPreset: "gamer", Description: "极快速键盘输入"},
		{ID: "type-002", Category: "type", Name: "标准输入", TimingOffset: &TimingVariant{PreDelayMs: 200, PostDelayMs: 300, JitterRatio: 0.1}, BehaviorPreset: "office-worker", Description: "正常打字速度"},
		{ID: "type-003", Category: "type", Name: "带错输入", TimingOffset: &TimingVariant{PreDelayMs: 300, PostDelayMs: 600, JitterRatio: 0.2}, BehaviorPreset: "elderly", Description: "有打字错误和纠正"},
		{ID: "type-004", Category: "type", Name: "犹豫输入", TimingOffset: &TimingVariant{PreDelayMs: 500, PostDelayMs: 800, JitterRatio: 0.25}, BehaviorPreset: "senior-executive", Description: "思考后输入，带停顿"},
		{ID: "type-005", Category: "type", Name: "分段输入", TimingOffset: &TimingVariant{PreDelayMs: 250, PostDelayMs: 1000, JitterRatio: 0.3}, BehaviorPreset: "office-worker", Description: "分几段输入长文本"},
	}
	variants = append(variants, typeVariants...)

	// ── wait (5) ──
	waitVariants := []OffsetVariant{
		{ID: "wait-001", Category: "wait", Name: "短暂停顿", TimingOffset: &TimingVariant{PreDelayMs: 200, PostDelayMs: 300, JitterRatio: 0.1}, BehaviorPreset: "student", Description: "1-3秒短暂浏览停顿"},
		{ID: "wait-002", Category: "wait", Name: "中等阅读", TimingOffset: &TimingVariant{PreDelayMs: 500, PostDelayMs: 800, JitterRatio: 0.15}, BehaviorPreset: "office-worker", Description: "3-8秒正常阅读时间"},
		{ID: "wait-003", Category: "wait", Name: "深度阅读", TimingOffset: &TimingVariant{PreDelayMs: 1000, PostDelayMs: 2000, JitterRatio: 0.2}, BehaviorPreset: "senior-executive", Description: "8-20秒深度阅读停顿"},
		{ID: "wait-004", Category: "wait", Name: "走神停顿", TimingOffset: &TimingVariant{PreDelayMs: 2000, PostDelayMs: 5000, JitterRatio: 0.3}, BehaviorPreset: "elderly", Description: "长时间走神/被打断"},
		{ID: "wait-005", Category: "wait", Name: "加载等待", TimingOffset: &TimingVariant{PreDelayMs: 500, PostDelayMs: 1500, JitterRatio: 0.25}, BehaviorPreset: "office-worker", Description: "等待页面加载/网络延迟"},
	}
	variants = append(variants, waitVariants...)

	return variants
}
