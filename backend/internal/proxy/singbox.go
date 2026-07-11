package proxy

import (
	"encoding/json"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"personal-pilot/backend/internal/apppath"
	"personal-pilot/backend/internal/config"
	"personal-pilot/backend/internal/fsutil"
	"personal-pilot/backend/internal/logger"
	"personal-pilot/backend/internal/transport"
	goruntime "runtime"
	"strings"
	"sync"
	"time"
)

// SingBoxBridge sing-box 桥接进程
const (
	singBoxBridgeIdleTTL         = 45 * time.Second
	singBoxBridgeCleanupInterval = 15 * time.Second
)

// SingBoxBridge sing-box 桥接进程
type SingBoxBridge struct {
	NodeKey    string
	Port       int
	Cmd        *exec.Cmd
	Pid        int
	Running    bool
	Stopping   bool
	LastError  string
	RefCount   int
	LastUsedAt time.Time
	SSHTunnel  *SSHTunnelRuntime
}

type SSHTunnelRuntime struct {
	Key        string
	Target     string
	RemoteHost string
	RemotePort int
	LocalPort  int
	Cmd        *exec.Cmd
	Pid        int
	Stopping   bool
	LastError  string
	mu         sync.Mutex
	waitOnce   sync.Once
	waitDone   chan struct{}
}

// SingBoxManager sing-box 桥接管理器
type SingBoxManager struct {
	Config       *config.Config
	AppRoot      string // 应用根目录，所有相对路径基于此解析
	Bridges      map[string]*SingBoxBridge
	OnBridgeDied func(key string, err error)
	mu           sync.Mutex
	stopCh       chan struct{}
	stopOnce     sync.Once
}

// NewSingBoxManager 创建 sing-box 管理器
func NewSingBoxManager(cfg *config.Config, appRoot string) *SingBoxManager {
	manager := &SingBoxManager{
		Config:  cfg,
		AppRoot: appRoot,
		Bridges: make(map[string]*SingBoxBridge),
		stopCh:  make(chan struct{}),
	}
	go manager.cleanupLoop()
	return manager
}

