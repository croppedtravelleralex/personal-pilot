package asymmetric

// AutopilotStep records one automated remediation action.
type AutopilotStep struct {
	ID      string `json:"id"`
	Status  string `json:"status"` // ok, skipped, failed
	Detail  string `json:"detail,omitempty"`
	Changed bool   `json:"changed"`
}

// AutopilotReport is the zero-human 99+ pipeline output.
type AutopilotReport struct {
	ProfileID    string          `json:"profileId"`
	Steps        []AutopilotStep `json:"steps"`
	Iterations   int             `json:"iterations"`
	StealthScore float64         `json:"stealthScore"`
	Grade        string          `json:"grade"`
	Target99Plus bool            `json:"target99Plus"`
	Gaps         []string        `json:"gaps,omitempty"`
	Blocked      bool            `json:"blocked"`
	BlockReason  string          `json:"blockReason,omitempty"`
}

const (
	StepBindResidential  = "bind_residential_proxy"
	StepPersistRuntime   = "persist_runtime_hardening"
	StepImportEnvToken   = "import_env_refresh_token"
	StepEnsureRunning    = "ensure_browser_running"
	StepHarvestTrust     = "harvest_browser_trust"
	StepRefreshGraph     = "refresh_graph_token"
	StepRunProbes        = "run_stealth_probes"
	StepApplyFeedback    = "apply_feedback"
)
