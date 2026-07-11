# 交互行为保真层设计

> 本文档描述 PersonaPilot 的交互行为保真层——在 CDP 浏览器控制的基础上，通过注入**符合人类操作特征的时序、轨迹和节奏模型**，提升自动化交互的拟真度。
> 
> 该层不是"反检测"或"绕过"机制，而是一种**交互质量优化中间件**，确保程序化操作在时序/空间分布上与真人操作不可区分。

---

## 0. 术语表

| 术语 | 等价含义 |
|------|---------|
| 行为保真度 (Behavioral Fidelity) | 程序化交互与真人操作在统计学上的接近程度 |
| 时序分布 (Timing Distribution) | 操作间延迟的概率分布模型（均匀/正态/右偏） |
| 轨迹引擎 (Trajectory Engine) | 生成鼠标移动路径的算法，包含贝塞尔曲线和生理噪声 |
| 敲击节奏 (Keystroke Rhythm) | 逐字输入的间隔分布模型，含认知暂停和纠错模式 |
| 滚动物理 (Scroll Physics) | 模拟手指/滚轮物理特性的滚动动画模型 |
| 交互间隙 (Action Gap) | 两次交互操作之间的自然停顿时间 |
| 故障恢复 (Failure Recovery) | 操作失败时的重试策略，含"人为"犹豫和决策延迟 |

---

## 一、整体架构

```
┌─────────────────────────────────────────────────────────────┐
│                    Behavioral Fidelity Layer                  │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│  ┌─────────┐   ┌─────────┐   ┌─────────┐   ┌────────────┐  │
│  │ Timing  │   │Traject.│   │ Keystrk │   │ Scroll     │  │
│  │ Engine  │──▶│ Engine │──▶│ Engine  │──▶│ Engine     │  │
│  └─────────┘   └─────────┘   └─────────┘   └────────────┘  │
│       │             │             │              │          │
│       └─────────────┼─────────────┼──────────────┘          │
│                     ▼             ▼                         │
│  ┌──────────────────────────────────────────────────────┐   │
│  │            CDP Input Layer (动作分发层)                │   │
│  │  Input.dispatchMouseEvent / Input.dispatchKeyEvent    │   │
│  └──────────────────────────────────────────────────────┘   │
│                                                              │
│  ┌──────────────────────────────────────────────────────┐   │
│  │            Failure Recovery Middleware                 │   │
│  │  错误分类 → 人为犹豫 → 重试决策 → 切换方案 → 放弃    │   │
│  └──────────────────────────────────────────────────────┘   │
│                                                              │
│  ┌──────────────────────────────────────────────────────┐   │
│  │            Config Presets (预设配置)                    │   │
│  │  None → Minimal → Medium → High → Extreme             │   │
│  └──────────────────────────────────────────────────────┘   │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

---

## 二、配置预设体系

### 2.1 五级保真度预设

```go
// backend/internal/behavior/humanize/config.go
// src/humanize/config.rs（Rust 版本）
```

| 级别 | 时序 | 鼠标轨迹 | 键盘 | 滚动 | 故障恢复 | 单步额外耗时 |
|------|------|---------|------|------|---------|------------|
| `None` | 0ms 延迟 | 无（直线跳转） | 9999 WPM | 无（直接跳转） | 立即重试 | 0ms |
| `Minimal` | 右偏 100-1500ms | Bezier2 + 3px jitter | 60 WPM, 2%纠错 | 15% overshoot | 0.5-3s 等待 | ~200ms |
| `Medium` | 右偏 150-2000ms | Bezier2 + 8px jitter | 40 WPM, 3%纠错, 8%暂停 | 35% overshoot | 1.5-6s 等待 | ~500ms |
| `High` | 右偏 300-4000ms | Bezier3 + 12px jitter | 35 WPM, 5%纠错, 12%暂停 | 50% overshoot | 3-12s 等待 | ~1.2s |
| `Extreme` | 右偏 500-6000ms | Catmull-Rom + Fitts' Law + 1/f 生理噪声 | 30 WPM, 8%纠错, 15%暂停 + 疲劳衰减 | 物理惯性 + 回读 + 微调 | 5-30s 等待 | ~2.5s |

### 2.2 时序分布模型

```go
// backend/internal/behavior/humanize/timing.go
```

**三种分布类型：**

| 类型 | 公式 | 典型参数 | 适用场景 |
|------|------|---------|---------|
| `DistUniform` | `U(min, max)` | [100, 500] | 简单确认操作 |
| `DistNormal` | Box-Muller: `μ + σ·z` | μ=800ms, σ=200ms | 表单字段间切换 |
| `DistRightSkewed` | 指数变换: `min + Exp(λ)` | mode=400ms, max=4000ms | 阅读/思考/决策 |

**右偏分布的行为学依据：**

```
真人操作间隙符合右偏分布:
  - 大部分操作在短时间窗口内完成（峰在 300-800ms）
  - 偶尔出现长间隔（2-10s），对应阅读/思考/犹豫
  - 极长间隔（>10s）概率趋近于零
  
  实现: 指数逆变换采样 → Exp(λ) + mode_ms
        λ = 1 / (mode_ms - min_ms + 1)
