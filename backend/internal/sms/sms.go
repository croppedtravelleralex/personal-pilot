package sms

import (
	"context"
	"fmt"
	"regexp"
	"sync"
	"time"
)

type NumberStatus string

const (
	StatusPending  NumberStatus = "PENDING"
	StatusReceived NumberStatus = "RECEIVED"
	StatusTimeout  NumberStatus = "TIMEOUT"
	StatusCanceled NumberStatus = "CANCELED"
)

type BuyRequest struct {
	Country  string
	Service  string
	Operator string
	MaxPrice float64
}

type Number struct {
	ID        string
	Phone     string
	PhoneCC   string
	PhoneNum  string
	Country   string
	Service   string
	Operator  string
	Price     float64
	Status    NumberStatus
	ExpiresAt time.Time
	CreatedAt time.Time
}

type SMSData struct {
	Code       string
	Text       string
	Sender     string
	ReceivedAt time.Time
}

type SMSResult struct {
	Status NumberStatus
	SMS    *SMSData
}

type Provider interface {
	Name() string
	BuyNumber(ctx context.Context, req *BuyRequest) (*Number, error)
	CheckSMS(ctx context.Context, orderID string) (*SMSResult, error)
	Cancel(ctx context.Context, orderID string) error
	Finish(ctx context.Context, orderID string) error
	GetBalance(ctx context.Context) (float64, error)
	GetPrices(ctx context.Context, country, service string) (float64, error)
}

type Config struct {
	PrimaryProvider  string        `yaml:"primary_provider"`
	PrimaryAPIKey    string        `yaml:"primary_api_key"`
	FallbackProvider string        `yaml:"fallback_provider"`
	FallbackAPIKey   string        `yaml:"fallback_api_key"`
	MinBalance       float64       `yaml:"min_balance"`
	PollInterval     time.Duration `yaml:"poll_interval"`
	PollTimeout      time.Duration `yaml:"poll_timeout"`
	PrefetchCount    int           `yaml:"prefetch_count"`
}

type Metrics struct {
	mu           sync.Mutex
	TotalCount   int
	SuccessCount int
	FailCount    int
	TotalCost    float64
}

func (m *Metrics) Snapshot() (total, success, fail int, cost float64) {
	m.mu.Lock()
	defer m.mu.Unlock()
	return m.TotalCount, m.SuccessCount, m.FailCount, m.TotalCost
}

type Blacklist struct {
	mu   sync.Mutex
	nums map[string]bool
}

func NewBlacklist() *Blacklist {
	return &Blacklist{nums: make(map[string]bool)}
}

func (b *Blacklist) Contains(phone string) bool {
	b.mu.Lock()
	defer b.mu.Unlock()
	return b.nums[phone]
}

func (b *Blacklist) Add(phone string) {
	b.mu.Lock()
	defer b.mu.Unlock()
	b.nums[phone] = true
}

func (b *Blacklist) Remove(phone string) {
	b.mu.Lock()
	defer b.mu.Unlock()
	delete(b.nums, phone)
}

func (b *Blacklist) List() []string {
	b.mu.Lock()
	defer b.mu.Unlock()
	result := make([]string, 0, len(b.nums))
	for p := range b.nums {
		result = append(result, p)
	}
	return result
}

type Manager struct {
	providers []Provider
	blacklist *Blacklist
	pool      *NumberPool
	metrics   *Metrics
	config    *Config
}

func NewManager(config *Config) *Manager {
	prefetch := config.PrefetchCount
	if prefetch <= 0 {
		prefetch = 3
	}
	return &Manager{
		blacklist: NewBlacklist(),
		pool:      NewNumberPool(prefetch),
		metrics:   &Metrics{},
		config:    config,
	}
}

func (m *Manager) AddProvider(p Provider) {
	m.providers = append(m.providers, p)
}

func (m *Manager) AcquireNumber(ctx context.Context, req *BuyRequest) (*Number, error) {
	if n := m.pool.Pop(req.Service); n != nil {
		return n, nil
	}

	var lastErr error
	for _, p := range m.providers {
		n, err := p.BuyNumber(ctx, req)
		if err != nil {
			lastErr = err
			continue
		}
		if m.blacklist.Contains(n.Phone) {
			p.Cancel(ctx, n.ID)
			continue
		}
		m.metrics.mu.Lock()
		m.metrics.TotalCount++
		m.metrics.TotalCost += n.Price
		m.metrics.mu.Unlock()
		return n, nil
	}
	return nil, fmt.Errorf("all providers exhausted: %w", lastErr)
}

