package browser

import (
	"context"
	"sync"
	"time"
)

// ExitIPCheckFunc performs an exit IP health check for a running profile proxy.
type ExitIPCheckFunc func(profileID, proxyID string)

// ExitIPDriftHandler is invoked when a running instance's proxy exit IP changes.
type ExitIPDriftHandler func(profileID, proxyID, oldIP, newIP string)

// ProxyIPMonitor periodically re-checks exit IPs for running browser instances.
type ProxyIPMonitor struct {
	mu       sync.Mutex
	interval time.Duration
	stopCh   chan struct{}
	doneCh   chan struct{}
	checkFn  ExitIPCheckFunc
	onDrift  ExitIPDriftHandler
	lastIP   map[string]string // proxyId -> ip
}

// NewProxyIPMonitor creates a monitor with the given interval (recommended 60s).
func NewProxyIPMonitor(interval time.Duration, checkFn ExitIPCheckFunc, onDrift ExitIPDriftHandler) *ProxyIPMonitor {
	if interval <= 0 {
		interval = 60 * time.Second
	}
	return &ProxyIPMonitor{
		interval: interval,
		checkFn:  checkFn,
		onDrift:  onDrift,
		lastIP:   make(map[string]string),
	}
}

func (m *ProxyIPMonitor) Start(ctx context.Context, listRunning func() []RunningProfileProxy) {
	if m == nil || m.checkFn == nil {
		return
	}
	m.mu.Lock()
	if m.stopCh != nil {
		m.mu.Unlock()
		return
	}
	m.stopCh = make(chan struct{})
	m.doneCh = make(chan struct{})
	m.mu.Unlock()

	go func() {
		defer close(m.doneCh)
		ticker := time.NewTicker(m.interval)
		defer ticker.Stop()
		for {
			select {
			case <-ctx.Done():
				return
			case <-m.stopCh:
				return
			case <-ticker.C:
				m.runOnce(listRunning)
			}
		}
	}()
}

func (m *ProxyIPMonitor) Stop() {
	if m == nil {
		return
	}
	m.mu.Lock()
	stopCh := m.stopCh
	doneCh := m.doneCh
	m.stopCh = nil
	m.doneCh = nil
	m.mu.Unlock()
	if stopCh != nil {
		close(stopCh)
	}
	if doneCh != nil {
		<-doneCh
	}
}

func (m *ProxyIPMonitor) runOnce(listRunning func() []RunningProfileProxy) {
	if listRunning == nil {
		return
	}
	items := listRunning()
	const maxConcurrent = 5
	sem := make(chan struct{}, maxConcurrent)
	var wg sync.WaitGroup
	for _, item := range items {
		if item.ProxyID == "" || item.ProfileID == "" {
			continue
		}
		wg.Add(1)
		sem <- struct{}{}
		go func(profileID, proxyID string) {
			defer wg.Done()
			defer func() { <-sem }()
			m.checkFn(profileID, proxyID)
		}(item.ProfileID, item.ProxyID)
	}
	wg.Wait()
}

// NoteExitIP records the latest exit IP for drift detection on subsequent checks.
func (m *ProxyIPMonitor) NoteExitIP(proxyID, ip string) (oldIP string, changed bool) {
	if m == nil {
		return "", false
	}
	m.mu.Lock()
	defer m.mu.Unlock()
	oldIP = m.lastIP[proxyID]
	if ip != "" && oldIP != "" && oldIP != ip {
		changed = true
		if m.onDrift != nil {
			m.onDrift("", proxyID, oldIP, ip)
		}
	}
	if ip != "" {
		m.lastIP[proxyID] = ip
	}
	return oldIP, changed
}

// RunningProfileProxy links a running profile to its bound proxy.
type RunningProfileProxy struct {
	ProfileID string
	ProxyID   string
}
