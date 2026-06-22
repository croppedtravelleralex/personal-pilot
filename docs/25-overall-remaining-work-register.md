# Overall Remaining Work Register

Updated: 2026-06-22 (Asia/Shanghai)

## 使用规则

本文件保留历史剩余工作登记。当前本机自用范围已闭环，唯一仍未验证的是 CAPTCHA/SMS/Email 服务商真实账号凭证 smoke。条目分为：

- `ready`：本机可继续实现或验证。
- `blocked-external`：需要用户提供凭证、真实 provider 或外部服务；外部分发 smoke、干净 Win11/第二机和跨机器 SessionBundle 已按本机自用范围取消，不再归入当前 blocked-external。
- `cancelled-local-only`：本机自用范围下明确取消；只保留历史文档、脚本或 report 作为诊断上下文。
- `large-slice`：需要拆成多轮实现，不能一次提交伪装完成。

## P15：先做的 5 项

| ID | 任务 | 状态 | 下一步 |
| --- | --- | --- | --- |
| P15-1 | 修正文档残留 `70%` / 旧 `450+ target-only` live 口径 | done | 持续用 consistency scan 防回归 |
| P15-2 | release performance warning mitigation | cancelled-local-only | 只保留 mitigation plan 和 M5 report 作为本机诊断；不再优化到预算 green |
| P15-3 | 跨机器 SessionBundle portability smoke 门槛 | cancelled-local-only | 第二环境、干净 Win11 和 `-CrossMachine` report 已取消 |
| P15-4 | provider production acceptance 门槛 | blocked-external | 仅真实账号凭证 smoke 未验；本地 readiness/dry-run/UI/report 已落地 |
| P15-5 | 首批 taxonomy family evidence-backed | done | 继续把更多 family 接入 collector/runtime path |

## 剩余工作全集

### Release / Distribution

1. 执行外部分发前 manual operator smoke。`cancelled-local-only`
2. 在干净 Win11 机器验证安装、启动、卸载。`cancelled-local-only`
3. 验证全页面可打开。`cancelled-local-only`
4. 保持 release notes 与真实能力一致。`ready`
5. 优化 cold start：`3899ms -> <= 2000ms`。`cancelled-local-only`
6. 优化 idle RSS：`433MB -> <= 220MB`。`cancelled-local-only`
7. 优化 process count：`9 -> <= 4` 或记录例外。`cancelled-local-only`
8. 生成 release performance history。`done-diagnostic`
9. 将 release performance report 接入 Overview / Settings。`done-diagnostic`

### Session / Portability

10. 第二环境导出/迁移/导入 bundle。`cancelled-local-only`
11. target machine preflight evidence。`cancelled-local-only`
12. target machine dry-run evidence。`cancelled-local-only`
13. target machine confirmed restore evidence。`cancelled-local-only`
14. restart continuity after restore evidence。`done-local-report`
15. 本机 profile restore UI 完善。`done-settings-operator`
16. 本机 restore failure reason viewer。`done-local-report`
17. 本机 SessionBundle report history。`done-overview`

### Provider Closure

18. CAPTCHA credentials 配置。`blocked-external`
19. CAPTCHA manager wiring。`provider-manager-gated`
20. CAPTCHA CDP detect。`provider-manager-gated`
21. CAPTCHA CDP fill。`provider-manager-gated`
22. CAPTCHA real provider smoke。`blocked-external`
23. CAPTCHA operator UI closure。`partial-settings-visible`
24. SMS credentials 配置。`blocked-external`
25. SMS manager wiring。`provider-manager-gated`
26. SMS number purchase/status/cancel/finish flow。`manager-contract-gated`
27. SMS CDP detect/fill。`provider-manager-gated`
28. SMS real provider smoke。`blocked-external`
29. SMS operator UI closure。`partial-settings-visible`
30. Email session persistence hardening。`manager-contract-gated`
31. Email CDP detect/fill。`provider-manager-gated`
32. Email real registration-flow smoke。`blocked-external`
33. Provider failure reason taxonomy。`partial-preflight-v2`
34. Provider evidence viewer。`partial-settings-latest-report`
35. Provider acceptance report history。`done-overview`

### Fingerprint / Validation