func (m *Manager) WaitForCode(ctx context.Context, number *Number, timeout time.Duration) (*SMSResult, error) {
	pollInterval := m.config.PollInterval
	if pollInterval <= 0 {
		pollInterval = 3 * time.Second
	}
	if timeout <= 0 {
		timeout = m.config.PollTimeout
	}
	if timeout <= 0 {
		timeout = 180 * time.Second
	}

	ctx, cancel := context.WithTimeout(ctx, timeout)
	defer cancel()

	ticker := time.NewTicker(pollInterval)
	defer ticker.Stop()

	for {
		select {
		case <-ctx.Done():
			m.metrics.mu.Lock()
			m.metrics.FailCount++
			m.metrics.mu.Unlock()
			return nil, fmt.Errorf("sms timeout after %v: %w", timeout, ctx.Err())
		case <-ticker.C:
			result, err := m.checkOneProvider(number)
			if err != nil {
				continue
			}
			if result.Status == StatusReceived && result.SMS != nil && result.SMS.Code != "" {
				m.metrics.mu.Lock()
				m.metrics.SuccessCount++
				m.metrics.mu.Unlock()
				return result, nil
			}
		}
	}
}

func (m *Manager) checkOneProvider(number *Number) (*SMSResult, error) {
	for _, p := range m.providers {
		result, err := p.CheckSMS(context.Background(), number.ID)
		if err != nil {
			continue
		}
		return result, nil
	}
	return nil, fmt.Errorf("no provider available")
}

func (m *Manager) ReleaseNumber(ctx context.Context, number *Number) error {
	var lastErr error
	for _, p := range m.providers {
		if err := p.Cancel(ctx, number.ID); err == nil {
			return nil
		} else {
			lastErr = err
		}
	}
	return lastErr
}

func (m *Manager) FinishNumber(ctx context.Context, number *Number) error {
	var lastErr error
	for _, p := range m.providers {
		if err := p.Finish(ctx, number.ID); err == nil {
			return nil
		} else {
			lastErr = err
		}
	}
	return lastErr
}

func (m *Manager) GetBalance(ctx context.Context) (float64, error) {
	for _, p := range m.providers {
		bal, err := p.GetBalance(ctx)
		if err == nil {
			return bal, nil
		}
	}
	return 0, fmt.Errorf("no provider available for balance check")
}

func (m *Manager) CheckSMS(ctx context.Context, orderID string) (*SMSResult, error) {
	for _, p := range m.providers {
		result, err := p.CheckSMS(ctx, orderID)
		if err != nil {
			continue
		}
		return result, nil
	}
	return nil, fmt.Errorf("no provider available")
}

func (m *Manager) CancelByID(ctx context.Context, orderID string) error {
	var lastErr error
	for _, p := range m.providers {
		if err := p.Cancel(ctx, orderID); err == nil {
			return nil
		} else {
			lastErr = err
		}
	}
	return lastErr
}

func (m *Manager) Config() *Config {
	return m.config
}

func (m *Manager) Providers() []Provider {
	return m.providers
}

func (m *Manager) Blacklist() *Blacklist {
	return m.blacklist
}

func (m *Manager) Metrics() *Metrics {
	return m.metrics
}

type NumberPool struct {
	mu       sync.Mutex
	pool     map[string][]*Number
	prefetch int
}

func NewNumberPool(prefetch int) *NumberPool {
	return &NumberPool{
		pool:     make(map[string][]*Number),
		prefetch: prefetch,
	}
}

func (p *NumberPool) Push(service string, n *Number) {
	p.mu.Lock()
	defer p.mu.Unlock()
	p.pool[service] = append(p.pool[service], n)
}

func (p *NumberPool) Pop(service string) *Number {
	p.mu.Lock()
	defer p.mu.Unlock()
	arr := p.pool[service]
	if len(arr) == 0 {
		return nil
	}
	n := arr[0]
	p.pool[service] = arr[1:]
	return n
}

func (p *NumberPool) Size(service string) int {
	p.mu.Lock()
	defer p.mu.Unlock()
	return len(p.pool[service])
}

func (p *NumberPool) AllSizes() map[string]int {
	p.mu.Lock()
	defer p.mu.Unlock()
	result := make(map[string]int, len(p.pool))
	for svc, arr := range p.pool {
		result[svc] = len(arr)
	}
	return result
}

type OTPExtractor struct {
	patterns []*regexp.Regexp
}

func NewOTPExtractor() *OTPExtractor {
	return &OTPExtractor{
		patterns: []*regexp.Regexp{
			regexp.MustCompile(`(?i)(?:code|otp|pin|код|验证码|确认码|认証)[^:]*?[:\s]+(\d{4,8})`),
			regexp.MustCompile(`\b(\d{6})\b`),
			regexp.MustCompile(`\b(\d{4})\b`),
			regexp.MustCompile(`\b([A-Z0-9]{4,8})\b`),
		},
	}
}

func (e *OTPExtractor) Extract(text string) string {
	if text == "" {
		return ""
	}
	for _, p := range e.patterns {
		if m := p.FindStringSubmatch(text); len(m) > 1 {
			return m[1]
		}
	}
	return ""
}
