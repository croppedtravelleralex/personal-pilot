# LIGHTPANDA_V1_PLAN.md

`LightpandaRunner` 第一版真实接入边界定义。

目的：回答一个核心问题——

> `lightpanda` 的第一版，究竟先做成什么，才算“真实接入开始了”？

---

## 1. 当前判断

当前项目已经具备：

- 任务创建
- 入队
- runner 抽象
- fake runner 可执行
- run / log 状态回写
- `lightpanda` runner 占位入口

缺的是：

- `lightpanda` 的真实执行动作
- `lightpanda` 的输入约束
- `lightpanda` 的输出边界
- 失败语义与日志策略

所以第一版目标不应该过大。

---

## 2. V1 目标

`LightpandaRunner` V1 的目标不是做完整浏览器系统，而是：

> **把 `lightpanda` 从“失败型占位实现”推进到“最小真实执行器”。**

也就是说，第一版只要求它：

1. 能接到真实任务输入
2. 能识别最小必要字段（如 `url`）
3. 能执行一个最小真实动作
4. 能把执行结果回写到现有 run/task/result/log 链路
5. 在失败时给出明确错误，而不是统一占位文案

---

## 3. V1 建议边界

### 必做
- 支持从 `RunnerTask.payload` 读取 `url`
- 当 `url` 缺失时，返回明确参数错误
- 通过 `lightpanda` 执行一个最小页面访问动作（哪怕只是打开页面 / 导航）
- 返回结构化结果，至少包含：
  - `runner`
  - `url`
  - `action`
  - `ok`
  - `message`
- 失败时返回明确错误原因，而不是统一“not implemented”

### 可以先不做
- 复杂脚本执行
- artifact 落盘
- 截图
- 代理接入
- 指纹控制
- 多步骤任务编排
- running cancel
- 高级超时控制

---

## 4. V1 输入约定

建议第一版只接受：

```json
{
  "url": "https://example.com",
  "timeout_seconds": 10
}
```

其他字段即使存在，也先不承诺真正生效。

这样可以避免：

- API 已经收了太多字段
- runner 其实根本没实现
- 让调用侧误以为复杂能力已经可用

---

## 5. V1 输出约定

成功时建议返回：

```json
{
  "runner": "lightpanda",
  "url": "https://example.com",
  "action": "open_page",
  "ok": true,
  "message": "lightpanda visited url successfully"
}
```

失败时建议返回：

```json
{
  "runner": "lightpanda",
  "url": "https://example.com",
  "action": "open_page",
  "ok": false,
  "message": "...具体失败原因..."
}
```

---

## 6. V1 验收标准

只要满足以下条件，就算 V1 成立：

1. 设置 `PERSONA_PILOT_RUNNER=lightpanda` 后，系统不再只是返回占位失败
2. 传入合法 `url` 时，runner 至少执行一次真实页面访问
3. 成功与失败都能写回现有任务状态链路
4. 日志和结果能看出这是 `lightpanda` 的真实执行，而不是 fake 分支

---

## 7. V1 之后再做什么

V1 成立后，下一阶段再考虑：

1. `script` 支持
2. 页面结果提取
3. artifact（截图 / HTML）
4. 更严格 timeout
5. cancel_running
6. 代理 / 指纹接入

---

## 8. 当前建议

当前最合理的工程顺序：

1. 先定 V1 边界
2. 再补 `LightpandaRunner` 的参数校验与错误语义
3. 再接最小真实执行动作
4. 成立后再扩功能

不要一上来就把代理、指纹、截图、脚本执行全部堆进去。



## 9. 当前落地约定

V1 先通过本地二进制方式接入：

- 环境变量：`LIGHTPANDA_BIN`
- 默认命令：`lightpanda fetch <url>`
- 先回收 `stdout / stderr / exit_code` 到结果链路
- 先不实现 artifact、CDP、Playwright/ Puppeteer 桥接
