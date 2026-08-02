# Personal Pilot — 架构审计与优化方案

> 基于全量代码扫描的架构臃肿度审计、技术债评估与优化路线。
> 配合 docs/22-cross-cutting-concerns-and-performance.md 一起阅读。

---

## 一、总体状况

| 维度 | 数据 | 健康度 |
|------|------|--------|
| Go 后端 | 64 根目录 Go 文件 + 194 internal Go 文件 | ⚠️ 根目录仍偏胖，但 backup/browser 已部分拆分 |
| Rust 重写 (src/) | 51 文件, 53,761 行, 无生产使用 | 🔴 僵住 |
| Tauri Shell (src-tauri/) | 4 文件, ~2300 行 | ✅ 轻量 |
| TypeScript 前端 | 241 node_modules 包, Vite + React + Tailwind | ✅ 正常 |
| .claude/worktrees/ | 12 个 worktree | ⚠️ 已少于旧审计，仍需定期 prune |
| go.mod | 11 direct deps, 127 transitive deps | ⚠️ mihomo 拖累 |
| 配置源 | 6+ 处分散 | 🔴 无单一真相源 |
| 生成代码 | 13,307 行 (事件系统) | ⚠️ 可优化 |

---

## 二、Critical 问题

### 2.1 Agent Worktree 残留 — 需定期清理

