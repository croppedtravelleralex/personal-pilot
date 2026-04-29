package humanize

// LlmActionType enumerates all actions the automation template engine can plan.
type LlmActionType int

const (
	ActionOpenBrowser  LlmActionType = 0
	ActionGoto         LlmActionType = 1
	ActionClick        LlmActionType = 2
	ActionTypeText     LlmActionType = 3
	ActionWait         LlmActionType = 4
	ActionScreenshot   LlmActionType = 5
	ActionGetHtml      LlmActionType = 6
	ActionGetText      LlmActionType = 7
	ActionScroll       LlmActionType = 8
	ActionExecuteJs    LlmActionType = 9
	ActionCloseBrowser LlmActionType = 10
)

// ScrollDirection indicates which way to scroll.
type ScrollDirection int

const (
	ScrollUp        ScrollDirection = 0
	ScrollDown      ScrollDirection = 1
	ScrollToElement ScrollDirection = 2
)

// OffsetOverride allows explicit click position override.
type OffsetOverride struct {
	Dx int32
	Dy int32
}

// LlmAction is a planned action from the automation template engine.
type LlmAction struct {
	Type           LlmActionType
	URL            string
	Selector       string
	Text           string
	DurationMs     uint32
	FullPage       bool
	Direction      ScrollDirection
	DistancePx     *uint32
	TargetSelector *string
	Script         string
	Offset         *OffsetOverride
}

// MutatedActionType enumerates the action types in a mutated (humanized) action.
type MutatedActionType int

const (
	MutatedOpenBrowser  MutatedActionType = 0
	MutatedGoto         MutatedActionType = 1
	MutatedClick        MutatedActionType = 2
	MutatedTypeText     MutatedActionType = 3
	MutatedWait         MutatedActionType = 4
	MutatedScreenshot   MutatedActionType = 5
	MutatedGetHtml      MutatedActionType = 6
	MutatedGetText      MutatedActionType = 7
	MutatedScroll       MutatedActionType = 8
	MutatedExecuteJs    MutatedActionType = 9
	MutatedCloseBrowser MutatedActionType = 10
)

// MutatedAction is an execution-ready action with humanized parameters.
type MutatedAction struct {
	Type     MutatedActionType
	URL      string
	Selector string
	PreGapMs uint32 // pre-action delay, present on ALL variants

	// Click
	ClickTarget *ClickTarget

	// Type
	TypingPlan *TypingPlan

	// Wait
	JitterMs uint32

	// Screenshot
	FullPage bool

	// GetHtml
	HTMLSelector *string

	// Scroll
	ScrollPlan *ScrollPlan

	// ExecuteJs
	Script string
}

// ActionResult represents the outcome of executing a browser action.
type ActionResult struct {
	Success          bool
	ErrorCode        *ActionErrorCode
	ErrorMessage     *string
	ResolvedSelector *string
}

// ActionResultSuccess returns a successful result.
func ActionResultSuccess() ActionResult {
	return ActionResult{Success: true}
}

// ActionResultFailure returns a failed result with the given error code and message.
func ActionResultFailure(code ActionErrorCode, msg string) ActionResult {
	return ActionResult{Success: false, ErrorCode: &code, ErrorMessage: &msg}
}

// ElementBounds describes an element's bounding box on the page.
type ElementBounds struct {
	X1, Y1, X2, Y2 int32
	Width, Height  uint32
}

// CenterX returns the horizontal center of the element.
func (b *ElementBounds) CenterX() int32 { return (b.X1 + b.X2) / 2 }

// CenterY returns the vertical center of the element.
func (b *ElementBounds) CenterY() int32 { return (b.Y1 + b.Y2) / 2 }

// BehavioralMutationMiddleware orchestrates all humanization sub-modules.
// It is the single public entry point for humanizing actions.
type BehavioralMutationMiddleware struct {
	config HumanizationConfig
}

// NewBehavioralMutationMiddleware creates a new middleware with the given config.
func NewBehavioralMutationMiddleware(config HumanizationConfig) *BehavioralMutationMiddleware {
	return &BehavioralMutationMiddleware{config: config}
}

// Config returns the current humanization config (read-only reference).
func (m *BehavioralMutationMiddleware) Config() *HumanizationConfig {
	return &m.config
}

