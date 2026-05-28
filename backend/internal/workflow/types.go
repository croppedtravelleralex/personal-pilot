package workflow

import "time"

type Workflow struct {
	ID        string
	Name      string
	Steps     []Step
	Variables map[string]any
	Config    Config
}

type Step struct {
	ID        string
	Action    Action
	DependsOn []string
	Condition string
	Retry     RetryPolicy
}

type Action struct {
	Type   string
	Params map[string]any
}

type Config struct {
	MaxParallel uint32
	Timeout     time.Duration
}

type StepStatus string

const (
	StepPending StepStatus = "pending"
	StepRunning StepStatus = "running"
	StepPassed  StepStatus = "passed"
	StepFailed  StepStatus = "failed"
	StepSkipped StepStatus = "skipped"
)

type ExecutionState struct {
	WorkflowID string
	StepStatus map[string]StepStatus
	Variables  map[string]any
	Errors     map[string]string
}