// EnsureBridge 确保 sing-box 桥接进程运行，返回 socks5://127.0.0.1:port
func (m *SingBoxManager) EnsureBridge(proxyConfig string, proxies []config.BrowserProxy, proxyId string) (string, error) {
	log := logger.New("SingBox")
	src := NormalizeStandardProxyScheme(proxyConfig)
	if proxyId != "" {
		for _, item := range proxies {
			if strings.EqualFold(item.ProxyId, proxyId) {
				src = NormalizeStandardProxyScheme(item.ProxyConfig)
				break
			}
		}
	}
	if src == "" {
		return "", fmt.Errorf("未找到代理节点")
	}

	src = normalizeNodeScheme(src)
	key := m.singBoxBridgeKey(src, proxies, proxyId)

	if socksURL, reused := m.tryReuseBridge(key); reused {
		log.Info("复用 sing-box 桥接", logger.F("key", key[:8]), logger.F("socks_url", socksURL))
		return socksURL, nil
	}

	binaryPath, err := m.resolveBinary()
	if err != nil {
		log.Error("sing-box 不可用", logger.F("error", err), logger.F("appRoot", m.AppRoot))
		return "", err
	}
	log.Debug("sing-box binary", logger.F("path", binaryPath))

	dnsServers := ""
	if proxyId != "" {
		for _, item := range proxies {
			if strings.EqualFold(item.ProxyId, proxyId) {
				dnsServers = strings.TrimSpace(item.DnsServers)
				break
			}
		}
	}

	const maxRetries = 5
	var lastErr error
	for attempt := 1; attempt <= maxRetries; attempt++ {
		port, releasePort, err := reservePortNumber()
		if err != nil {
			lastErr = err
			continue
		}

		outboundSource := src
		var sshTunnel *SSHTunnelRuntime
		if directive, ok, directiveErr := ParseSSHTunnelDirective(src); directiveErr != nil {
			return "", directiveErr
		} else if ok {
			sshTunnel, err = m.startSSHTunnel(key, directive)
			if err != nil {
				releasePort()
				lastErr = err
				log.Error("SSH 隧道启动失败", logger.F("target", directive.Target), logger.F("error", err), logger.F("attempt", attempt))
				time.Sleep(200 * time.Millisecond)
				continue
			}
			outboundSource, err = rewriteProxyURLToLocalForward(directive.CleanProxyURL, sshTunnel.LocalPort)
			if err != nil {
				releasePort()
				m.stopSSHTunnel(sshTunnel)
				return "", err
			}
		}

		outbound, err := BuildSingBoxOutbound(outboundSource)
		if err != nil {
			releasePort()
			if sshTunnel != nil {
				m.stopSSHTunnel(sshTunnel)
			}
			log.Error("节点解析失败", logger.F("error", err))
			return "", err
		}
		if sshTunnel != nil {
			if directive, ok, _ := ParseSSHTunnelDirective(src); ok {
				applyHTTPSProxyServerName(outbound, directive.UpstreamHost)
			}
		}

		cfgPath, err := m.buildConfig(key, outbound, port, dnsServers)
		if err != nil {
			releasePort()
			if sshTunnel != nil {
				m.stopSSHTunnel(sshTunnel)
			}
			return "", fmt.Errorf("sing-box 配置生成失败: %w", err)
		}

		cmd := exec.Command(binaryPath, "run", "-c", cfgPath)
		hideWindow(cmd)
		cmd.Dir = filepath.Dir(cfgPath)
		stderrPath := filepath.Join(filepath.Dir(cfgPath), "singbox-stderr.log")
		stderrFile, err := os.Create(stderrPath)
		if err != nil {
			log.Warn("sing-box stderr 文件创建失败", logger.F("error", err))
		} else {
			cmd.Stderr = stderrFile
			defer stderrFile.Close()
		}

		if err := cmd.Start(); err != nil {
			releasePort()
			if sshTunnel != nil {
				m.stopSSHTunnel(sshTunnel)
			}
			log.Error("sing-box 启动失败", logger.F("error", err), logger.F("attempt", attempt))
			lastErr = err
			continue
		}

		bridge := &SingBoxBridge{
			NodeKey:    key,
			Port:       port,
			Cmd:        cmd,
			Pid:        cmd.Process.Pid,
			Running:    true,
			LastUsedAt: time.Now(),
			SSHTunnel:  sshTunnel,
		}
		log.Info("sing-box 启动", logger.F("key", key[:8]), logger.F("pid", bridge.Pid), logger.F("port", port))

		if err := waitPortReady("127.0.0.1", port, 15*time.Second); err != nil {
			releasePort()
			if content, readErr := os.ReadFile(stderrPath); readErr == nil && len(content) > 0 {
				log.Error("sing-box stderr", logger.F("output", string(content)))
			}
			bridge.Stopping = true
			m.stopBridgeProcess(bridge)
			bridge.Running = false
			bridge.Pid = 0
			bridge.LastError = err.Error()
			log.Error("sing-box 端口不可用，重试", logger.F("error", err), logger.F("attempt", attempt))
			lastErr = err
			time.Sleep(200 * time.Millisecond)
			continue
		}
		releasePort()

		if socksURL, reused := m.registerBridge(key, bridge); reused {
			log.Info("复用已就绪 sing-box 桥接", logger.F("key", key[:8]), logger.F("socks_url", socksURL))
			bridge.Stopping = true
			m.stopBridgeProcess(bridge)
			return socksURL, nil
		}

		go m.watchBridge(bridge, key)
		return fmt.Sprintf("socks5://127.0.0.1:%d", port), nil
	}

	return "", fmt.Errorf("sing-box 启动失败（已重试 %d 次）: %w", maxRetries, lastErr)
}

