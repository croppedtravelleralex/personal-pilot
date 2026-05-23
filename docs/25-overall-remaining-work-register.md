# Overall Remaining Work Register

Updated: 2026-05-23 (Asia/Shanghai)

## 使用规则

本文件承接当前 `40% / 60% / yellow` 的剩余工作。条目分为：

- `ready`：本机可继续实现或验证。
- `blocked-external`：需要凭证、第二台机器、真实 provider、外部服务或人工验收。
- `large-slice`：需要拆成多轮实现，不能一次提交伪装完成。

## P15：先做的 5 项

| ID | 任务 | 状态 | 下一步 |
| --- | --- | --- | --- |
| P15-1 | 修正文档残留 `70%` / 旧 `450+ target-only` live 口径 | done | 持续用 consistency scan 防回归 |
| P15-2 | release performance warning mitigation | ready | 已有 mitigation plan；P16 已接入 evidence report history，下一步优化启动/RSS/进程数 |
| P15-3 | 跨机器 SessionBundle portability smoke 门槛 | blocked-external | 按 `docs/sessionbundle-cross-machine-portability-runbook.md` 在第二环境执行 |
| P15-4 | provider production acceptance 门槛 | blocked-external | 按 `docs/provider-production-acceptance-runbook.md` 配凭证并跑真实 smoke |
| P15-5 | 首批 taxonomy family evidence-backed | done | 继续把更多 family 接入 collector/runtime path |

## 剩余工作全集

### Release / Distribution

1. 执行外部分发前 manual operator smoke。`blocked-external`
2. 在干净 Win11 机器验证安装、启动、卸载。`blocked-external`
3. 验证全页面可打开。`blocked-external`
4. 保持 release notes 与真实能力一致。`ready`
5. 优化 cold start：`9301ms -> <= 2000ms`。`large-slice`
6. 优化 idle RSS：`411MB -> <= 220MB`。`large-slice`
7. 优化 process count：`14 -> <= 4` 或记录例外。`large-slice`
8. 生成 release performance history。`done`
9. 将 release performance report 接入 Overview / Settings。`done-overview`

### Session / Portability

10. 第二环境导出/迁移/导入 bundle。`blocked-external`
11. target machine preflight evidence。`blocked-external`
12. target machine dry-run evidence。`blocked-external`
13. target machine confirmed restore evidence。`blocked-external`
14. restart continuity after restore evidence。`blocked-external`
15. profile portability UI 完善。`ready`
16. portability failure reason viewer。`partial-overview-history`
17. portability report history。`done-overview`

### Provider Closure

18. CAPTCHA credentials 配置。`blocked-external`
19. CAPTCHA manager wiring。`large-slice`
20. CAPTCHA CDP detect。`large-slice`
21. CAPTCHA CDP fill。`large-slice`
22. CAPTCHA real provider smoke。`blocked-external`
23. CAPTCHA operator UI closure。`large-slice`
24. SMS credentials 配置。`blocked-external`
25. SMS manager wiring。`large-slice`
26. SMS number purchase/status/cancel/finish flow。`large-slice`
27. SMS CDP detect/fill。`large-slice`
28. SMS real provider smoke。`blocked-external`
29. SMS operator UI closure。`large-slice`
30. Email session persistence hardening。`ready`
31. Email CDP detect/fill。`large-slice`
32. Email real registration-flow smoke。`blocked-external`
33. Provider failure reason taxonomy。`ready`
34. Provider evidence viewer。`partial-overview-history`
35. Provider acceptance report history。`done-overview`

### Fingerprint / Validation

36. WebGL observed collector。`large-slice`
37. font/text metrics observed collector。`large-slice`
38. media devices observed collector。`partial-desktop-webview`
39. timezone/locale observed collector。`partial-desktop-webview`
40. hardware/os observed collector。`partial-desktop-webview`
41. storage partitioning observed collector。`partial-desktop-webview`
42. screen/display observed collector。`partial-desktop-webview`
43. navigator/client hints observed collector。`partial-desktop-webview`
44. permission/device capability observed collector。`partial-desktop-webview`
45. detector/coherence observed matrix。`large-slice`
46. observed coverage dashboard。`ready`
47. observed coverage report history。`ready`
48. profile-level full fingerprint evidence export。`ready`
49. repeatability sampling report。`ready`
50. desktop WebView vs profile browser comparison。`ready`

### 450 Fingerprint Taxonomy

51. 为全部 fingerprint family 定义 collector path。`large-slice`
52. 为全部 signal 定义 declared/applied/observed layer。`large-slice`
53. 为全部 signal 定义 adapter support。`large-slice`
54. 为全部 signal 定义 failure mode。`large-slice`
55. 为全部 signal 定义 repeatability rule。`large-slice`
56. 为全部 signal 定义 evidence schema。`large-slice`
57. signal coverage dashboard。`ready`
58. signal audit report。`ready`
59. signal version / collector version 管理。`ready`

### 450 Behavior Taxonomy

60. 为全部 behavior family 定义 replay semantics。`large-slice`
61. 为全部 behavior family 定义 audit payload。`large-slice`
62. 为全部 behavior family 定义 failure states。`large-slice`
63. 为全部 behavior family 定义 recovery behavior。`large-slice`
64. 扩展 `13` shipped primitives。`large-slice`
65. workflow graph runtime。`large-slice`
66. replay debugger。`large-slice`
67. per-event manual gate semantics。`large-slice`
68. deterministic replay evidence。`large-slice`
69. behavior event coverage dashboard。`ready`
70. behavior event audit report history。`partial-taxonomy-history`

### Runtime / Adapter / External Browser

71. headed runtime realism smoke。`large-slice`
72. headed external adapter contract implementation。`large-slice`
73. headed runtime fingerprint evidence。`large-slice`
74. headed runtime leak/coherence evidence。`large-slice`
75. Fake/Lightpanda/headed adapter compare report。`ready`
76. adapter capability matrix。`ready`
77. adapter failure reason taxonomy。`ready`
78. external browser process lifecycle contract。`large-slice`
79. external browser CDP/session attach contract。`large-slice`
80. external browser profile/runtime compatibility report。`ready`
81. keep Chromium/Firefox forks out of the main repo。`always-on`

### AdsPower / Benchmark

82. B1-B5 新证据后重跑 AdsPower refresh。`blocked-evidence`
83. 刷新官方公开 AdsPower source set。`ready`
84. 重算 capability score。`blocked-evidence`
85. 生成 current/target/AdsPower comparison table。`blocked-evidence`
86. 生成 next-gap prioritization。`blocked-evidence`

### Docs / Governance

87. 每次变更后同步 root/docs/RUN_STATE/TODO。`always-on`
88. 每次变更跑 stage entry consistency。`always-on`
89. 防止历史 `30/70`、`77/23`、`82/18` 复活为 live truth。`always-on`
90. 防止 taxonomy seed 冒充 observed/replay runtime。`always-on`

## 说明

聊天里列出的 167 个细项已经归并为 90 个可执行工作包。展开实现时，每个工作包可拆回更细的 issue/task；不能把 `blocked-external` 或 `large-slice` 一次性标为完成。
