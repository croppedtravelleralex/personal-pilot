package captcha

import (
	"context"
	"crypto/sha256"
	"encoding/hex"
	"fmt"
	"sync"
	"time"
)

type CaptchaType string

const (
	CaptchaImage     CaptchaType = "image"
	CaptchaReCaptcha CaptchaType = "recaptcha"
	CaptchaHCaptcha  CaptchaType = "hcaptcha"
	CaptchaTurnstile CaptchaType = "turnstile"
	CaptchaGeeTest   CaptchaType = "geetest"
	CaptchaFun       CaptchaType = "funcaptcha"
)

type SolveRequest struct {
	Type      CaptchaType
	ImageData []byte
	ImageURL  string
	SiteKey   string
	PageURL   string
	Proxy     string
	UserAgent string
	Options   map[string]any
	Timeout   time.Duration
}

type SolveResult struct {
	Text     string
	Token    string
	SolvedAt time.Time
	Cost     float64
	Provider string
}

type Solver interface {
	Name() string
	Solve(ctx context.Context, req *SolveRequest) (*SolveResult, error)
	GetBalance(ctx context.Context) (float64, error)
}

type Config struct {
	PrimaryProvider  string        `yaml:"primary_provider"`
	PrimaryAPIKey    string        `yaml:"primary_api_key"`
	FallbackProvider string        `yaml:"fallback_provider"`
	FallbackAPIKey   string        `yaml:"fallback_api_key"`
	UseLocalOCR      bool          `yaml:"use_local_ocr"`
	CacheTTL         time.Duration `yaml:"cache_ttl"`
	MaxRetries       int           `yaml:"max_retries"`
	Timeout          time.Duration `yaml:"timeout"`
}

type Manager struct {
	solvers []Solver
	cache   *resultCache
	config  *Config
	metrics *Metrics
}

type Metrics struct {
	mu          sync.Mutex
	TotalSolved int
	TotalFailed int
	TotalCost   float64
	ByProvider  map[string]*ProviderStats
}

type ProviderStats struct {
	Solved int
	Failed int
	Cost   float64
}

func NewManager(config *Config) *Manager {
	m := &Manager{
		cache:   newResultCache(config.CacheTTL),
		config:  config,
		metrics: &Metrics{ByProvider: make(map[string]*ProviderStats)},
	}
	return m
}

func (m *Manager) AddSolver(s Solver) {
	m.solvers = append(m.solvers, s)
}

func (m *Manager) Solve(ctx context.Context, req *SolveRequest) (*SolveResult, error) {
	cacheKey := cacheKeyFromRequest(req)
	if cached, ok := m.cache.Get(cacheKey); ok {
		return cached, nil
	}

	var lastErr error
	for _, solver := range m.solvers {
		result, err := solver.Solve(ctx, req)
		if err == nil {
			m.cache.Set(cacheKey, result)
			m.recordSuccess(solver.Name(), result.Cost)
			return result, nil
		}
		lastErr = err
		m.recordFailure(solver.Name())
	}
	return nil, fmt.Errorf("all solvers failed: %w", lastErr)
}

func (m *Manager) Solvers() []Solver {
	return m.solvers
}

func (m *Manager) Config() *Config {
	return m.config
}

func (m *Manager) recordSuccess(provider string, cost float64) {
	m.metrics.mu.Lock()
	defer m.metrics.mu.Unlock()
	m.metrics.TotalSolved++
	m.metrics.TotalCost += cost
	s := m.metrics.ByProvider[provider]
	if s == nil {
		s = &ProviderStats{}
		m.metrics.ByProvider[provider] = s
	}
	s.Solved++
	s.Cost += cost
}

func (m *Manager) recordFailure(provider string) {
	m.metrics.mu.Lock()
	defer m.metrics.mu.Unlock()
	m.metrics.TotalFailed++
	s := m.metrics.ByProvider[provider]
	if s == nil {
		s = &ProviderStats{}
		m.metrics.ByProvider[provider] = s
	}
	s.Failed++
}

func (m *Manager) GetMetrics() *Metrics {
	m.metrics.mu.Lock()
	defer m.metrics.mu.Unlock()
	cp := &Metrics{
		TotalSolved: m.metrics.TotalSolved,
		TotalFailed: m.metrics.TotalFailed,
		TotalCost:   m.metrics.TotalCost,
		ByProvider:  make(map[string]*ProviderStats),
	}
	for k, v := range m.metrics.ByProvider {
		sv := *v
		cp.ByProvider[k] = &sv
	}
	return cp
}

type resultCache struct {
	mu      sync.Mutex
	entries map[string]*cacheEntry
	ttl     time.Duration
}

type cacheEntry struct {
	result    *SolveResult
	expiresAt time.Time
}

func newResultCache(ttl time.Duration) *resultCache {
	if ttl <= 0 {
		ttl = 60 * time.Second
	}
	return &resultCache{
		entries: make(map[string]*cacheEntry),
		ttl:     ttl,
	}
}

func (c *resultCache) Get(key string) (*SolveResult, bool) {
	c.mu.Lock()
	defer c.mu.Unlock()
	e, ok := c.entries[key]
	if !ok || time.Now().After(e.expiresAt) {
		delete(c.entries, key)
		return nil, false
	}
	return e.result, true
}

func (c *resultCache) Set(key string, result *SolveResult) {
	c.mu.Lock()
	defer c.mu.Unlock()
	c.entries[key] = &cacheEntry{
		result:    result,
		expiresAt: time.Now().Add(c.ttl),
	}
}

func cacheKeyFromRequest(req *SolveRequest) string {
	h := sha256.New()
	h.Write([]byte(req.SiteKey))
	h.Write([]byte(req.PageURL))
	h.Write([]byte(string(req.Type)))
	if len(req.ImageData) > 0 {
		h.Write(req.ImageData)
	}
	return hex.EncodeToString(h.Sum(nil))
}