// StopAll 关闭所有 sing-box 桥接进程
// AcquireBridge pins a sing-box bridge for a browser instance lifecycle.
func (m *SingBoxManager) AcquireBridge(proxyConfig string, proxies []config.BrowserProxy, proxyId string) (string, string, error) {
	src := NormalizeStandardProxyScheme(proxyConfig)
	if proxyId != "" {
		for _, item := range proxies {
			if strings.EqualFold(item.ProxyId, proxyId) {
				src = NormalizeStandardProxyScheme(item.ProxyConfig)
				break
			}
		}
	}
	src = normalizeNodeScheme(src)
	if src == "" {
		return "", "", fmt.Errorf("proxy node is empty")
	}

	socksURL, err := m.EnsureBridge(proxyConfig, proxies, proxyId)
	if err != nil {
		return "", "", err
	}
	key := m.singBoxBridgeKey(src, proxies, proxyId)
	if !m.pinBridge(key) {
		return "", "", fmt.Errorf("sing-box bridge exited before it could be pinned")
	}
	return socksURL, key, nil
}

func (m *SingBoxManager) singBoxBridgeKey(src string, proxies []config.BrowserProxy, proxyId string) string {
	dnsServers := ""
	if proxyId != "" {
		for _, item := range proxies {
			if strings.EqualFold(item.ProxyId, proxyId) {
				dnsServers = strings.TrimSpace(item.DnsServers)
				break
			}
		}
	}
	return computeNodeKey(src + "\x00" + dnsServers)
}

func (m *SingBoxManager) ReleaseBridge(key string) {
	key = strings.TrimSpace(key)
	if key == "" {
		return
	}

	m.mu.Lock()
	defer m.mu.Unlock()

	bridge, ok := m.Bridges[key]
	if !ok || bridge == nil {
		return
	}
	if bridge.RefCount > 0 {
		bridge.RefCount--
	}
	bridge.LastUsedAt = time.Now()
}

func (m *SingBoxManager) pinBridge(key string) bool {
	m.mu.Lock()
	defer m.mu.Unlock()

	bridge, ok := m.Bridges[key]
	if !ok || bridge == nil {
		return false
	}
	alive := bridge.Running && bridge.Cmd != nil && bridge.Cmd.Process != nil && bridge.Cmd.ProcessState == nil
	if !alive {
		return false
	}
	bridge.RefCount++
	bridge.LastUsedAt = time.Now()
	return true
}

var _ BridgeManager = (*SingBoxManager)(nil)

func (m *SingBoxManager) CanHandle(proxyConfig string) bool {
	return IsSingBoxProtocol(proxyConfig) || IsStandardAuthProxy(proxyConfig) || HasSSHTunnelDirective(proxyConfig)
}

func (m *SingBoxManager) StopAll() {
	m.stopOnce.Do(func() {
		close(m.stopCh)
	})

	m.mu.Lock()
	bridges := make([]*SingBoxBridge, 0, len(m.Bridges))
	for key, bridge := range m.Bridges {
		if bridge != nil {
			bridge.Stopping = true
			bridges = append(bridges, bridge)
		}
		delete(m.Bridges, key)
	}
	m.mu.Unlock()

	for _, bridge := range bridges {
		m.stopBridgeProcess(bridge)
	}
}

func (m *SingBoxManager) tryReuseBridge(key string) (string, bool) {
	var stale *SingBoxBridge

	m.mu.Lock()
	if bridge, ok := m.Bridges[key]; ok && bridge != nil {
		alive := bridge.Running && bridge.Cmd != nil && bridge.Cmd.Process != nil && bridge.Cmd.ProcessState == nil
		if alive && waitPortReady("127.0.0.1", bridge.Port, 800*time.Millisecond) == nil && m.isSSHTunnelReady(bridge.SSHTunnel) {
			bridge.LastUsedAt = time.Now()
			socksURL := fmt.Sprintf("socks5://127.0.0.1:%d", bridge.Port)
			m.mu.Unlock()
			return socksURL, true
		}

		bridge.Stopping = true
		stale = bridge
		delete(m.Bridges, key)
	}
	m.mu.Unlock()

	if stale != nil {
		m.stopBridgeProcess(stale)
	}
	return "", false
}

