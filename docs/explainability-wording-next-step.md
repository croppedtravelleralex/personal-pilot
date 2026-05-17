# Explainability 文案下一步（2026-04-02）

## 当前状态

当前 explainability 已经完成：
- trust score components 强类型化
- winner vs runner-up diff 结构化
- selection decision summary artifact 可读化
- verify 风险标签已做第一轮人类可读翻译

## 下一步最值得补的文案点

1. `/status` latest summary 中增加更稳定的短句模板
2. `/proxies/:id/explain` 中让 factor summary 更少依赖内部 label 语义
3. 区分“奖励项”和“惩罚项”的自然语言口吻，减少都写成同一种格式
4. 将 `provider_scope_flip / provider_region_scope_flip` 这类内部机制，继续限制在 perf / diagnostics 侧，不直接泄漏到用户向 summary 文案

## 当前判断

文案这条线值得继续做，但优先级仍低于写侧范围刷新优化。
