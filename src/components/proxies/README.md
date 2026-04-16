# Proxies Components

这里放 `Proxy Center` 专用展示组件。

建议拆成：

- `ProxyToolbar.tsx`
- `ProxyTable.tsx`
- `ProxyHealthCell.tsx`
- `ProxyUsagePanel.tsx`
- `ProxyFilterBar.tsx`

规则：

- 组件只负责展示和交互发射
- 不直接调用 `desktop.ts`
- 状态由 `src/features/proxies/*` 提供
