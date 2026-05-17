# 第一族 Schema / 一致性图 / Runtime Projection
Updated: 2026-04-16 (Asia/Shanghai)

## 目的

把 `Win11 商务办公本 / 主流轻薄本` 第一族从“字段清单”推进到“可实现的 canonical spec”。

这份文档只定义三件事：

1. `schema` 怎么组织。
2. `consistency graph` 怎么判定。
3. `runtime projection` 怎么投影到当前运行时。

这份文档是内部规格书，不直接承担整体 benchmark 评分。
详细评分、阶段计划、以及 `当前 / 终局 / AdsPower` 对比入口，见 `docs/19-phase-plan-and-scorecard.md`。

## Canonical Schema

建议的第一族 canonical profile 结构如下：

```ts
type FirstFamilyProfile = {
  profileId: string;
  familyId: "win11_business_laptop";
  familyVariant: string;
  control: {
    browser: BrowserControl;
    os: OsControl;
    display: DisplayControl;
    hardware: HardwareControl;
    rendering: RenderingControl;
    locale: LocaleControl;
    network: NetworkControl;
    behavior: BehaviorControl;
  };
  derived: {
    coherenceScore: number;
    riskReasons: string[];
    supportedRuntimeFields: string[];
    unsupportedControlFields: string[];
  };
  observation: {
    signalSummary: string[];
    auditTrail: string[];
    consistencyNotes: string[];
  };
};
```

### 1. `browser`

核心职责：

- 决定浏览器身份族谱。
- 驱动 UA / UA-CH / 平台 / 版本通道一致性。

建议字段：

- `browser_family`
- `browser_channel`
- `browser_major_version`
- `browser_minor_version`
- `user_agent`
- `ua_platform`
- `ua_brand_list`
- `ua_full_version_list`
- `ua_mobile`
- `ua_architecture`

### 2. `os`

核心职责：

- 决定 Win11 形态、构建、语言和地区底座。

建议字段：

- `os_name`
- `os_version`
- `os_build_number`
- `os_edition`
- `os_branch`
- `system_locale`
- `ui_language`
- `region_format`
- `timezone`
- `daylight_saving_rule`

### 3. `display`

核心职责：

- 决定屏幕、窗口、DPR、缩放和可视区关系。

建议字段：

- `screen_width`
- `screen_height`
- `available_width`
- `available_height`
- `viewport_width`
- `viewport_height`
- `device_pixel_ratio`
- `page_zoom`
- `color_depth`
- `multi_monitor_count`

### 4. `hardware`

核心职责：

- 决定 CPU / 内存 / GPU / 供电 / 触控形态。

建议字段：

- `cpu_architecture`
- `hardware_concurrency`
- `device_memory_gb`
- `cpu_class`
- `gpu_vendor`
- `gpu_renderer`
- `touch_support`
- `max_touch_points`
- `battery_presence`
- `power_plan`

### 5. `rendering`

核心职责：

- 决定图形、字体、媒体、编码的稳定轮廓。

建议字段：

- `canvas_profile`
- `webgl_vendor`
- `webgl_renderer`
- `webgl_version`
- `audio_profile`
- `font_fingerprint_profile`
- `media_codec_profile`
- `image_decode_profile`
- `color_gamut_profile`
- `hdr_support_profile`

### 6. `locale`

核心职责：

- 决定语言、输入法、格式和文本习惯。

建议字段：

- `locale`
- `accept_language`
- `keyboard_layout`
- `input_method`
- `text_direction`
- `date_format`
- `number_format`
- `first_day_of_week`
- `typing_latency_profile`
- `punctuation_profile`

### 7. `network`

核心职责：

- 决定代理、出口、DNS、驻留和轮换策略。

建议字段：

- `proxy_type`
- `proxy_provider`
- `proxy_host`
- `proxy_port`
- `proxy_auth_mode`
- `proxy_region`
- `exit_ip`
- `dns_mode`
- `sticky_session_ttl`
- `rotation_policy`

### 8. `behavior`

核心职责：

- 决定会话长度、操作节奏、自动化边界。

建议字段：

- `click_speed_profile`
- `scroll_speed_profile`
- `pointer_smoothing_profile`
- `dwell_time_profile`
- `tab_switch_cadence`
- `idle_timeout_profile`
- `session_length_bucket`
- `automation_policy`
- `extension_profile`
- `isolation_mode`