```

---

## 三、轨迹引擎 (Trajectory Engine)

### 3.1 已有实现

```go
// backend/internal/behavior/cdp_ops.go (mouseMove, 238-301行)
// src/humanize/trajectory.rs（Rust 版本）
```

**三种曲线模型：**

| 模型 | 控制点 | 公式 | 物理意义 |
|------|--------|------|---------|
| `bezier2` | 1 个控制点 | `(1-t)²·P0 + 2t(1-t)·P1 + t²·P2` | 简单弧线运动 |
| `bezier3` | 2 个控制点 | `(1-t)³·P0 + 3t(1-t)²·P1 + 3t²(1-t)·P2 + t³·P3` | 更自然的 S 形路径 |
| `natural` | 1个控制点 + 噪声 | 类 Bezier2 + 随机偏移 | 快速粗略运动 |

**已有速度曲线：**

```
sin(π·t) 调制:
  - 起止段慢 (加速/减速)
  - 中段快 (巡航速度)
  - 叠加随机微暂停 pauseProb * pauseMaxMs
```

### 3.2 增强方案（Extreme 级别）

| 特性 | 现有 | 增强 (Extreme) | 实现文件 |
|------|------|---------------|---------|
| 路径曲线 | Cubic Bezier | **Catmull-Rom + 三段式**（加速段/匀速段/减速段分别控制点） | `trajectory_fitts.go` |
| 速度模型 | sin(π·t) | **Fitts' Law**: `MT = a + b·log₂(D/W+1)` | `trajectory_fitts.go` |
| 生理噪声 | 均匀 jitter ±3-12px | **1/f 粉噪声 8-12Hz** 叠加手部微震颤 | `trajectory_noise.go` |
| 点击动作 | 2 段式 (down/up) | **4 段式**: 预压(20ms) → 触峰 → 后压(50ms) → 释放 | `trajectory_click.go` |
| 双击 | 无 | **N(300ms, 50ms)** 间隔，首击后微移 <5px | `trajectory_doubleclick.go` |
| 拖拽 | 无 | 按下→慢移→微停→继续→松开 | `trajectory_drag.go` |
| 右键 | 无 | Hover → pause → contextMenu event | `trajectory_context.go` |
| 微纠正 | 无 | 接近目标时的 1-3 次 sub-movement 修正 | `trajectory_submovement.go` |
| 元素感知 | 简单 inset | **按元素类型加权**: button/input/select/image/link 分别用不同偏移半径 | `trajectory_element.go` |

### 3.3 Fitts' Law 模型

```go
// Fitts' Law: MT = a + b * log2(D/W + 1)
// MT = Movement Time, D = Distance, W = Target Width
// a, b = 经验常数（来自 Welford 或 MacKenzie 公式）

func movementTime(distancePx, targetWidthPx float64) time.Duration {
    id := math.Log2(distancePx/targetWidthPx + 1)  // 难度指数
    mt := 50.0 + 150.0*id                           // a=50, b=150 (ms)
    return time.Duration(mt) * time.Millisecond
}

// Fitts' Law 的意义:
//   目标越小、越远 → 运动时间越长
//   目标越大、越近 → 运动时间越短
//   真人操作天然遵循此规律
```

### 3.4 生理噪声

```go
// 1/f 噪声 (粉噪声):
// 使用 Voss-McCartney 算法生成
// 频率范围: 8-12 Hz（对应手部生理震颤频率）
// 幅度: ±2-5px（在标准使用场景中不可感知）

type PinkNoiseGenerator struct {
    rows    [16]float64
    running float64
    index   int
}

func (p *PinkNoiseGenerator) Next() float64 {
    // Voss-McCartney 算法
    // 叠加 16 个不同频率的随机方波
    return amplitude * value
}