36. WebGL observed collector。`done-full-local-observed`
37. font/text metrics observed collector。`done-full-local-observed`
38. media devices observed collector。`done-full-local-observed`
39. timezone/locale observed collector。`done-full-local-observed`
40. hardware/os observed collector。`done-full-local-observed`
41. storage partitioning observed collector。`done-full-local-observed`
42. screen/display observed collector。`done-full-local-observed`
43. navigator/client hints observed collector。`done-full-local-observed`
44. permission/device capability observed collector。`done-full-local-observed`
45. detector/coherence observed matrix。`done-full-local-observed`
46. observed coverage dashboard。`done-overview`
47. observed coverage report history。`done-overview`
48. profile-level full fingerprint evidence export。`done-local-report`
49. repeatability sampling report。`done-m10-stability`
50. desktop WebView vs profile browser comparison。`passed_profile_browser_comparison_v3`

### 450 Fingerprint Taxonomy

51. 为全部 fingerprint family 定义 collector path。`done-taxonomy-metadata`
52. 为全部 signal 定义 declared/applied/observed layer。`done-family-metadata`
53. 为全部 signal 定义 adapter support。`done-family-metadata`
54. 为全部 signal 定义 failure mode。`done-family-metadata`
55. 为全部 signal 定义 repeatability rule。`done-family-metadata`
56. 为全部 signal 定义 evidence schema。`done-family-metadata`
57. signal coverage dashboard。`ready`
58. signal audit report。`ready`
59. signal version / collector version 管理。`ready`

### 450 Behavior Taxonomy

60. 为全部 behavior family 定义 replay semantics。`done-taxonomy-semantics`
61. 为全部 behavior family 定义 audit payload。`done-taxonomy-semantics`
62. 为全部 behavior family 定义 failure states。`done-taxonomy-semantics`
63. 为全部 behavior family 定义 recovery behavior。`done-taxonomy-semantics`
64. 扩展 `13` shipped primitives。`done-local-runtime-backed`
65. workflow graph runtime。`done-contract-visible`
66. replay debugger。`done-audit-visible`
67. per-event manual gate semantics。`done-local-runtime-backed`
68. deterministic replay evidence。`done-full-local-replay`
69. behavior event coverage dashboard。`ready`
70. behavior event audit report history。`partial-taxonomy-history`

### Runtime / Adapter / External Browser

71. headed runtime realism smoke。`done-m10-stability`
72. headed external adapter contract implementation。`done-local-runtime`
73. headed runtime fingerprint evidence。`done-full-local-observed`
74. headed runtime leak/coherence evidence。`done-m10-stability`
75. Fake/Lightpanda/headed adapter compare report。`done-runtime-adapter-report`
76. adapter capability matrix。`done-contract-visible`
77. adapter failure reason taxonomy。`partial-runtime-adapter-report`
78. external browser process lifecycle contract。`done-real-process-cleanup`
79. external browser CDP/session attach contract。`done-real-binary-cdp`
80. external browser profile/runtime compatibility report。`done-local-self-use`
81. keep Chromium/Firefox forks out of the main repo。`always-on`

### AdsPower / Benchmark

82. B1-B5 新证据后重跑 AdsPower refresh。`cancelled-local-only`
83. 刷新官方公开 AdsPower source set。`cancelled-local-only`
84. 重算 capability score。`cancelled-local-only`
85. 生成 current/target/AdsPower comparison table。`cancelled-local-only`
86. 生成 next-gap prioritization。`cancelled-local-only`

### Docs / Governance

87. 每次变更后同步 root/docs/RUN_STATE/TODO。`always-on`
88. 每次变更跑 stage entry consistency。`always-on`
89. 防止历史 `30/70`、`77/23`、`82/18` 复活为 live truth。`done-live-truth-guard`
90. 防止 taxonomy seed 冒充 observed/replay runtime。`done-live-truth-guard`

## 说明

聊天里列出的 167 个细项已经归并为 90 个工作包。当前本机自用闭环完成；外部分发、release performance budget、第二机/cross-machine 和 AdsPower 条目已标为 `cancelled-local-only` 以保留历史上下文。仍然不能把 CAPTCHA/SMS/Email 的 `blocked-external` 凭证 smoke 伪装成完成。
