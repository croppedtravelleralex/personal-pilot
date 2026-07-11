package platformpack

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"time"

	"gopkg.in/yaml.v3"
)

// CadenceConfig is the nurture/login budget loaded from platform-packs/*/cadence.yaml.
type CadenceConfig struct {
	Nurture CadenceNurture `yaml:"nurture" json:"nurture"`
	Login   CadenceLogin   `yaml:"login" json:"login"`
	Source  string         `yaml:"-" json:"source,omitempty"`
}

type CadenceNurture struct {
	SessionMinutes       []int `yaml:"sessionMinutes" json:"sessionMinutes"`
	ScrollPauseMs        []int `yaml:"scrollPauseMs" json:"scrollPauseMs"`
	MaxActionsPerSession int   `yaml:"maxActionsPerSession" json:"maxActionsPerSession"`
}

type CadenceLogin struct {
	OTPWaitSeconds int `yaml:"otpWaitSeconds" json:"otpWaitSeconds"`
}

// LoadCadence reads platform-packs/<pack>/cadence.yaml relative to appRoot.
func LoadCadence(appRoot, pack string) (CadenceConfig, error) {
	pack = strings.TrimSpace(pack)
	if pack == "" {
		pack = "xhs"
	}
	path := filepath.Join(appRoot, "platform-packs", pack, "cadence.yaml")
	data, err := os.ReadFile(path)
	if err != nil {
		return CadenceConfig{}, fmt.Errorf("load cadence %s: %w", path, err)
	}
	var cfg CadenceConfig
	if err := yaml.Unmarshal(data, &cfg); err != nil {
		return CadenceConfig{}, fmt.Errorf("parse cadence %s: %w", path, err)
	}
	cfg.Source = path
	return cfg, nil
}

// ApplyToBudget clamps pageBudget and session duration using nurture limits.
func (c CadenceConfig) ApplyToBudget(pageBudget uint32, minDur, maxDur time.Duration) (uint32, time.Duration, time.Duration) {
	if c.Nurture.MaxActionsPerSession > 0 && pageBudget > uint32(c.Nurture.MaxActionsPerSession) {
		pageBudget = uint32(c.Nurture.MaxActionsPerSession)
	}
	if len(c.Nurture.SessionMinutes) >= 2 {
		lo := c.Nurture.SessionMinutes[0]
		hi := c.Nurture.SessionMinutes[1]
		if lo > 0 {
			minDur = time.Duration(lo) * time.Minute
		}
		if hi > lo {
			maxDur = time.Duration(hi) * time.Minute
		}
	} else if len(c.Nurture.SessionMinutes) == 1 && c.Nurture.SessionMinutes[0] > 0 {
		minDur = time.Duration(c.Nurture.SessionMinutes[0]) * time.Minute
		maxDur = minDur + 2*time.Minute
	}
	return pageBudget, minDur, maxDur
}

// ScrollPauseRange returns nurture scroll pause bounds in milliseconds.
func (c CadenceConfig) ScrollPauseRange() (minMs, maxMs int) {
	if len(c.Nurture.ScrollPauseMs) >= 2 {
		return c.Nurture.ScrollPauseMs[0], c.Nurture.ScrollPauseMs[1]
	}
	if len(c.Nurture.ScrollPauseMs) == 1 {
		return c.Nurture.ScrollPauseMs[0], c.Nurture.ScrollPauseMs[0]
	}
	return 1200, 2800
}