**.claude/worktrees/** 当前仍有约 12 个 worktree。旧审计中的 39 个 / 42 GB 状态已不再准确，但临时 worktree 仍可能积累。

**影响:** 磁盘占用增加，构建/搜索可能误扫隔离工作区。

**修复:**
```powershell
git worktree prune
# 如确认无需保留 agent 工作区，再删除 .claude/worktrees 下已合并分支
```

---

### 2.2 Rust 重写僵住 — 51 文件 53,761 行

`src/` 目录包含一整套独立的 Rust Axum 后端实现，与生产 Go 后端并行。这是 `docs/14-rust-rewrite-analysis.md` 评估后搁置的重写。

| 文件 | 行数 | 问题 |
|------|------|------|
| `tests/integration_api.rs` | 9,660 | 单体测试文件，不可导航 |
| `src/api/handlers/core.rs` | 7,985 | 单体 handler — 立即拆分 |
| `src/api/handlers.rs` | 7,223 | 同上 |
| `src/desktop/mod.rs` | 6,968 | 桌面 IPC 混杂 |
| `src/runner/engine.rs` | 4,464 | 执行引擎 |
| `src/runner/lightpanda.rs` | 4,353 | Lightpanda runner |
| `src/gateway/control.rs` | 4,034 | 网关控制 |
| 其余 44 文件 | ~20,000 | 各类模块 |

**影响:**
- 双代码库并行维护，心智负担翻倍
- Rust 代码 **无任何生产流量** — 所有用户操作走 Go 后端
- `Cargo.toml` (根) + `src-tauri/Cargo.toml` 架构混乱
- Go 和 Rust 有重复的代理解析逻辑 (`parser.go` vs `proxy_harvest.rs`)

**推荐方案: 二选一**

| 方案 | 工作 | 风险 | 推荐? |
|------|------|------|-------|
| **A: 彻底提交 Rust 重写** | 18 周 (per `14-rust-rewrite-analysis.md`) | 高 — 需完整重写所有 handler + 测试 | ❌ |
| **B: 删除 src/ 下所有 .rs + tests/** | 1 天 | 低 — Go 生产代码完整 | ✅ **强烈推荐** |

**如选 B，保留清单:**
- `src-tauri/` — 保留 (Tauri shell, 依赖 persona-pilot core)
- `src/main.rs` — 删除
- `src/` 下所有非 `src-tauri/` 的 .rs — 删除
- `tests/` — 删除
- `Cargo.toml` (根) — 删除 (src-tauri/Cargo.toml 保留)
- 如果未来需要 Rust，从零开始小步增量引入

---

### 2.3 mihomo 依赖拖累

`github.com/metacubex/mihomo v1.19.20` 的唯一用途: `adapter.ParseProxy` 在 `backend/internal/proxy/speedtest.go`。

**拖入的传递依赖:** 80+ (crypto, quic, wireguard, tls, dns, reedsolomon 等)

**影响对比:**

| 指标 | 当前 (有 mihomo) | 去除后 |
|------|-----------------|--------|
| 传递依赖数 | 127 | ~40 |
| 构建时间 | ~2min | ~30s |
| 二进制大小 | ~25 MB | ~8 MB |
| CI 时间 | 3-4min | ~1min |

**修复:** 已有 fallback `httpClientDelayTest()`，删除 mihomo 后自动使用。

---

### 2.4 配置源 6+ 处分散

| 来源 | 位置 | 类型 |
|------|------|------|
| 主配置 | `backend/config.yaml` | YAML |
| App 配置结构体 | `backend/internal/config/config.go` | Go struct + defaults |
| 初始化模板 | `publish/config.init.yaml`, `*.linux.yaml`, `*.mac.yaml` | YAML |
| API 密钥 | `.env.example` | env |
| 网关配置 | `.env.gateway.example` | env |
| Humanize 配置 | `backend/internal/behavior/humanize/config.go` | Go struct |
| Rust 配置 | `src/config/` (删除后消失) | Rust module |
| Rust humanize | `src/humize/config.rs` (删除后消失) | Rust struct |
| 事件 schema | `backend/internal/events/schema.yaml` | YAML |
| 硬编码默认值 | `backend/app.go`, `backend/bootstrap.go`, `backend/internal/config/config.go` | Code |

**影响:**
- 字段在 Go 和 Rust 间重复定义，可能漂移
- 新开发者不知道在哪改配置
- 无启动时验证，错误配置静默使用默认值

**推荐统一方案:**

```
单一真相源: backend/config.yaml
  └── 环境变量覆盖 (PERSONA_PILOT_* prefix)
       └── 启动时 validate + fail-fast
            └── frozen AppConfig struct (启动后不可变)
```

```go
// backend/internal/config/config.go
type AppConfig struct {
    // 启动时从此结构体读取所有配置
    // 初始化后不可变
}

func LoadConfig(path string) (*AppConfig, error) {
    // 1. 加载 config.yaml
    // 2. 环境变量覆盖
    // 3. validate all fields
    // 4. fail-fast on any invalid value
}
```

---

### 2.5 Go 后端根目录文件膨胀

`backend/` 根目录仍有 64 个 Go 文件，但备份与浏览器启动/进程监控已部分拆分：

| 文件 | 行数 | 当前状态 / 建议 |
|------|------|----------------|
| `app_backup_ops.go` | 68 | 已拆薄为操作入口 |
| `app_backup_core.go` | 269 | 备份核心逻辑 |
| `app_backup_import.go` | 535 | 导入/恢复逻辑，后续可继续拆测试 |
| `app_backup_merge.go` | 679 | 合并逻辑，仍偏大 |
| `app_backup_utils.go` | 452 | 工具逻辑 |
| `app.go` | 1,471 | 仍建议拆分 App struct / Wails 绑定 / 生命周期 |
| `app_instance.go` | 988 | 仍建议拆分 CRUD / 操作逻辑 |
| `app_launchcode.go` | 878 | 可接受 |
| `app_deepseek_register.go` | 822 | 可接受 |

**根目录文件归位 (按 docs/12-module-split-and-backlog.md):**

| 当前位置 | 目标位置 |
|----------|----------|
| `browser_runtime_state.go` | `internal/browser/` |
| `browser_start_settings.go` | `internal/browser/` |
| `browser_process_monitor.go` | ✅ 已迁移到 `internal/browser/process_monitor.go` |
| `browser_launch_args.go` | ✅ 已迁移到 `internal/browser/launch_args.go` |
| `window_control_*.go` | `internal/wininput/` 或新建 `internal/winctrl` |
| `sysproc_*.go` | `internal/proxy/` |
| `residual_processes_*.go` | `internal/browser/` |
| `license_state.go` / `app_license.go` | `internal/license/` |
| `runtime_bridge.go` / `runtime_wails.go` | `internal/runtime/` (或删除) |

---

## 三、Major 问题

### 3.1 Wails 遗留代码

Tauri 迁移后，Wails 相关文件未清理:

| 文件 | 说明 |
|------|------|
| `backend/runtime_bridge.go` | Wails runtime bridge |
| `backend/runtime_wails.go` | Wails 专属 runtime |
| `src/wailsjs/go/main/App.js` + 5 个绑定文件 | Wails JS bindings |
| `src/services/tauriWailsBridge.ts` (272 行) | 兼容性 shim |
| `go.mod` 中 `github.com/wailsapp/wails/v2` | 未使用依赖 |

**清理方案:**
1. 确认 Tauri invoke 已覆盖所有前端调用路径
2. 删除 `backend/runtime_bridge.go`, `backend/runtime_wails.go`
3. 删除 `src/wailsjs/` 目录
4. 删除 `src/services/tauriWailsBridge.ts`
5. 从 `go.mod` 移除 wails v2
6. 前端调用统一走 `@tauri-apps/api`

---

### 3.2 事件系统生成代码膨胀

| 文件 | 行数 | 类型 |
|------|------|------|
| `backend/internal/events/gen_emitters.go` | 5,981 | 自动生成 |
| `backend/internal/events/gen_constants.go` | 878 | 自动生成 |
| `backend/internal/events/schema.yaml` | 6,449 | 自动生成 (从 registry.go) |
| `backend/internal/events/registry.go` | 1,448 | 人工维护但被作为生成源 |

**架构问题:**
- `registry.go` 是手工源文件但被作为代码生成源
- `schema.yaml` 从 registry.go 生成，再从 schema.yaml 生成 TS 类型
- 正常的代码生成方向: schema → code，而非反方向

**优化方案:**
```yaml
# 当前: registry.go (手工) → gen_emitters.go + gen_constants.go + schema.yaml
# 推荐: schema.yaml (单一真相源) → gen_emitters.go + gen_constants.go + gen_types.ts
```

---

### 3.3 CDP Executor 静默 fallthrough

`backend/internal/behavior/cdp_executor.go:602`:
```go
default:
    return nil
```

未处理的 mutation type 静默返回 nil，调用方无法感知操作未执行。

**修复:** 改为返回明确的 error:
```go
default:
    return fmt.Errorf("unknown mutation type: %T %+v", m, m)
```

---

### 3.4 `crashTimestamps` 无上限增长

`backend/app.go:73`:
```go
crashTimestamps map[string][]time.Time
```

每次浏览器崩溃追加时间戳，但**从不清理**。长期运行内存泄漏。

**修复:** 加 LRU 上限 + TTL 清理:
```go
type CrashTracker struct {
    sync.Mutex
    maxEntries int           // 每 profile 最多 N 条
    ttl        time.Duration // 超过此时间的条目自动清理
    data       map[string][]time.Time
}
```

---

## 四、优化路线 (Phased)

```
Phase 0 — 立即 (1 小时内)
├── git worktree prune + 清理已合并/无需保留的 .claude/worktrees
└── go.mod 移除 mihomo → 构建 2min → 30s

Phase 1 — 第 1 周
├── Rust 删除决策: 删除 src/ 下 51 文件 + tests/ + 根 Cargo.toml
├── 删除 Wails 遗留 (runtime_bridge.go / wailsjs / tauriWailsBridge.ts)
├── CDP Executor default → 返回 error
└── crashTimestamps 加 LRU 上限

Phase 2 — 第 2-3 周
├── 配置统一: config.yaml 单一真相源 + env override + startup validation
├── 继续拆分大文件: app.go / app_instance.go / app_backup_merge.go
├── Go 根目录剩余文件归位 → internal/ 包
└── 事件系统生成方向反转: schema.yaml → code

Phase 3 — 第 4-6 周
├── 超大 Go 文件持续拆分
├── 双层 JSON 键名统一 (camelCase → snake_case)
├── 代理凭证全路径脱敏
├── 日志: Rust println! 移除, Go 日志系统统一
└── 测试覆盖: automation / database / tray / webhook / wininput 补全

Phase 4 — 第 7-12 周
├── 增量重构: internal/browser/ 和 internal/proxy/ 包深度清理
├── 移除重复逻辑: Go vs 已删 Rust 的代理解析统一
├── SQLite 迁移框架 (golang-migrate)
└── OpenAPI 3.0 规范文档
```