func (m *SingBoxManager) registerBridge(key string, bridge *SingBoxBridge) (string, bool) {
	var duplicate *SingBoxBridge

	m.mu.Lock()
	if existing, ok := m.Bridges[key]; ok && existing != nil {
		if existing == bridge {
			m.mu.Unlock()
			return "", false
		}

		alive := existing.Running && existing.Cmd != nil && existing.Cmd.Process != nil && existing.Cmd.ProcessState == nil
		if alive && waitPortReady("127.0.0.1", existing.Port, 800*time.Millisecond) == nil && m.isSSHTunnelReady(existing.SSHTunnel) {
			existing.LastUsedAt = time.Now()
			duplicate = bridge
			socksURL := fmt.Sprintf("socks5://127.0.0.1:%d", existing.Port)
			m.mu.Unlock()
			if duplicate != nil {
				duplicate.Stopping = true
				m.stopBridgeProcess(duplicate)
			}
			return socksURL, true
		}

		existing.Stopping = true
		delete(m.Bridges, key)
		duplicate = existing
	}
	bridge.LastUsedAt = time.Now()
	m.Bridges[key] = bridge
	m.mu.Unlock()

	if duplicate != nil {
		m.stopBridgeProcess(duplicate)
	}
	return "", false
}

func (m *SingBoxManager) startSSHTunnel(key string, directive SSHTunnelDirective) (*SSHTunnelRuntime, error) {
	localPort, releasePort, err := reservePortNumber()
	if err != nil {
		return nil, fmt.Errorf("SSH 隧道本地端口分配失败: %w", err)
	}

	args := []string{
		"-N",
		"-L", buildLocalForwardSpec(localPort, directive.RemoteHost, directive.RemotePort),
		"-o", "ExitOnForwardFailure=yes",
		"-o", "ServerAliveInterval=30",
		"-o", "ServerAliveCountMax=3",
		"-o", "BatchMode=yes",
	}
	if directive.SSHPort > 0 {
		args = append(args, "-p", fmt.Sprintf("%d", directive.SSHPort))
	}
	args = append(args, directive.Target)

	cmd := exec.Command("ssh", args...)
	hideWindow(cmd)

	runtime := &SSHTunnelRuntime{
		Key:        key,
		Target:     directive.Target,
		RemoteHost: directive.RemoteHost,
		RemotePort: directive.RemotePort,
		LocalPort:  localPort,
		Cmd:        cmd,
		waitDone:   make(chan struct{}),
	}

	if err := cmd.Start(); err != nil {
		releasePort()
		return nil, fmt.Errorf("SSH 隧道启动失败: %w", err)
	}
	runtime.Pid = cmd.Process.Pid

	go runtime.wait()

	if err := waitPortReady("127.0.0.1", localPort, 15*time.Second); err != nil {
		runtime.setLastError(err)
		m.stopSSHTunnel(runtime)
		releasePort()
		return nil, fmt.Errorf("SSH 隧道端口未就绪: %w", err)
	}
	releasePort()

	logger.New("SingBox").Info("SSH 隧道启动",
		logger.F("target", directive.Target),
		logger.F("remote_host", directive.RemoteHost),
		logger.F("remote_port", directive.RemotePort),
		logger.F("local_port", localPort),
		logger.F("pid", runtime.Pid),
	)
	return runtime, nil
}

func (t *SSHTunnelRuntime) wait() {
	if t == nil || t.Cmd == nil {
		return
	}
	err := t.Cmd.Wait()
	t.mu.Lock()
	if err != nil && !t.Stopping {
		t.LastError = err.Error()
	}
	t.mu.Unlock()
	t.waitOnce.Do(func() {
		close(t.waitDone)
	})
}

func (t *SSHTunnelRuntime) setLastError(err error) {
	if t == nil || err == nil {
		return
	}
	t.mu.Lock()
	t.LastError = err.Error()
	t.mu.Unlock()
}

