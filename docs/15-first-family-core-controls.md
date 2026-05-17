# Win11 商务办公本 第一族 80 核心控制字段
Updated: 2026-04-16 (Asia/Shanghai)

## 目标

第一族先锁 `Win11 商务办公本 / 主流轻薄本`，优先做高真实性、低批量、强一致性。

这里的 `80` 不是 `80` 个随机开关，而是第一族的核心控制输入集。
它们会驱动 profile 生成、约束校验、运行时投影和解释输出。

这份文档是 `control dictionary`，不是成熟度评分板。
详细评分、阶段计划、以及 `80 declared / 12 runtime / 450+ target` 的汇总入口，见 `docs/19-phase-plan-and-scorecard.md`。

## 原则

- 只保留真正决定族谱和一致性的控制字段。
- 细节信号尽量由族谱和规则派生，不单独随机。
- 所有控制字段都必须能被解释、回放、评分。
- 不是每个字段都要直接暴露给用户。

## 80 个核心控制字段

### 1. 浏览器身份与版本族

1. `browser_family`
2. `browser_channel`
3. `browser_major_version`
4. `browser_minor_version`
5. `user_agent`
6. `ua_platform`
7. `ua_brand_list`
8. `ua_full_version_list`
9. `ua_mobile`
10. `ua_architecture`

### 2. OS / Shell / Build

11. `os_name`
12. `os_version`
13. `os_build_number`
14. `os_edition`
15. `os_branch`
16. `system_locale`
17. `ui_language`
18. `region_format`
19. `timezone`
20. `daylight_saving_rule`

### 3. 屏幕 / 窗口 / 缩放

21. `screen_width`
22. `screen_height`
23. `available_width`
24. `available_height`
25. `viewport_width`
26. `viewport_height`
27. `device_pixel_ratio`
28. `page_zoom`
29. `color_depth`
30. `multi_monitor_count`

### 4. 硬件 / 设备类

31. `cpu_architecture`
32. `hardware_concurrency`
33. `device_memory_gb`
34. `cpu_class`
35. `gpu_vendor`
36. `gpu_renderer`
37. `touch_support`
38. `max_touch_points`
39. `battery_presence`
40. `power_plan`

### 5. 渲染 / 字体 / 媒体

41. `canvas_profile`
42. `webgl_vendor`
43. `webgl_renderer`
44. `webgl_version`
45. `audio_profile`
46. `font_fingerprint_profile`
47. `media_codec_profile`
48. `image_decode_profile`
49. `color_gamut_profile`
50. `hdr_support_profile`

### 6. Locale / 文本 / 输入

51. `locale`
52. `accept_language`
53. `keyboard_layout`
54. `input_method`
55. `text_direction`
56. `date_format`
57. `number_format`
58. `first_day_of_week`
59. `typing_latency_profile`
60. `punctuation_profile`

### 7. 网络 / 代理 / DNS

61. `proxy_type`
62. `proxy_provider`
63. `proxy_host`
64. `proxy_port`
65. `proxy_auth_mode`
66. `proxy_region`
67. `exit_ip`
68. `dns_mode`
69. `sticky_session_ttl`
70. `rotation_policy`

### 8. 会话 / 行为 / 策略

71. `click_speed_profile`
72. `scroll_speed_profile`
73. `pointer_smoothing_profile`
74. `dwell_time_profile`
75. `tab_switch_cadence`
76. `idle_timeout_profile`
77. `session_length_bucket`
78. `automation_policy`
79. `extension_profile`
80. `isolation_mode`

## 这些字段怎么用

- `browser identity` 和 `OS / shell` 决定 profile 的基础族谱。
- `screen / hardware / rendering` 决定设备是否像同一类真实 Win11 机器。
- `locale / text / input` 决定地域、语言、键盘、文本行为是否互相一致。
- `network / proxy` 决定出口、地域、冷却、驻留是否同步。
- `session / behavior / policy` 决定长期会话、操作节奏和自动化边界。

## 与当前代码的关系

- `src/network_identity/fingerprint_policy.rs` 负责优先级与预算分层。
- `src/network_identity/fingerprint_consistency.rs` 负责跨字段一致性检查。
- `src/network_identity/validator.rs` 负责范围、必填、基础合法性。
- `src/network_identity/fingerprint_consumption.rs` 负责运行时投影与部分支持消费。

## 现阶段约束

- 这 `80` 个字段不等于 `80` 个直接运行时 env。
- 当前运行时只会消费其中可投影的一部分，其余由一致性图和 profile spec 管理。
- 后续扩到 `450+` 时，新增的大量字段应优先进入派生面和观测面。

## 建议验收

- 每个 profile 能稳定归类到同一设备族。
- 核心控制变化不会引发大面积随机漂移。
- 一致性评分和风险原因可解释。
- 代理驻留、locale、timezone、屏幕和行为节奏能够互相对齐。

下一层的 schema / consistency graph / runtime projection 见 [docs/16-first-family-schema-consistency-runtime.md](/D:/SelfMadeTool/persona-pilot/docs/16-first-family-schema-consistency-runtime.md)。
