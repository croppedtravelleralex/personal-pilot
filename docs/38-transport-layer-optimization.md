# 传输层特征优化设计

> 本文档描述 HTTP 传输层的特征优化——通过控制 TLS 握手参数、HTTP/2 帧序列、HTTP 头顺序等传输层特征，使浏览器流量的网络层特征与真实浏览器不可区分。

---

## 0. 术语表

| 术语 | 含义 |
|------|------|
| TLS 握手 (TLS Handshake) | 客户端与服务器建立加密连接时的参数协商过程 |
| JA3/JA4 指纹 | 基于 TLS ClientHello 参数的哈希值，用于识别 TLS 栈实现 |
| HTTP/2 帧序 | HTTP/2 连接的初始 SETTINGS/WINDOW_UPDATE 帧发送顺序 |
| TCP 初始窗口 | TCP 连接建立时的初始拥塞窗口大小 |
| 请求头顺序 | HTTP 请求头字段的排列顺序 |

## 一、架构

```
┌─────────────────────────────────────────────────────────────┐
│                 Transport Layer Optimizer                     │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│  TLS Level                    HTTP/2 Level                   │
│  ┌──────────────────┐        ┌──────────────────────┐       │
│  │ Cipher Suite      │        │  Initial SETTINGS    │       │
│  │ 顺序模板           │        │  帧序随机化          │       │
│  │ TLSCiphers[]      │        │  SETTINGS[]         │       │
│  └──────────────────┘        └──────────────────────┘       │
│  ┌──────────────────┐        ┌──────────────────────┐       │
│  │ Extension Order   │        │  PING / WINDOW_UPDATE│       │
│  │ 扩展顺序随机化    │        │  间隔抖动            │       │
│  │ Extensions[]      │        │  Interval_jitter     │       │
│  └──────────────────┘        └──────────────────────┘       │
│                                                              │
│  HTTP Level                      TCP Level                   │
│  ┌──────────────────┐        ┌──────────────────────┐       │
│  │ Header Order      │        │  TCP Init Window     │       │
│  │ 请求头顺序模板     │        │  初始窗口随机化      │       │
│  │ Headers[]         │        │  TCPInitWnd[10-44]   │       │
│  └──────────────────┘        └──────────────────────┘       │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

## 二、TLS 层优化

### 2.1 密码套件顺序

```go
type TLSCipherSuite string

// 真实 Chrome 139 的密码套件顺序
var Chrome139Suites = []TLSCipherSuite{
    "TLS_AES_128_GCM_SHA256",
    "TLS_AES_256_GCM_SHA384",
    "TLS_CHACHA20_POLY1305_SHA256",
    "ECDHE-ECDSA-AES128-GCM-SHA256",
    "ECDHE-RSA-AES128-GCM-SHA256",
    // ...
}

// 每次连接从完整模板中选取子集（选 8-12 个）
// 顺序保持不变，但可选集合在多个模板间轮换
type TLSFingerprint struct {
    CipherSuites    []TLSCipherSuite  // 密码套件顺序
    Extensions      []TLSExtension    // 扩展顺序
    SupportedGroups []string          // 椭圆曲线
    ECPointFormats  []string          // 椭圆曲线点格式
}
```

### 2.2 JA3/4 轮换库

```go
// 从真实浏览器的 TLS 采集样本
var JA3Profiles = []TLSFingerprint{
    // Chrome 139 on Win10
    {CipherSuites: [...], Extensions: [...], Groups: ["x25519", "secp256r1"]},
    // Chrome 142 on Win11
    {CipherSuites: [...], Extensions: [...], Groups: ["x25519", "secp256r1", "secp384r1"]},
    // Chrome 144 on Win10
    {CipherSuites: [...], Extensions: [...], Groups: ["x25519"]},
}

// 每个浏览器实例启动时从库中选择一个 TLS 指纹
// 与所使用的 Chromium 版本关联
```

## 三、HTTP/2 层优化

### 3.1 初始 SETTINGS 帧

```
真实 Chrome 的 SETTINGS 帧:
  SETTINGS_HEADER_TABLE_SIZE = 65536
  SETTINGS_ENABLE_PUSH = 0
  SETTINGS_MAX_CONCURRENT_STREAMS = 1000
  SETTINGS_INITIAL_WINDOW_SIZE = 6291456
  SETTINGS_MAX_FRAME_SIZE = 16384
  SETTINGS_MAX_HEADER_LIST_SIZE = 262144

随机化策略:
  - HEADER_TABLE_SIZE: 在 4096-65536 之间选择常见值
  - MAX_CONCURRENT_STREAMS: 100 或 1000
  - INITIAL_WINDOW_SIZE: 6291456 或 1048576
  - 帧顺序: 在前 3 帧内随机化 SETTINGS/WINDOW_UPDATE 顺序
```

### 3.2 请求头顺序

```
Chrome 的真实请求头顺序模板:
  :method
  :authority  
  :scheme
  :path
  sec-ch-ua
  sec-ch-ua-mobile
  sec-ch-ua-platform
  user-agent
  content-type
  accept
  origin
  sec-fetch-site
  sec-fetch-mode
  sec-fetch-dest
  referer
  accept-encoding
  accept-language
  cookie

轻随机化策略:
  - sec-ch-* 块内顺序不变（浏览器固定）
  - 其余 header 在同类内轻微换位
  - cookie 始终在最后（浏览器行为）
```

## 四、TCP 层优化

| 参数 | 浏览器典型值 | 随机化范围 |
|------|------------|-----------|
| MSS | 1460 | 固定 |
| Window Scaling | 7 | 6-8 |
| Initial CWND | 10 | 10-44 |
| TTL | 64/128 | 64/128/255 选一 |
| SackPermitted | 1 | 固定 |
| 时间戳选项 | 开启 | 开启/关闭 |

## 五、与现有系统的集成

| 组件 | 集成方式 |
|------|---------|
| Chromium 启动参数 | `--ssl-version-min`, `--cipher-suite-blacklist` |
| Camoufox 配置 | Firefox 原生 about:config 参数 |
| Xray/Sing-Box 路由 | 出站连接的 TLS 配置 |
| 环境标识 Profile | TLS 指纹与浏览器版本关联存储 |