// UpdateConfig replaces the middleware's config at runtime.
func (m *BehavioralMutationMiddleware) UpdateConfig(config HumanizationConfig) {
	m.config = config
}

// Mutate transforms a planned LLM action into an execution-ready humanized action.
func (m *BehavioralMutationMiddleware) Mutate(action LlmAction, elementInfo *ElementBounds) MutatedAction {
	preGapMs := ComputeActionGap(&m.config)

	switch action.Type {
	case ActionOpenBrowser:
		return MutatedAction{Type: MutatedOpenBrowser, URL: action.URL, PreGapMs: preGapMs}

	case ActionGoto:
		return MutatedAction{Type: MutatedGoto, URL: action.URL, PreGapMs: preGapMs}

	case ActionClick:
		target := m.mutateClick(action.Selector, action.Offset, elementInfo)
		return MutatedAction{Type: MutatedClick, Selector: action.Selector, PreGapMs: preGapMs, ClickTarget: &target}

	case ActionTypeText:
		plan := BuildTypingPlan(action.Text, &m.config)
		return MutatedAction{Type: MutatedTypeText, Selector: action.Selector, PreGapMs: preGapMs, TypingPlan: &plan}

	case ActionWait:
		var jitterMs uint32
		if m.config.Level.IsActive() {
			jitterPct := uint32(float64(action.DurationMs) * 0.2)
			if jitterPct > 0 {
				jitterMs = randRangeUint32(0, jitterPct)
			}
		}
		return MutatedAction{Type: MutatedWait, PreGapMs: preGapMs, JitterMs: jitterMs}

	case ActionScreenshot:
		return MutatedAction{Type: MutatedScreenshot, FullPage: action.FullPage, PreGapMs: preGapMs}

	case ActionGetHtml:
		return MutatedAction{Type: MutatedGetHtml, Selector: action.Selector, PreGapMs: preGapMs}

	case ActionGetText:
		return MutatedAction{Type: MutatedGetText, Selector: action.Selector, PreGapMs: preGapMs}

	case ActionScroll:
		plan := m.mutateScroll(action.Direction, action.DistancePx)
		return MutatedAction{Type: MutatedScroll, PreGapMs: preGapMs, ScrollPlan: &plan}

	case ActionExecuteJs:
		return MutatedAction{Type: MutatedExecuteJs, Script: action.Script, PreGapMs: preGapMs}

	case ActionCloseBrowser:
		return MutatedAction{Type: MutatedCloseBrowser, PreGapMs: preGapMs}

	default:
		return MutatedAction{PreGapMs: preGapMs}
	}
}

// DecideRetry determines whether and how to retry after a failed action.
func (m *BehavioralMutationMiddleware) DecideRetry(result *ActionResult, attempt uint32) RetryDecision {
	if result.Success {
		return retryNow() // shouldn't be called on success, but be safe
	}
	errCode := ErrUnknown
	if result.ErrorCode != nil {
		errCode = *result.ErrorCode
	}
	return HumanizedRetryDecision(errCode, attempt, &m.config)
}

// SuggestApproach returns a human-readable recovery suggestion for an error.
func (m *BehavioralMutationMiddleware) SuggestApproach(errCode ActionErrorCode) string {
	return SuggestedApproach(errCode)
}

// mutateClick computes a humanized click target.
func (m *BehavioralMutationMiddleware) mutateClick(selector string, offset *OffsetOverride, elementInfo *ElementBounds) ClickTarget {
	if elementInfo != nil {
		return ComputeClickTarget(
			elementInfo.CenterX(), elementInfo.CenterY(),
			elementInfo.Width, elementInfo.Height,
			&m.config,
		)
	}
	if offset != nil {
		var hoverMs *uint32
		if m.config.Scroll.HoverBeforeScrollMs != nil {
			hoverMs = m.config.Scroll.HoverBeforeScrollMs
		}
		return ClickTarget{X: offset.Dx, Y: offset.Dy, HoverBeforeMs: hoverMs}
	}
	return ClickTarget{}
}

// mutateScroll computes a humanized scroll plan.
func (m *BehavioralMutationMiddleware) mutateScroll(direction ScrollDirection, distancePx *uint32) ScrollPlan {
	var dist uint32
	if distancePx != nil {
		dist = *distancePx
	}
	if dist == 0 {
		dist = 300 // default scroll distance
	}
	return BuildScrollPlan(dist, &m.config)
}