func (m *SingBoxManager) stopSSHTunnel(tunnel *SSHTunnelRuntime) {
	if tunnel == nil {
		return
	}

	tunnel.mu.Lock()
	tunnel.Stopping = true
	cmd := tunnel.Cmd
	tunnel.mu.Unlock()

	if cmd != nil && cmd.Process != nil && cmd.ProcessState == nil {
		_ = cmd.Process.Kill()
	}

	if tunnel.waitDone != nil {
		select {
		case <-tunnel.waitDone:
		case <-time.After(2 * time.Second):
		}
	}
}

func (m *SingBoxManager) isSSHTunnelReady(tunnel *SSHTunnelRuntime) bool {
	if tunnel == nil {
		return true
	}
	tunnel.mu.Lock()
	cmd := tunnel.Cmd
	localPort := tunnel.LocalPort
	stopping := tunnel.Stopping
	tunnel.mu.Unlock()
	if stopping || cmd == nil || cmd.Process == nil || cmd.ProcessState != nil || localPort <= 0 {
		return false
	}
	return waitPortReady("127.0.0.1", localPort, 800*time.Millisecond) == nil
}

func applyHTTPSProxyServerName(outbound map[string]interface{}, upstreamHost string) {
	upstreamHost = strings.TrimSpace(upstreamHost)
	if outbound == nil || upstreamHost == "" {
		return
	}
	if !strings.EqualFold(fmt.Sprint(outbound["type"]), "http") {
		return
	}
	tls, _ := outbound["tls"].(map[string]interface{})
	if tls == nil {
		return
	}
	if enabled, ok := tls["enabled"].(bool); ok && !enabled {
		return
	}
	tls["server_name"] = upstreamHost
	outbound["tls"] = tls
}

func (m *SingBoxManager) watchBridge(bridge *SingBoxBridge, key string) {
	if bridge == nil || bridge.Cmd == nil {
		return
	}
	_ = bridge.Cmd.Wait()

	m.mu.Lock()
	if current, ok := m.Bridges[key]; ok && current == bridge {
		delete(m.Bridges, key)
	}
	bridge.Running = false
	stopping := bridge.Stopping
	m.mu.Unlock()

	if !stopping && m.OnBridgeDied != nil {
		m.OnBridgeDied(key, fmt.Errorf("sing-box 桥接进程意外退出"))
	}
	m.stopSSHTunnel(bridge.SSHTunnel)
}

func (m *SingBoxManager) stopBridgeProcess(bridge *SingBoxBridge) {
	if bridge == nil || bridge.Cmd == nil || bridge.Cmd.Process == nil {
		if bridge != nil {
			m.stopSSHTunnel(bridge.SSHTunnel)
		}
		return
	}
	_ = bridge.Cmd.Process.Kill()
	m.stopSSHTunnel(bridge.SSHTunnel)
}

// cleanupLoop 定期回收空闲桥接进程
func (m *SingBoxManager) cleanupLoop() {
	ticker := time.NewTicker(singBoxBridgeCleanupInterval)
	defer ticker.Stop()

	for {
		select {
		case <-ticker.C:
			m.recycleIdleBridges()
		case <-m.stopCh:
			return
		}
	}
}

// recycleIdleBridges 回收引用计数为 0 且超过空闲 TTL 的桥接进程
func (m *SingBoxManager) recycleIdleBridges() {
	now := time.Now()
	var stale []*SingBoxBridge

	m.mu.Lock()
	for key, bridge := range m.Bridges {
		if bridge == nil {
			delete(m.Bridges, key)
			continue
		}
		if bridge.RefCount > 0 {
			continue
		}
		if now.Sub(bridge.LastUsedAt) < singBoxBridgeIdleTTL {
			continue
		}

		bridge.Stopping = true
		stale = append(stale, bridge)
		delete(m.Bridges, key)
	}
	m.mu.Unlock()

	if len(stale) == 0 {
		return
	}

	log := logger.New("SingBox")
	for _, bridge := range stale {
		log.Info("回收空闲桥接进程", logger.F("key", bridge.NodeKey), logger.F("pid", bridge.Pid))
		m.stopBridgeProcess(bridge)
	}
}

