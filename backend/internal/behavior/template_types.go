package behavior

// ExecutionPermissionMode is the user-selected permission tier for replaying a template.
type ExecutionPermissionMode string

const (
	PermissionAskEachTime ExecutionPermissionMode = "ask_each_time"
	PermissionAutoReview  ExecutionPermissionMode = "auto_review"
	PermissionFullAccess  ExecutionPermissionMode = "full_access"
)

// HumanBoundary is an action family that must always pause for the user.
type HumanBoundary string

const (
	HumanBoundaryCaptcha         HumanBoundary = "captcha"
	HumanBoundaryTwoFactor       HumanBoundary = "2fa"
	HumanBoundaryDeviceTrust     HumanBoundary = "device_trust"
	HumanBoundaryPassword        HumanBoundary = "password"
	HumanBoundaryDelete          HumanBoundary = "delete"
	HumanBoundaryAccountSettings HumanBoundary = "account_settings"
	HumanBoundarySwitchAccount   HumanBoundary = "switch_account"
)

var mandatoryHumanBoundaries = []HumanBoundary{
	HumanBoundaryCaptcha,
	HumanBoundaryTwoFactor,
	HumanBoundaryDeviceTrust,
	HumanBoundaryPassword,
	HumanBoundaryDelete,
	HumanBoundaryAccountSettings,
	HumanBoundarySwitchAccount,
}

// LowConfidencePausePolicy keeps low-confidence pauses reason/action-only.
type LowConfidencePausePolicy struct {
	Enabled                 bool     `json:"enabled"`
	RevealTargetScreenshot  bool     `json:"revealTargetScreenshot"`
	RevealCandidateElements bool     `json:"revealCandidateElements"`
	RevealRecommendedPoint  bool     `json:"revealRecommendedPoint"`
	PromptFields            []string `json:"promptFields"`
}

// ExecutionPolicy is the stable policy envelope that can be embedded in template playback requests.
type ExecutionPolicy struct {
	PermissionMode     ExecutionPermissionMode  `json:"permissionMode"`
	HumanBoundaries    []HumanBoundary          `json:"humanBoundaries"`
	LowConfidencePause LowConfidencePausePolicy `json:"lowConfidencePause"`
}

// TemplateSemantics captures the MVP semantics for a recording-backed template.
type TemplateSemantics struct {
	SchemaVersion        int             `json:"schemaVersion"`
	Source               string          `json:"source"`
	Intent               string          `json:"intent"`
	ThirdPartyDetection  bool            `json:"thirdPartyDetection"`
	ScreenshotCandidates bool            `json:"screenshotCandidates"`
	HumanBoundaries      []HumanBoundary `json:"humanBoundaries"`
}

// MandatoryHumanBoundaries returns a copy of forced human-review boundaries.
func MandatoryHumanBoundaries() []HumanBoundary {
	boundaries := make([]HumanBoundary, len(mandatoryHumanBoundaries))
	copy(boundaries, mandatoryHumanBoundaries)
	return boundaries
}

// DefaultExecutionPolicy returns the least-permissive policy unless a valid mode is supplied.
func DefaultExecutionPolicy(mode ExecutionPermissionMode) ExecutionPolicy {
	if !isValidExecutionPermissionMode(mode) {
		mode = PermissionAskEachTime
	}
	return ExecutionPolicy{
		PermissionMode:  mode,
		HumanBoundaries: MandatoryHumanBoundaries(),
		LowConfidencePause: LowConfidencePausePolicy{
			Enabled:                 true,
			RevealTargetScreenshot:  false,
			RevealCandidateElements: false,
			RevealRecommendedPoint:  false,
			PromptFields:            []string{"reason", "action"},
		},
	}
}

// DefaultTemplateSemantics records that the MVP has no third-party detection or screenshot candidates.
func DefaultTemplateSemantics(policy ExecutionPolicy) TemplateSemantics {
	boundaries := policy.HumanBoundaries
	if len(boundaries) == 0 {
		boundaries = MandatoryHumanBoundaries()
	}
	copied := make([]HumanBoundary, len(boundaries))
	copy(copied, boundaries)
	return TemplateSemantics{
		SchemaVersion:        1,
		Source:               "recording_template",
		Intent:               "unknown",
		ThirdPartyDetection:  false,
		ScreenshotCandidates: false,
		HumanBoundaries:      copied,
	}
}

func isValidExecutionPermissionMode(mode ExecutionPermissionMode) bool {
	switch mode {
	case PermissionAskEachTime, PermissionAutoReview, PermissionFullAccess:
		return true
	default:
		return false
	}
}
