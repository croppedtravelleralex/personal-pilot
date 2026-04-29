package offsets

// OffsetVariant 偏移模板定义
type OffsetVariant struct {
	ID             string         `json:"id"`
	Category       string         `json:"category"`
	Name           string         `json:"name"`
	ClickOffset    *ClickVariant  `json:"clickOffset,omitempty"`
	TimingOffset   *TimingVariant `json:"timingOffset,omitempty"`
	ScrollOffset   *ScrollVariant `json:"scrollOffset,omitempty"`
	BehaviorPreset string         `json:"behaviorPreset"`
	Description    string         `json:"description"`
}

type ClickVariant struct {
	RadiusPx    int    `json:"radiusPx"`
	BiasDir     string `json:"biasDir"`
	HoverMs     int    `json:"hoverMs"`
	DoubleClick bool   `json:"doubleClick"`
}

type TimingVariant struct {
	PreDelayMs  int     `json:"preDelayMs"`
	PostDelayMs int     `json:"postDelayMs"`
	JitterRatio float64 `json:"jitterRatio"`
}

type ScrollVariant struct {
	StepPx         int     `json:"stepPx"`
	OvershootRatio float64 `json:"overshootRatio"`
	PauseMs        int     `json:"pauseMs"`
	SmoothSteps    int     `json:"smoothSteps"`
}

// CategoryHumanNames maps category codes to display names
var CategoryHumanNames = map[string]string{
	"open":   "打开应用",
	"browse": "浏览/滚动",
	"click":  "点击/点赞",
	"type":   "搜索/输入",
	"wait":   "等待/空闲",
}
