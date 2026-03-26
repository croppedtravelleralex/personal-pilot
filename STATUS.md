# STATUS.md

## 当前状态

- 状态：规划中 → 骨架初始化 → 文档驱动阶段 → 周期执行协议已落地
- 日期：2026-03-26

## 已完成

- 建立项目目录
- 建立 README / STATUS / TODO 基础文档
- 明确核心技术方向：Rust + SQLite + REST API + 内存任务队列 + fake runner
- 明确后续真实执行引擎：`lightpanda-io/browser`

## 当前未完成

- Rust 工程初始化
- 数据库 schema 设计
- REST API 路由定义
- 任务模型与状态流转
- fake runner 实现
- 与真实浏览器引擎的适配层

## 风险与注意点

- 需要尽早定义任务模型，否则 API / DB / runner 会反复返工
- 需要预留 fake runner 与 real runner 的统一接口
- SQLite 适合当前阶段，但后续若并发/吞吐升高要提前考虑边界

## 建议的下一步

1. 先产出开源项目参考清单与借鉴点
2. 初始化 Cargo 项目
3. 定义任务表与执行记录表
4. 定义 REST API 最小集合
5. 落一个 fake runner
6. 跑通最小闭环

## 当前任务补充

当前主任务已明确为：
- 借鉴开源项目
- 完善工程文档
- 再根据工程文档推进 app

详见 `CURRENT_TASK.md`。