func (m *SingBoxManager) resolveBinary() (string, error) {
	configPath := strings.TrimSpace(m.Config.Browser.SingBoxBinaryPath)
	if configPath != "" {
		resolved := resolveEnvPath(configPath, m.AppRoot)
		if resolved != "" {
			if _, err := os.Stat(resolved); err == nil {
				if err := fsutil.EnsureExecutable(resolved); err != nil {
					return "", fmt.Errorf("sing-box 文件不可执行: %s: %w", resolved, err)
				}
				return resolved, nil
			}
		}
	}
	if env := strings.TrimSpace(os.Getenv("SINGBOX_BINARY_PATH")); env != "" {
		if _, err := os.Stat(env); err == nil {
			if err := fsutil.EnsureExecutable(env); err != nil {
				return "", fmt.Errorf("sing-box 文件不可执行: %s: %w", env, err)
			}
			return env, nil
		}
	}

	binaryNames := []string{"sing-box"}
	if goruntime.GOOS == "windows" {
		binaryNames = []string{"sing-box.exe", "sing-box"}
	}
	platformDir := fmt.Sprintf("%s-%s", goruntime.GOOS, goruntime.GOARCH)

	searchDirs := make([]string, 0, 4)
	if m.AppRoot != "" {
		searchDirs = append(searchDirs,
			filepath.Join(m.AppRoot, "bin", platformDir),
			filepath.Join(m.AppRoot, "bin"),
		)
	}
	if exePath, err := os.Executable(); err == nil {
		exeDir := filepath.Dir(exePath)
		searchDirs = append(searchDirs,
			filepath.Join(exeDir, "bin", platformDir),
			filepath.Join(exeDir, "bin"),
		)
	}

	for _, dir := range searchDirs {
		for _, name := range binaryNames {
			candidate := filepath.Join(dir, name)
			if _, err := os.Stat(candidate); err == nil {
				if err := fsutil.EnsureExecutable(candidate); err != nil {
					return "", fmt.Errorf("sing-box 文件不可执行: %s: %w", candidate, err)
				}
				return candidate, nil
			}
		}
	}

	for _, name := range binaryNames {
		if path, err := exec.LookPath(name); err == nil {
			if err := fsutil.EnsureExecutable(path); err != nil {
				return "", fmt.Errorf("sing-box 文件不可执行: %s: %w", path, err)
			}
			return path, nil
		}
	}

	return "", fmt.Errorf("未找到 sing-box 可执行文件。请将 sing-box 放到 bin/%s/ 或 bin/ 目录，或在配置中设置 SingBoxBinaryPath", platformDir)
}

func (m *SingBoxManager) buildConfig(key string, outbound map[string]interface{}, port int, dnsServers string) (string, error) {
	baseDir := m.resolveWorkdir(key)
	if err := os.MkdirAll(baseDir, 0755); err != nil {
		return "", err
	}

	transportProfile := transport.RuntimeProfile(transport.RuntimeFamilyChrome)
	outbound = applySingBoxTransportProfile(outbound, transportProfile)
	cfg := map[string]interface{}{
		"log": map[string]interface{}{
			"level":     "info",
			"output":    filepath.Join(baseDir, "singbox.log"),
			"timestamp": true,
		},
		"inbounds": []interface{}{
			map[string]interface{}{
				"type":        "socks",
				"tag":         "socks-in",
				"listen":      "127.0.0.1",
				"listen_port": port,
			},
		},
		"outbounds": []interface{}{
			outbound,
			map[string]interface{}{
				"type": "direct",
				"tag":  "direct",
			},
		},
		"route": map[string]interface{}{
			"rules": []interface{}{
				map[string]interface{}{
					"inbound":  []string{"socks-in"},
					"outbound": "proxy-out",
				},
			},
		},
	}
	if dnsBlock := buildSingBoxDNS(dnsServers); dnsBlock != nil {
		cfg["dns"] = dnsBlock
	}

	data, err := json.MarshalIndent(cfg, "", "  ")
	if err != nil {
		return "", err
	}

	cfgPath := filepath.Join(baseDir, "singbox-config.json")
	if err := os.WriteFile(cfgPath, data, 0600); err != nil {
		return "", err
	}
	return cfgPath, nil
}