## Consistency Graph

一致性图不是“规则列表”，而是带权重的约束网络。

建议分三类边：

- `hard_edge`：违反即不通过。
- `soft_edge`：偏离即降分。
- `derived_edge`：由上游字段推导，不单独判错。

### 关键节点关系

| 上游节点 | 下游节点 | 约束类型 | 说明 |
| --- | --- | --- | --- |
| `browser_family` | `user_agent` / `ua_brand_list` / `ua_full_version_list` | hard | 浏览器族谱必须一致 |
| `os_version` | `ua_platform` / `platform` | soft | 平台表达要和 Win11 形态匹配 |
| `locale` | `accept_language` | hard | 语言主语必须一致 |
| `locale` | `keyboard_layout` / `input_method` | soft | 文本输入习惯要贴合地区 |
| `timezone` | `proxy_region` / `exit_ip` | soft | 时区与出口区域不能长期打架 |
| `screen_width` / `screen_height` | `viewport_width` / `viewport_height` | hard | 可视区不能大于物理屏幕 |
| `device_pixel_ratio` | `page_zoom` | soft | 缩放与 DPR 需要落在同一族谱 |
| `hardware_concurrency` / `device_memory_gb` | `cpu_class` / `power_plan` | soft | 硬件档位要相互解释得通 |
| `gpu_vendor` / `gpu_renderer` | `webgl_vendor` / `webgl_renderer` | hard | 渲染表述必须一致 |
| `canvas_profile` / `audio_profile` / `font_fingerprint_profile` | `browser_family` | soft | 这类高阶信号必须落在对应族谱 |
| `touch_support` / `max_touch_points` | `familyVariant` | hard | 触控与机型变体强绑定 |
| `battery_presence` | `power_plan` / `session_length_bucket` | soft | 电池形态影响功耗和驻留策略 |
| `sticky_session_ttl` | `rotation_policy` | hard | 驻留和轮换必须自洽 |
| `automation_policy` | `idle_timeout_profile` / `tab_switch_cadence` | derived | 自动化策略决定行为节奏边界 |

### 评分规则建议

- `100`：全部硬约束通过，软约束无明显偏离。
- `80-99`：硬约束通过，少量软约束偏离。
- `50-79`：存在多处软约束冲突，需要人工确认。
- `<50`：出现结构性冲突，不应投放到默认 profile。

## Runtime Projection

当前运行时投影应分两层：

1. `canonical profile`：保留完整控制面与派生面。
2. `runtime projection`：只投影当前 runner / browser backend 真正消费的字段。

### 当前 Lightpanda 投影对齐

基于 `src/network_identity/fingerprint_consumption.rs`，当前已明确支持的投影字段主要包括：

- `accept_language`
- `timezone`
- `locale`
- `platform`
- `user_agent`
- `viewport_width`
- `viewport_height`
- `screen_width`
- `screen_height`
- `device_pixel_ratio`
- `hardware_concurrency`
- `device_memory_gb`

### 投影结果建议包含

- `declared_fields`
- `resolved_fields`
- `applied_fields`
- `ignored_fields`
- `consumption_status`
- `consumption_version`
- `partial_support_warning`

### 投影原则

- 不能因为 runtime 目前只支持一部分字段，就删掉 canonical profile 的其余字段。
- canonical profile 是 truth source，runtime projection 只是适配层。
- 每次投影都要保留 `ignored_fields`，否则无法解释支持缺口。
- `partial_support_warning` 不是失败，而是当前 backend 覆盖范围的事实记录。

## 推荐实现切片

### Slice 1: Canonical schema

- 建立 `FirstFamilyProfile` 的 TS / Rust 对齐结构。
- 把 80 核心控制字段映射进 `control` 分组。

### Slice 2: Consistency graph

- 实现节点、边、权重和解释输出。
- 先覆盖 hard edge，再覆盖 soft edge。

### Slice 3: Runtime projection

- 将 canonical profile 投影到当前运行时支持字段。
- 输出消费状态和支持缺口。

### Slice 4: Expanded observations

- 让 450+ 信号总量逐步进入 derived / observation 层。

## 建议验收

- schema 可序列化、可迁移、可解释。
- 一致性图能拦截结构性冲突。
- runtime projection 与当前 backend 支持范围对齐。
- canonical profile 不会因 backend 缺口被压扁成“少量 env”。
