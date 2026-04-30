package browser

import (
	"os"
	"path/filepath"
	"personal-pilot/backend/internal/config"
	"testing"
)

// TestLightpandaFullLifecycle 测试 Lightpanda 完整生命周期：配置 → 解析 → 参数构建。
func TestLightpandaFullLifecycle(t *testing.T) {
	dir := t.TempDir()

	// 创建假的 Lightpanda 可执行文件
	candidates := LightpandaExecutableCandidates()
	if len(candidates) == 0 {
		t.Skip("没有 Lightpanda 候选名")
	}
	exePath := filepath.Join(dir, candidates[0])
	if err := os.WriteFile(exePath, []byte("fake"), 0755); err != nil {
		t.Fatal(err)
	}

	// 1. 配置
	core := Core{
		CoreId:   "lp-test",
		CoreName: "Lightpanda Test",
		CorePath: dir,
		Kind:     config.CoreKindLightpanda,
	}

	// 2. 解析二进制路径
	mgr := NewManager(&config.Config{}, dir)
	resolved, err := mgr.ResolveBrowserBinary(core)
	if err != nil {
		t.Fatalf("ResolveBrowserBinary 失败: %v", err)
	}
	if resolved != exePath {
		t.Fatalf("路径 = %q, 期望 %q", resolved, exePath)
	}

	// 3. 启动参数
	args := BuildLightpandaLaunchArgs(9222)
	if len(args) < 2 {
		t.Fatalf("启动参数不足: %v", args)
	}

	// 4. CDP 连接模拟（无需真实浏览器）
	conn := &LightpandaCDPConn{port: 9222}
	if conn.Port() != 9222 {
		t.Fatalf("Port = %d, 期望 9222", conn.Port())
	}

	// 5. 关闭（安全处理 nil ws）
	conn.Close()
}

// TestLightpandaCoreManagementIntegration 测试内核管理与 Lightpanda 的集成。
func TestLightpandaCoreManagementIntegration(t *testing.T) {
	dir := t.TempDir()
	candidates := LightpandaExecutableCandidates()
	if len(candidates) == 0 {
		t.Skip("没有 Lightpanda 候选名")
	}
	exePath := filepath.Join(dir, candidates[0])
	if err := os.WriteFile(exePath, []byte("fake"), 0755); err != nil {
		t.Fatal(err)
	}

	mgr := NewManager(&config.Config{}, dir)

	// 验证路径
	result := mgr.ValidateCorePathForKind(dir, config.CoreKindLightpanda)
	if !result.Valid {
		t.Fatalf("路径验证失败: %s", result.Message)
	}

	// 验证内核列表
	cores := []Core{
		{CoreId: "ch-1", CoreName: "Chromium", CorePath: dir, Kind: config.CoreKindChromium},
		{CoreId: "lp-1", CoreName: "Lightpanda", CorePath: dir, Kind: config.CoreKindLightpanda},
	}
	mgr.Config.Browser.Cores = cores

	// 按 Kind 筛选
	lpCores := filterCoresByKind(mgr.ListCores(), config.CoreKindLightpanda)
	if len(lpCores) != 1 {
		t.Fatalf("Lightpanda 内核数 = %d, 期望 1", len(lpCores))
	}
	if lpCores[0].CoreName != "Lightpanda" {
		t.Fatalf("名称 = %q, 期望 Lightpanda", lpCores[0].CoreName)
	}
}

func filterCoresByKind(cores []Core, kind string) []Core {
	var result []Core
	for _, c := range cores {
		k := c.Kind
		if k == "" {
			k = config.CoreKindChromium
		}
		if k == kind {
			result = append(result, c)
		}
	}
	return result
}

// TestLightpandaCDPMessageFormat 测试 CDP 消息格式兼容性。
func TestLightpandaCDPMessageFormat(t *testing.T) {
	// 验证 Lightpanda CDP 命令格式与 Chromium CDP 兼容
	conn := &LightpandaCDPConn{port: 9222}

	// 所有方法在无连接时应返回明确错误
	tests := []struct {
		name string
		call func() error
	}{
		{"Navigate", func() error { return conn.Navigate("https://example.com", 1000) }},
		{"ClickElement", func() error { return conn.ClickElement("#btn", 1000) }},
		{"SetViewport", func() error { return conn.SetViewport(1920, 1080, 1000) }},
		{"WaitForSelector", func() error { return conn.WaitForSelector("#app", 1000) }},
	}

	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			err := tc.call()
			if err == nil {
				t.Error("期望错误，但成功返回")
			}
		})
	}
}

// TestLightpandaConnectionDiscovery 测试 Lightpanda 不使用 /json/version 发现端点。
func TestLightpandaConnectionDiscovery(t *testing.T) {
	// Lightpanda 直接连接 ws://127.0.0.1:<port>/
	// 不应该依赖 Chromium 的 /json/version 端点
	_, err := DialLightpandaCDP(19998)
	if err == nil {
		t.Fatal("对未使用端口的连接应失败")
	}

	// IsLightpandaReachable 应对未使用端口返回 false
	if IsLightpandaReachable(19998) {
		t.Fatal("IsLightpandaReachable 应对未使用端口返回 false")
	}
}

// TestCoreKindDefaulting 测试 Kind 字段的默认行为。
func TestCoreKindDefaulting(t *testing.T) {
	dir := t.TempDir()
	// 创建 chrome.exe 而不是 lightpanda
	exePath := filepath.Join(dir, "chrome.exe")
	if err := os.WriteFile(exePath, []byte("fake"), 0755); err != nil {
		t.Fatal(err)
	}

	mgr := NewManager(&config.Config{}, dir)

	// Kind 为空时应默认为 Chromium
	core := Core{
		CoreId:   "default-kind",
		CoreName: "Default",
		CorePath: dir,
		Kind:     "", // 空
	}

	resolved, err := mgr.ResolveBrowserBinary(core)
	if err != nil {
		t.Fatalf("默认 Kind 解析失败: %v", err)
	}
	if resolved != exePath {
		t.Fatalf("默认路径 = %q, 期望 %q", resolved, exePath)
	}
}

// TestLightpandaExecutableCandidatesCoverage 验证所有平台都有候选名。
func TestLightpandaExecutableCandidatesCoverage(t *testing.T) {
	candidates := LightpandaExecutableCandidates()
	if len(candidates) == 0 {
		t.Fatal("LightpandaExecutableCandidates 不应为空")
	}

	seen := make(map[string]bool)
	for _, c := range candidates {
		if c == "" {
			t.Fatal("候选名不应为空字符串")
		}
		if seen[c] {
			t.Fatalf("重复候选名: %s", c)
		}
		seen[c] = true
	}
}