func buildSingBoxDNS(raw string) map[string]interface{} {
	raw = strings.TrimSpace(raw)
	parts := make([]string, 0)
	if raw != "" {
		for _, part := range strings.Split(raw, ",") {
			part = strings.TrimSpace(part)
			if part != "" {
				parts = append(parts, part)
			}
		}
	}
	if len(parts) == 0 {
		// Bridge-only configs do not need an explicit DNS block; sing-box 1.12+
		// rejects legacy string server entries.
		return nil
	}

	servers := make([]interface{}, 0, len(parts))
	for i, part := range parts {
		entry, ok := singBoxDNSServerEntry(part, i)
		if !ok {
			continue
		}
		servers = append(servers, entry)
	}
	if len(servers) == 0 {
		return nil
	}
	finalTag := "remote"
	if first, ok := servers[0].(map[string]interface{}); ok {
		if tag, _ := first["tag"].(string); tag != "" {
			finalTag = tag
		}
	}
	return map[string]interface{}{
		"servers": servers,
		"final":   finalTag,
	}
}

func singBoxDNSServerEntry(raw string, index int) (map[string]interface{}, bool) {
	raw = strings.TrimSpace(raw)
	if raw == "" {
		return nil, false
	}
	tag := fmt.Sprintf("dns-%d", index+1)
	if index == 0 {
		tag = "remote"
	}

	lower := strings.ToLower(raw)
	switch {
	case lower == "local":
		return map[string]interface{}{"type": "local", "tag": tag}, true
	case strings.HasPrefix(lower, "https://"):
		host := strings.TrimPrefix(strings.TrimPrefix(raw, "https://"), "https://")
		if idx := strings.Index(host, "/"); idx >= 0 {
			host = host[:idx]
		}
		host = strings.Trim(host, "/")
		if host == "" {
			return nil, false
		}
		return map[string]interface{}{"type": "https", "tag": tag, "server": host}, true
	case strings.HasPrefix(lower, "tls://"):
		host := strings.TrimPrefix(raw, "tls://")
		host = strings.TrimPrefix(host, "TLS://")
		if host == "" {
			return nil, false
		}
		return map[string]interface{}{"type": "tls", "tag": tag, "server": host}, true
	case strings.HasPrefix(lower, "tcp://"):
		host := strings.TrimPrefix(raw, "tcp://")
		host = strings.TrimPrefix(host, "TCP://")
		if host == "" {
			return nil, false
		}
		return map[string]interface{}{"type": "tcp", "tag": tag, "server": host}, true
	case strings.HasPrefix(lower, "udp://"):
		host := strings.TrimPrefix(raw, "udp://")
		host = strings.TrimPrefix(host, "UDP://")
		if host == "" {
			return nil, false
		}
		return map[string]interface{}{"type": "udp", "tag": tag, "server": host}, true
	default:
		return map[string]interface{}{"type": "udp", "tag": tag, "server": raw}, true
	}
}

func applySingBoxTransportProfile(outbound map[string]interface{}, profile transport.OutboundConfig) map[string]interface{} {
	if outbound == nil {
		outbound = map[string]interface{}{}
	}
	if len(profile.TLS.ALPN) > 0 {
		tls, _ := outbound["tls"].(map[string]interface{})
		if tls != nil {
			tls["alpn"] = append([]string{}, profile.TLS.ALPN...)
			outbound["tls"] = tls
		}
	}
	return outbound
}

func (m *SingBoxManager) resolveWorkdir(key string) string {
	root := strings.TrimSpace(m.Config.Browser.UserDataRoot)
	if root == "" {
		root = "data"
	}
	if !filepath.IsAbs(root) {
		root = apppath.Resolve(m.AppRoot, root)
	}
	return filepath.Join(root, "_singbox", key)
}