// 叠加到轨迹上，模拟真人无法完全静止的手部震颤
```

---

## 四、敲击节奏引擎 (Keystroke Rhythm Engine)

### 4.1 已有实现

```go
// backend/internal/behavior/humanize/typing.go (151行)
// src/humanize/typing.rs（Rust 版本）
```

**已有特性：**

| 特性 | 参数 | 描述 |
|------|------|------|
| Base WPM | 35-60 | 基准打字速度 |
| Per-key Variance | 20-60% | 每键间隔随机波动 |
| Error Retry | 1-8% | QWERTY 邻键纠错概率 |
| Thinking Pause | 2-15% | 中间思考暂停概率 |
| Word-final Pause | 30-80ms | 词末自然停顿 |
| Shift/Caps Penalty | +25ms | 大小写切换额外时间 |
| Symbol Penalty | +25ms | 符号键额外时间 |

### 4.2 增强方案（Extreme 级别）

| 特性 | 现有 | 增强 | 说明 |
|------|------|------|------|
| 击键节奏 | 统一 WPM ± variance | **按手指生理分区**: 食指 80%, 中指 70%, 无名指 60%, 小指 45% | `typing_finger.go` |
| 同键重复 | 无 | **同键延迟 +30%**（松开→再按需要时间） | 内置 |
| 疲劳衰减 | 无 | **每分钟 WPM 衰减 2-3%** | `typing_fatigue.go` |
| 输入法模拟 | 无 | **IME 序列**: 拼音→候选→箭头选择→Enter 确认 | `typing_ime.go` |
| 组合键 | 无 | Ctrl+C/V/A/Z + Windows 键布局 | `typing_modifier.go` |
| 退格纠正 | 单步退格 | **退格后 pause + re-type**: N(200, 50)ms 重新输入 | 内置 |
| 密码输入 | 无 | **速度比普通文本慢 40%**，但保持节奏 | `typing_context.go` |
| 表单 Tab | 无 | Tab 切换字段间 N(500, 200)ms 间隙 | `typing_tab.go` |
| 剪贴板操作 | 无 | Ctrl+A → Ctrl+C: 真实 clipboard 交互 | `typing_clipboard.go` |

### 4.3 QWERTY 手指分区

```go
// 标准 QWERTY 键盘的手指分配:
//   左手:   QWERT (小指/无名/中/食/食)
//           ASDFG (小指/无名/中/食/食)
//           ZXCVB (小指/无名/中/食/食)
//   右手:   YUIOP (食/食/中/无名/小指)
//           HJKL; (食/中/无名/小指/小指)
//           NM,./ (食/中/无名/小指)

// 各手指 WPM 速度比（相对于基准）:
var fingerSpeedRatio = map[string]float64{
    "left_pinky":    0.45,
    "left_ring":     0.60,
    "left_middle":   0.70,
    "left_index":    0.80,
    "right_index":   0.85,
    "right_middle":  0.75,
    "right_ring":    0.60,
    "right_pinky":   0.40,
}
```

---

## 五、滚动物理引擎 (Scroll Physics)

### 5.1 已有实现

```go
// backend/internal/behavior/humanize/scroll.go (125行)
// src/humanize/scroll.rs（Rust 版本）
```

**四阶段 Overshoot-Return：**

```
阶段 0: Hover pause (0-400ms)
阶段 1: Overshoot 快滚动 (15-50% of target)
阶段 2: 暂停 (200-450ms) — "看过头了"
阶段 3: 慢滚动回到目标位置
阶段 4: Micro-adjustment (±30px) — "微调"
```

### 5.2 增强方案（Extreme 级别）

| 特性 | 现有 | 增强 | 说明 |
|------|------|------|------|
| 物理模型 | Overshoot 固定比例 | **手指 flick 惯性**: 初速度 → 减速 → 回弹 | `scroll_physics.go` |
| 内容感知 | 无固定 | **段落末减速, 图片上方暂停, 标题跳过** | `scroll_content.go` |
| 回读行为 | 无 | **15-25% 概率回滚 100-300px** ("我刚才看到哪了") | `scroll_reread.go` |
| 快速扫读 | 无 | 先快滚至底部 → 再慢滚回顶部 | `scroll_scan.go` |
| 横向滚动 | 无 | 宽表格/大图时水平 scroll | `scroll_horizontal.go` |
| 无限滚动 | 无 | 到底 → 暂停 → 加载更多 → 继续 | `scroll_infinite.go` |
| 微抖 | 无 | 靠在触摸板上的无意微滚 ±20px | `scroll_microjitter.go` |

### 5.3 物理惯性模型

```go
// 触摸板 flick 物理模型
type FlickPhysics struct {
    initialVelocity float64   // 初始速度 (px/s)
    friction        float64   // 摩擦系数 (0.92-0.97)
    minVelocity     float64   // 停止速度阈值
    deceleration    float64   // 减速度
}

