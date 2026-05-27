# 跨浏览器会话可移植性框架设计

> 本文档描述将浏览器会话状态（Cookie、Storage、环境标识）在不同浏览器运行时之间无缝迁移的框架。
> 目标是使"同一个人的会话"在 Chromium ↔ Camoufox ↔ Lightpanda 之间保持连续性。

---

## 0. 术语表

| 术语 | 含义 |
|------|------|
| 会话票据 (SessionBundle) | 跨机器可移植的会话状态序列化包 |
| 环境标识映射 (Signature Mapping) | 不同浏览器之间参数值的等价转换表 |
| 源运行时 (Source Runtime) | 导出会话的浏览器类型 |
| 目标运行时 (Target Runtime) | 导入会话的浏览器类型 |

## 一、跨运行时会话迁移

### 1.1 迁移内容

| 数据 | Chromium → | Camoufox → | Lightpanda → |
|------|-----------|-----------|-------------|
| Cookies | ✅ 直接注入 CDP | ⚠️ 需要 Gecko CDP 适配 | ✅ 直接注入 CDP |
| LocalStorage | ✅ | ⚠️ 兼容层 | ✅ |
| SessionStorage | ✅ | ⚠️ | ✅ |
| IndexedDB | ✅ 文件级复制 | ⚠️ | ⚠️ 无持久化存储 |
| ServiceWorker | ⚠️ 重新注册 | ⚠️ | ❌ |
| 环境标识 | ✅ 映射表 | ✅ 映射表 | ✅ 映射表 |

### 1.2 环境标识映射表

```json
{
  "chromium_139_to_camoufox": {
    "navigator.userAgent": "Mozilla/5.0... → Mozilla/5.0 (Gecko)...",
    "navigator.platform": "Win32 → Win32",
    "screen.width": "1920 → 1920",
    "navigator.hardwareConcurrency": "8 → 8"
  },
  "camoufox_to_chromium_139": { ... }
}
```

## 二、SessionBundle 跨机器移植规范

### 2.1 Bundle 结构

```
session_bundle/
├── manifest.json          # 元数据: created_at, source_runtime, profile_id
├── cookies.json           # Cookies (name, value, domain, path, secure, httponly)
├── storage.json           # localStorage + sessionStorage
├── fingerprint.json       # 环境标识参数快照
├── routing.json           # 出口路由配置
└── sessions/              # IndexedDB 文件 (option)
```

### 2.2 移植流程

```
1. Bundle 序列化:
   Cookie → Network.getAllCookies() 提取
   Storage → Runtime.evaluate 提取
   参数 → Profile Fingerprint 配置快照
   打包 → AES-256-GCM 加密 → .ppbundle 文件

2. Bundle 恢复:
   解密 → 校验 Manifest
   启动目标运行时 → 注入固定参数
   Cookie → Network.setCookie() 批量注入
   Storage → Runtime.evaluate 写入
   验证 → 确认所有状态恢复成功
```

## 三、参数映射策略

| 参数 | Chromium 原生值 | Camoufox 映射值 | Lightpanda 映射值 |
|------|----------------|----------------|-------------------|
| userAgent | Chrome/139 | Firefox/115 | Lightpanda/1.0 |
| platform | Win32 | Win32 | Linux |
| vendor | Google Inc. | — | — |
| plugins.length | 3 | 0 (Firefox 设计) | 0 |
| webdriver | undefined | undefined | true (可修改) |

## 四、API

| 端点 | 方法 | 功能 |
|------|------|------|
| `/api/session/export` | POST | 导出当前会话为一键迁移 Bundle |
| `/api/session/import` | POST | 从 Bundle 恢复会话 |
| `/api/session/migrate` | POST | 直接迁移到指定运行时 (export+import 一步完成) |
| `/api/session/validate` | POST | 校验 Bundle 完整性 |
| `/api/signature/map` | GET | 获取运行时间的参数映射表 |
