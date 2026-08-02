# 实例生命周期管理系统设计

> 本文档描述浏览器实例的完整生命周期管理——包含**实例同步操作**、**快照创建与恢复**、**加密备份与跨机迁移**三大子系统。

---

## 0. 现有代码资产

| 子系统 | 代码文件 | 行数 |
|--------|---------|------|
| 实例同步器 | `backend/app_synchronizer.go` | 820 |
| 实例快照 | `backend/app_snapshot.go` | 394 |
| 备份系统 | `backend/app_backup*.go` (5个文件), `backend/internal/backup/` | — |
| 实例管理 | `backend/app_instance.go` | — |

## 一、实例同步器

### 1.1 功能
允许多个浏览器实例之间进行**状态同步**——如将一个实例的点击/输入/滚动操作同步复制到其他实例。

### 1.2 核心概念

```go
type SyncWindow struct {
    ID        string
    Groups    []SyncGroup        // 同步分组
    Mode      string             // broadcast / selected
}

type SyncGroup struct {
    ID         string
    Instances  []string          // 组内实例 ID 列表
    SyncOps    []string          // 同步操作类型: click / input / scroll / navigate
    MasterID   string            // 主控实例
}

// 同步流程:
//   主控实例产生操作 → EventBus 广播 → 各从实例接收 → 重放操作
//   支持: navigate / click / type / scroll / script
```

### 1.3 使用场景
| 场景 | 说明 |
|------|------|
| 一致性验证 | 同一操作在多个隔离环境中产生同一结果 |
| A/B 测试 | 同时对控制组和实验组执行操作 |
| 批量部署 | 一次操作同步到 N 个实例 |

## 二、实例快照系统

### 2.1 快照内容

```go
type SnapshotInfo struct {
    ID            string    `json:"id"`
    ProfileID     string    `json:"profileId"`
    DataDir       string    `json:"dataDir"`     // 用户数据目录
    SizeBytes     int64     `json:"sizeBytes"`
    Includes      []string  `json:"includes"`    // cookies / storage / cache / session
    CreatedAt     time.Time `json:"createdAt"`
}
```

### 2.2 快照/恢复流

```
创建快照:
  profile 运行中 → 触发 CDP 序列化
    ├── Cookie → Network.getAllCookies → Network.setCookie
    ├── LocalStorage → Runtime.evaluate
    ├── SessionStorage → Runtime.evaluate
    ├── IndexedDB → 数据目录 ZIP 打包
    └── 文件系统 → zipDir(userDataDir) → 写入备份目录

恢复快照:
  新 profile 启动 → 按 ID 加载快照
    ├── 解压数据目录
    ├── CDP 恢复 Cookies
    ├── CDP 恢复 Storage
    └── 恢复 Service Worker 注册
```

## 三、备份系统

### 3.1 备份范围

```go
type BackupScope string
const (
    ScopeProfile    BackupScope = "profile"     // 浏览器配置 + 数据目录
    ScopeSession    BackupScope = "session"     // 会话票据 + Cookie
    ScopeConfig     BackupScope = "config"      // 系统配置
    ScopeFull       BackupScope = "full"        // 以上全部
)
```

### 3.2 加密与传输

```go
// backend/internal/backup/encrypt.go
// AES-256-GCM 加密 → 压缩 → 可选远程存储
//
// Manifest 格式:
//   backup_manifest.json
//   ├── version, created_at, scope
//   ├── file_index (文件列表 + SHA256)
//   └── metadata (profile_id, session_ids)
```

### 3.3 跨机迁移

```
Export:  Profile + SessionBundle → AES 加密 → 单文件 (.ppbundle)
Import:  .ppbundle → AES 解密 → 校验 → 恢复到本地
```

## 四、API 端点

| 端点 | 方法 | 功能 |
|------|------|------|
| `/api/sync/window` | POST | 创建同步窗口 |
| `/api/sync/window/{id}` | DELETE | 关闭同步窗口 |
| `/api/snapshot` | POST | 创建快照 |
| `/api/snapshot/{id}/restore` | POST | 恢复快照 |
| `/api/backup` | POST | 创建备份 |
| `/api/backup/{id}/export` | GET | 导出加密包 |
| `/api/backup/import` | POST | 导入加密包 |
