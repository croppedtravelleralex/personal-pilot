# MODULE_SCOPE.md

第一阶段 Rust 模块骨架范围定义。

## 目标

先锁定首批模块，保证工程初始化时目录结构与设计文档一致。

---

## 第一阶段模块（P0）

### 1. `src/api/`
职责：
- REST API 路由
- 请求/响应结构
- 健康检查接口
- 任务创建/查询接口骨架

### 2. `src/domain/`
职责：
- 核心领域模型
- task / run / policy / profile / proxy 等 struct
- 状态枚举与基础规则

### 3. `src/db/`
职责：
- SQLite 连接
- repository 接口
- schema 初始化 / migration 占位

### 4. `src/queue/`
职责：
- 内存任务队列
- 入队/出队
- 最小调度接口

### 5. `src/runner/`
职责：
- runner trait
- fake runner 骨架
- real runner 适配占位

### 6. `src/network_identity/`
职责：
- 指纹模型
- 代理模型
- 网络策略模型
- 代理验证与分配占位

### 7. `src/app/`
职责：
- 应用层组合
- 把 api / db / queue / runner 组装起来

---

## 暂缓到第二阶段（P1）

- `src/harvester/`
- `src/validator/`
- `src/artifact/`
- `src/metrics/`
- `src/policy_engine/`

---

## 第一阶段目录建议

```text
AutoOpenBrowser/
  Cargo.toml
  src/
    main.rs
    lib.rs
    app/
    api/
    domain/
    db/
    queue/
    runner/
    network_identity/
```

---

## 结论

第一阶段工程骨架只做一件事：

> 让任务闭环、网络身份抽象、fake runner 演进路径，有明确的代码承载点。
