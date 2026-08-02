# Provider Production Acceptance Runbook

Updated: 2026-05-23 (Asia/Shanghai)

## 目标

把 CAPTCHA / SMS / Email 从 readiness surface 推进到真实 provider acceptance。当前状态是 readiness、blockers、preflight 已可见；没有凭证、manager wiring、CDP detect/fill 和真实 smoke 时不能写成 production closure。

## 通用退出条件

每个 provider domain 至少要保存一份 report，包含：

- domain：`captcha` / `sms` / `email`
- provider name
- credential status
- manager wiring status
- CDP detect status
- CDP fill status
- operator UI status
- real provider smoke status
- failure reason
- report path

## CAPTCHA 验收

1. 配置 `TWOCAPTCHA_API_KEY` 或 `CAPSOLVER_API_KEY`。
2. 将 production solver manager 接入 runtime task flow。
3. 通过 CDP/page probe 检测 challenge 和 sitekey。
4. 调用 provider 获取 token/result。
5. 通过 CDP/browser automation 填入 token/result。
6. 保存 provider response、fill result、页面结果和 failure reason。

## SMS 验收

1. 配置 `FIVESIM_API_KEY`、`SMSPOOL_API_KEY` 或等价 provider。
2. 将 SMS manager 和 provider selection config 接入 runtime flow。
3. 购买/申请号码。
4. 通过 CDP 检测 phone/code fields。
5. 填入手机号，等待 code，填入验证码。
6. 处理 cancel/finish/failure，并保存状态流证据。

## Email 验收

1. 配置 mail.tm 或 worker endpoint。
2. 保证 email session identity 能贯穿注册流程。
3. 创建 inbox，等待 code。
4. 通过 CDP 检测 email/code fields。
5. 填入邮箱和验证码。
6. 保存真实 registration-flow smoke evidence。

## 阻塞规则

- 只有 credential present 不能算 ready。
- 只有 manager wiring 不能算 provider acceptance。
- 只有 CDP detect 没有 fill/result 不能算 closure。
- 没有真实 provider response 或 sandbox response，不能写 production closure。