func (f *FlickPhysics) Simulate() []ScrollPosition {
    var positions []ScrollPosition
    v := f.initialVelocity
    
    for v > f.minVelocity {
        v *= f.friction           // 摩擦减速
        v -= f.deceleration * dt  // 恒定减速度
        
        pos := integrate(v, dt)
        positions = append(positions, pos)
    }
    
    // 末段回弹（触摸板释放后的小反弹）
    if rand.Float64() < 0.3 {
        positions = append(positions, rebound(10, 30))
    }
    
    return positions
}
```

---

## 六、故障恢复中间件 (Failure Recovery)

### 6.1 错误分类

```go
// backend/internal/behavior/humanize/failure.go
// src/humanize/failure.rs（Rust 版本）
```

**8 种错误类型：**

| 错误码 | 场景 | 致命性 | 恢复策略 |
|--------|------|--------|---------|
| `ErrElementNotFound` | 选择器未找到元素 | 非致命 | 重试 + 等待后重试 |
| `ErrClickFailed` | 点击无响应 | 非致命 | 偏移后重试 |
| `ErrNavigationTimeout` | 导航超时 | 非致命 | 切换路由后重试 |
| `ErrProxyDead` | 路由节点不可用 | 致命（第1次可重试） | 切换路由后重试 |
| `ErrFingerprintRejected` | 站点拒绝环境标识 | 致命 | 切换标识后重试 |
| `ErrSessionCrashed` | 浏览器崩溃 | 致命 | 重启会话 |
| `ErrRateLimited` | 请求频率限制 | 非致命 | 延长等待后重试 |
| `ErrContentBlinded` | 内容被遮挡 | 非致命 | 滚入视图后重试 |

### 6.2 人为化重试决策

```go
// HumanizedRetryDecision 模拟人遇到错误时的反应

// 机器模式 (FailureInstant):
//   总是立即重试，直到 maxRetries
//   像机器一样快速连续尝试

// 人为模式 (FailureHuman):
//   1. 判断是否致命错误（致命→放弃或切换方案）
//   2. 判断是否已超过心理阈值（随机提前放弃）
//   3. 等待并重试（等待时间随着尝试次数递增 + 30%）
//   4. 重试策略:
//      - 第 1 次失败: "再试一下" (短等待)
//      - 第 2 次失败: "怎么回事" (中等等待)  
//      - 第 3 次失败: "换个方式试试" (长等待 + 切换方案)
//      - 第 4 次失败: "算了不弄了" (放弃)
```

**恢复策略组合：**

```
┌─────────────────────────────────────────────────────────┐
│  RecoveryActionType                                      │
│  ├── RecoveryRetry（立即重试）                             │
│  ├── RecoveryRetryAfter（等待后重试）                      │
│  │     WaitMs ↑ 随着 attempt 递增                          │
│  ├── RecoveryGiveUp（放弃）                                │
│  ├── RecoverySwitchApproach（换方案重试）                   │
│  │     click → offset_click                               │
│  │     navigate → switch_proxy → navigate                  │
│  └── RecoverySkip（跳过当前操作）                            │
└─────────────────────────────────────────────────────────┘
```

---

## 七、已有代码资产汇总

| 模块 | 代码文件 | 行数 | 文档状态 |
|------|---------|------|---------|
| 配置预设 | `backend/internal/behavior/humanize/config.go` | 191 | ❌ 本文档 |
| 时序引擎 | `backend/internal/behavior/humanize/timing.go` | 131 | ❌ 本文档 |
| 轨迹引擎 | `backend/internal/behavior/humanize/trajectory.go` | 269 | ❌ 本文档 |
| 敲击节奏 | `backend/internal/behavior/humanize/typing.go` | 151 | ❌ 本文档 |
| 滚动模型 | `backend/internal/behavior/humanize/scroll.go` | 125 | ❌ 本文档 |
| 故障恢复 | `backend/internal/behavior/humanize/failure.go` | 156 | ❌ 本文档 |
| CDP 操作层 | `backend/internal/behavior/cdp_ops.go` | 360 | ❌ 本文档 |
| CDP 执行器 | `backend/internal/behavior/cdp_executor.go` | 1087 | ❌ 本文档 |
| Rust 轨迹 | `src/humanize/trajectory.rs` | — | ❌ 本文档 |
| Rust 敲击 | `src/humanize/typing.rs` | — | ❌ 本文档 |
| Rust 滚动 | `src/humanize/scroll.rs` | — | ❌ 本文档 |
| Rust 时序 | `src/humanize/timing.rs` | — | ❌ 本文档 |
| Rust 故障 | `src/humanize/failure.rs` | — | ❌ 本文档 |
| Rust 配置 | `src/humanize/config.rs` | — | ❌ 本文档 |
| Rust 中间件 | `src/humanize/middleware.rs` | — | ❌ 本文档 |
| 测试套 | 12 个测试文件 | — | ✅ 有测试 |
