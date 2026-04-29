package backup

import (
	"crypto/sha256"
	"encoding/hex"
	"fmt"
	"sort"
	"strings"
)

const (
	IssueSeverityWarn  = "warn"
	IssueSeverityBlock = "block"
)

type PreflightIssue struct {
	Severity string `json:"severity"`
	Code     string `json:"code"`
	Message  string `json:"message"`
}

type DestructiveConfirmation struct {
	Confirmed         bool   `json:"confirmed"`
	ConfirmationToken string `json:"confirmationToken"`
}

type DestructivePreflight struct {
	Operation            string           `json:"operation"`
	TargetProfileId      string           `json:"targetProfileId,omitempty"`
	TargetUserDataDir    string           `json:"targetUserDataDir,omitempty"`
	RequiresStop         bool             `json:"requiresStop"`
	Adds                 []string         `json:"adds"`
	Overwrites           []string         `json:"overwrites"`
	Skips                []string         `json:"skips"`
	WritesCookie         bool             `json:"writesCookie"`
	DestructivePaths     []string         `json:"destructivePaths"`
	Warnings             []PreflightIssue `json:"warnings"`
	Blockers             []PreflightIssue `json:"blockers"`
	RequiresConfirmation bool             `json:"requiresConfirmation"`
	ConfirmationToken    string           `json:"confirmationToken"`
	ConfirmationPrompt   string           `json:"confirmationPrompt"`
}

func NewIssue(severity, code, message string) PreflightIssue {
	return PreflightIssue{
		Severity: strings.TrimSpace(severity),
		Code:     strings.TrimSpace(code),
		Message:  strings.TrimSpace(message),
	}
}

func (p *DestructivePreflight) AddWarning(code, message string) {
	p.Warnings = append(p.Warnings, NewIssue(IssueSeverityWarn, code, message))
}

func (p *DestructivePreflight) AddBlocker(code, message string) {
	p.Blockers = append(p.Blockers, NewIssue(IssueSeverityBlock, code, message))
}

func (p *DestructivePreflight) Finalize() {
	p.Operation = strings.TrimSpace(p.Operation)
	p.TargetProfileId = strings.TrimSpace(p.TargetProfileId)
	p.TargetUserDataDir = strings.TrimSpace(p.TargetUserDataDir)
	p.Adds = sortedUniqueStrings(p.Adds)
	p.Overwrites = sortedUniqueStrings(p.Overwrites)
	p.Skips = sortedUniqueStrings(p.Skips)
	p.DestructivePaths = sortedUniqueStrings(p.DestructivePaths)
	p.RequiresConfirmation = true
	if strings.TrimSpace(p.ConfirmationPrompt) == "" {
		p.ConfirmationPrompt = "Confirm this destructive operation before continuing."
	}
	p.ConfirmationToken = p.confirmationToken()
}

func (p DestructivePreflight) HasBlockers() bool {
	return len(p.Blockers) > 0
}

func (p DestructivePreflight) BlockerError() error {
	if len(p.Blockers) == 0 {
		return nil
	}
	first := p.Blockers[0]
	if first.Message != "" {
		return fmt.Errorf("%s", first.Message)
	}
	if first.Code != "" {
		return fmt.Errorf("%s", first.Code)
	}
	return fmt.Errorf("destructive preflight blocked")
}

func (p DestructivePreflight) ValidateConfirmation(confirmation DestructiveConfirmation) error {
	if !confirmation.Confirmed {
		return fmt.Errorf("%s requires confirmation", p.Operation)
	}
	if strings.TrimSpace(confirmation.ConfirmationToken) == "" {
		return fmt.Errorf("%s requires confirmation token", p.Operation)
	}
	if confirmation.ConfirmationToken != p.ConfirmationToken {
		return fmt.Errorf("%s confirmation token mismatch", p.Operation)
	}
	if err := p.BlockerError(); err != nil {
		return err
	}
	return nil
}

func (p DestructivePreflight) confirmationToken() string {
	lines := []string{
		"operation=" + p.Operation,
		"profile=" + p.TargetProfileId,
		"userDataDir=" + p.TargetUserDataDir,
		fmt.Sprintf("requiresStop=%t", p.RequiresStop),
		fmt.Sprintf("writesCookie=%t", p.WritesCookie),
		"adds=" + strings.Join(p.Adds, "\n"),
		"overwrites=" + strings.Join(p.Overwrites, "\n"),
		"skips=" + strings.Join(p.Skips, "\n"),
		"destructivePaths=" + strings.Join(p.DestructivePaths, "\n"),
	}
	for _, issue := range p.Warnings {
		lines = append(lines, "warn="+issue.Code+":"+issue.Message)
	}
	for _, issue := range p.Blockers {
		lines = append(lines, "block="+issue.Code+":"+issue.Message)
	}
	sum := sha256.Sum256([]byte(strings.Join(lines, "\n")))
	return hex.EncodeToString(sum[:])
}

func sortedUniqueStrings(items []string) []string {
	seen := map[string]struct{}{}
	out := make([]string, 0, len(items))
	for _, item := range items {
		item = strings.TrimSpace(item)
		if item == "" {
			continue
		}
		key := strings.ToLower(item)
		if _, ok := seen[key]; ok {
			continue
		}
		seen[key] = struct{}{}
		out = append(out, item)
	}
	sort.Strings(out)
	return out
}
