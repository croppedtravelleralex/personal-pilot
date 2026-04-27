package events

// EventDef describes an event for documentation and code generation.
type EventDef struct {
	Name        string       // e.g. "account:login:success"
	Namespace   string       // e.g. "account"
	Description string       // human-readable description
	Payload     []EventField // payload fields
	Severity    string       // "info" | "warn" | "error" | "critical"
}

// EventField describes a single field in an event's payload.
type EventField struct {
	Name     string // field name in JSON
	Type     string // Go type: "string", "int", "int64", "float64", "bool", "[]string", "map[string]interface{}"
	Required bool
}

// Registry maps every event name to its definition.
var Registry = map[string]EventDef{
	// ─── Browser Instance (existing) ──────────────────────────────────────────
	EventBrowserInstanceStarted: {
		Name: EventBrowserInstanceStarted, Namespace: "browser", Description: "浏览器实例已启动",
		Payload: []EventField{
			{Name: "profileId", Type: "string", Required: true},
			{Name: "profileName", Type: "string", Required: false},
			{Name: "debugPort", Type: "int", Required: false},
			{Name: "debugReady", Type: "bool", Required: false},
			{Name: "pid", Type: "int", Required: false},
			{Name: "reused", Type: "bool", Required: false},
			{Name: "running", Type: "bool", Required: false},
			{Name: "runtimeWarning", Type: "string", Required: false},
		},
		Severity: "info",
	},
	EventBrowserInstanceStopped: {
		Name: EventBrowserInstanceStopped, Namespace: "browser", Description: "浏览器实例已停止",
		Payload:  []EventField{{Name: "profileId", Type: "string", Required: true}},
		Severity: "info",
	},
	EventBrowserInstanceCrashed: {
		Name: EventBrowserInstanceCrashed, Namespace: "browser", Description: "浏览器实例异常崩溃",
		Payload: []EventField{
			{Name: "profileId", Type: "string", Required: true},
			{Name: "profileName", Type: "string", Required: false},
			{Name: "error", Type: "string", Required: true},
		},
		Severity: "error",
	},
	EventBrowserInstanceUpdated: {
		Name: EventBrowserInstanceUpdated, Namespace: "browser", Description: "浏览器实例状态更新",
		Payload: []EventField{
			{Name: "profileId", Type: "string", Required: true},
			{Name: "profileName", Type: "string", Required: false},
			{Name: "debugPort", Type: "int", Required: false},
			{Name: "debugReady", Type: "bool", Required: false},
			{Name: "pid", Type: "int", Required: false},
			{Name: "running", Type: "bool", Required: false},
			{Name: "runtimeWarning", Type: "string", Required: false},
		},
		Severity: "info",
	},

	// ─── Proxy Bridge (existing) ─────────────────────────────────────────────
	EventProxyBridgeDied: {
		Name: EventProxyBridgeDied, Namespace: "proxy", Description: "代理桥接进程死亡",
		Payload: []EventField{
			{Name: "engine", Type: "string", Required: true},
			{Name: "key", Type: "string", Required: false},
			{Name: "error", Type: "string", Required: true},
		},
		Severity: "error",
	},
	EventProxyBridgeFailed: {
		Name: EventProxyBridgeFailed, Namespace: "proxy", Description: "代理桥接启动失败",
		Payload: []EventField{
			{Name: "profileId", Type: "string", Required: true},
			{Name: "profileName", Type: "string", Required: false},
			{Name: "error", Type: "string", Required: true},
		},
		Severity: "error",
	},

	// ─── Proxy Quality (existing) ────────────────────────────────────────────
	EventProxySpeedResult: {
		Name: EventProxySpeedResult, Namespace: "proxy", Description: "代理测速结果",
		Payload: []EventField{
			{Name: "proxyId", Type: "string", Required: true},
			{Name: "ok", Type: "bool", Required: true},
			{Name: "latencyMs", Type: "int64", Required: false},
			{Name: "error", Type: "string", Required: false},
		},
		Severity: "info",
	},
	EventProxyIPHealthResult: {
		Name: EventProxyIPHealthResult, Namespace: "proxy", Description: "代理出口 IP 健康检测结果",
		Payload: []EventField{
			{Name: "proxyId", Type: "string", Required: true},
			{Name: "ok", Type: "bool", Required: true},
			{Name: "ip", Type: "string", Required: false},
			{Name: "fraudScore", Type: "int64", Required: false},
			{Name: "isResidential", Type: "bool", Required: false},
			{Name: "country", Type: "string", Required: false},
			{Name: "city", Type: "string", Required: false},
		},
		Severity: "info",
	},

	// ─── Risk: Fingerprint (existing) ────────────────────────────────────────
	EventRiskFingerprintMismatch: {
		Name: EventRiskFingerprintMismatch, Namespace: "risk", Description: "CDP 指纹验证发现偏差",
		Payload: []EventField{
			{Name: "profileId", Type: "string", Required: true},
			{Name: "mismatches", Type: "[]string", Required: true},
		},
		Severity: "warn",
	},
	EventRiskFingerprintTimezoneIP: {
		Name: EventRiskFingerprintTimezoneIP, Namespace: "risk", Description: "浏览器时区与代理 IP 地理位置不匹配",
		Payload: []EventField{
			{Name: "profileId", Type: "string", Required: true},
			{Name: "ipCountry", Type: "string", Required: true},
			{Name: "ipCity", Type: "string", Required: false},
			{Name: "timezoneIana", Type: "string", Required: true},
			{Name: "isMismatch", Type: "bool", Required: true},
			{Name: "mismatchScore", Type: "int", Required: true},
		},
		Severity: "warn",
	},

	// ─── Risk: Proxy (existing) ──────────────────────────────────────────────
	EventRiskProxyHighLatency: {
		Name: EventRiskProxyHighLatency, Namespace: "risk", Description: "代理延迟超过 3000ms",
		Payload: []EventField{
			{Name: "proxyId", Type: "string", Required: true},
			{Name: "latencyMs", Type: "int64", Required: true},
		},
		Severity: "warn",
	},
	EventRiskProxyHealthDrop: {
		Name: EventRiskProxyHealthDrop, Namespace: "risk", Description: "代理 IP 欺诈分骤升（>=70）",
		Payload: []EventField{
			{Name: "proxyId", Type: "string", Required: true},
			{Name: "ip", Type: "string", Required: false},
			{Name: "fraudScore", Type: "int64", Required: true},
		},
		Severity: "error",
	},
	EventRiskProxyDatacenter: {
		Name: EventRiskProxyDatacenter, Namespace: "risk", Description: "代理 IP 被识别为机房/托管",
		Payload: []EventField{
			{Name: "proxyId", Type: "string", Required: true},
			{Name: "ip", Type: "string", Required: false},
			{Name: "fraudScore", Type: "int64", Required: false},
			{Name: "isResidential", Type: "bool", Required: false},
			{Name: "country", Type: "string", Required: false},
		},
		Severity: "warn",
	},
	EventRiskProxyAuthFailure: {
		Name: EventRiskProxyAuthFailure, Namespace: "risk", Description: "代理认证被拒",
		Payload: []EventField{
			{Name: "proxyId", Type: "string", Required: true},
			{Name: "error", Type: "string", Required: true},
		},
		Severity: "error",
	},

	// ─── Risk: Network (existing) ────────────────────────────────────────────
	EventRiskWebRTCLeak: {
		Name: EventRiskWebRTCLeak, Namespace: "risk", Description: "WebRTC 泄露了真实 IP",
		Payload: []EventField{
			{Name: "profileId", Type: "string", Required: true},
			{Name: "localIP", Type: "string", Required: false},
			{Name: "publicIP", Type: "string", Required: false},
		},
		Severity: "critical",
	},
	EventRiskDNSLeak: {
		Name: EventRiskDNSLeak, Namespace: "risk", Description: "DNS 解析绕过了代理",
		Payload: []EventField{
			{Name: "profileId", Type: "string", Required: true},
			{Name: "leakedDomains", Type: "[]string", Required: false},
		},
		Severity: "critical",
	},

	// ─── Risk: Captcha (existing) ────────────────────────────────────────────
	EventRiskCaptchaDetected: {
		Name: EventRiskCaptchaDetected, Namespace: "risk", Description: "浏览器出现验证码",
		Payload: []EventField{
			{Name: "profileId", Type: "string", Required: true},
			{Name: "url", Type: "string", Required: false},
			{Name: "captchaType", Type: "string", Required: false},
		},
		Severity: "warn",
	},
	EventRiskCaptchaFailed: {
		Name: EventRiskCaptchaFailed, Namespace: "risk", Description: "验证码解决失败",
		Payload: []EventField{
			{Name: "profileId", Type: "string", Required: true},
			{Name: "captchaType", Type: "string", Required: false},
			{Name: "error", Type: "string", Required: true},
		},
		Severity: "error",
	},

	// ─── Risk: Browser (existing) ────────────────────────────────────────────
	EventRiskBrowserCrashLoop: {
		Name: EventRiskBrowserCrashLoop, Namespace: "risk", Description: "同一实例 5 分钟内崩溃 3+ 次",
		Payload: []EventField{
			{Name: "profileId", Type: "string", Required: true},
			{Name: "profileName", Type: "string", Required: false},
			{Name: "crashCount", Type: "int", Required: true},
			{Name: "window", Type: "string", Required: false},
		},
		Severity: "critical",
	},

	// ─── Risk: Session (existing) ────────────────────────────────────────────
	EventRiskSessionRateLimit: {
		Name: EventRiskSessionRateLimit, Namespace: "risk", Description: "小红书返回限流响应",
		Payload: []EventField{
			{Name: "profileId", Type: "string", Required: true},
			{Name: "url", Type: "string", Required: false},
			{Name: "statusCode", Type: "int", Required: false},
		},
		Severity: "warn",
	},
	EventRiskSessionCookieCleared: {
		Name: EventRiskSessionCookieCleared, Namespace: "risk", Description: "会话 Cookie 意外清除",
		Payload: []EventField{
			{Name: "profileId", Type: "string", Required: true},
			{Name: "domain", Type: "string", Required: false},
		},
		Severity: "warn",
	},
	EventRiskSessionSecurityChallenge: {
		Name: EventRiskSessionSecurityChallenge, Namespace: "risk", Description: "小红书触发安全验证",
		Payload: []EventField{
			{Name: "profileId", Type: "string", Required: true},
			{Name: "url", Type: "string", Required: false},
			{Name: "challengeType", Type: "string", Required: false},
		},
		Severity: "error",
	},

	// ─── Risk: System (existing) ─────────────────────────────────────────────
	EventRiskSystemLowDisk: {
		Name: EventRiskSystemLowDisk, Namespace: "risk", Description: "磁盘空间不足 5%",
		Payload: []EventField{
			{Name: "diskPath", Type: "string", Required: false},
			{Name: "freePercent", Type: "float64", Required: true},
			{Name: "freeGB", Type: "float64", Required: false},
		},
		Severity: "critical",
	},
	EventRiskSystemMemoryPressure: {
		Name: EventRiskSystemMemoryPressure, Namespace: "risk", Description: "内存使用超过 80%",
		Payload: []EventField{
			{Name: "usedPercent", Type: "float64", Required: true},
			{Name: "usedMB", Type: "int64", Required: false},
			{Name: "totalMB", Type: "int64", Required: false},
		},
		Severity: "warn",
	},

	// ─── Risk: Profile (existing) ────────────────────────────────────────────
	EventRiskProfileCorrupted: {
		Name: EventRiskProfileCorrupted, Namespace: "risk", Description: "Profile 目录损坏",
		Payload: []EventField{
			{Name: "profileId", Type: "string", Required: true},
			{Name: "error", Type: "string", Required: true},
		},
		Severity: "critical",
	},

	// ─── Risk: Node (existing) ───────────────────────────────────────────────
	EventRiskNodeBanned: {
		Name: EventRiskNodeBanned, Namespace: "risk", Description: "代理节点被封",
		Payload: []EventField{
			{Name: "proxyId", Type: "string", Required: true},
			{Name: "ip", Type: "string", Required: false},
			{Name: "reason", Type: "string", Required: false},
		},
		Severity: "critical",
	},
	EventRiskNodeGeoJump: {
		Name: EventRiskNodeGeoJump, Namespace: "risk", Description: "节点出口 IP 跳变",
		Payload: []EventField{
			{Name: "proxyId", Type: "string", Required: true},
			{Name: "oldIP", Type: "string", Required: false},
			{Name: "newIP", Type: "string", Required: false},
		},
		Severity: "warn",
	},
	EventRiskNodeOffline: {
		Name: EventRiskNodeOffline, Namespace: "risk", Description: "代理节点不可达",
		Payload: []EventField{
			{Name: "proxyId", Type: "string", Required: true},
			{Name: "error", Type: "string", Required: false},
		},
		Severity: "error",
	},

	// ─── App (existing) ──────────────────────────────────────────────────────
	EventAppRequestClose: {
		Name: EventAppRequestClose, Namespace: "app", Description: "请求关闭应用",
		Payload:  []EventField{},
		Severity: "info",
	},

	// ─── Download (existing) ─────────────────────────────────────────────────
	EventDownloadProgress: {
		Name: EventDownloadProgress, Namespace: "download", Description: "内核下载进度",
		Payload: []EventField{
			{Name: "phase", Type: "string", Required: true},
			{Name: "progress", Type: "int", Required: true},
			{Name: "message", Type: "string", Required: false},
		},
		Severity: "info",
	},

	// ═══════════════════════════════════════════════════════════════════════════
	// Phase 2: New Events
	// ═══════════════════════════════════════════════════════════════════════════

	// ─── Account: Login ──────────────────────────────────────────────────────
	"account:login:attempt": {
		Name: "account:login:attempt", Namespace: "account", Description: "账号登录尝试",
		Payload: []EventField{
			{Name: "profileId", Type: "string", Required: true},
			{Name: "accountName", Type: "string", Required: false},
			{Name: "loginMethod", Type: "string", Required: false},
		},
		Severity: "info",
	},
	"account:login:success": {
		Name: "account:login:success", Namespace: "account", Description: "账号登录成功",
		Payload: []EventField{
			{Name: "profileId", Type: "string", Required: true},
			{Name: "accountName", Type: "string", Required: true},
			{Name: "accountId", Type: "string", Required: false},
			{Name: "loginDuration", Type: "int64", Required: false},
		},
		Severity: "info",
	},
	"account:login:failed": {
		Name: "account:login:failed", Namespace: "account", Description: "账号登录失败",
		Payload: []EventField{
			{Name: "profileId", Type: "string", Required: true},
			{Name: "accountName", Type: "string", Required: false},
			{Name: "reason", Type: "string", Required: true},
		},
		Severity: "error",
	},
	"account:login:captcha": {
		Name: "account:login:captcha", Namespace: "account", Description: "登录触发验证码",
		Payload: []EventField{
			{Name: "profileId", Type: "string", Required: true},
			{Name: "accountName", Type: "string", Required: false},
			{Name: "captchaType", Type: "string", Required: false},
		},
		Severity: "warn",
	},
	"account:login:verify": {
		Name: "account:login:verify", Namespace: "account", Description: "需要二次验证（短信/扫码）",
		Payload: []EventField{
			{Name: "profileId", Type: "string", Required: true},
			{Name: "accountName", Type: "string", Required: false},
			{Name: "verifyMethod", Type: "string", Required: true},
		},
		Severity: "warn",
	},
	"account:login:blocked": {
		Name: "account:login:blocked", Namespace: "account", Description: "账号被限制登录",
		Payload: []EventField{
			{Name: "profileId", Type: "string", Required: true},
			{Name: "accountName", Type: "string", Required: false},
			{Name: "blockReason", Type: "string", Required: false},
			{Name: "blockUntil", Type: "string", Required: false},
		},
		Severity: "critical",
	},
	"account:login:timeout": {
		Name: "account:login:timeout", Namespace: "account", Description: "登录超时",
		Payload: []EventField{
			{Name: "profileId", Type: "string", Required: true},
			{Name: "accountName", Type: "string", Required: false},
			{Name: "timeoutSeconds", Type: "int", Required: false},
		},
		Severity: "error",
	},
	"account:login:session-expired": {
		Name: "account:login:session-expired", Namespace: "account", Description: "登录会话过期",
		Payload: []EventField{
			{Name: "profileId", Type: "string", Required: true},
			{Name: "accountName", Type: "string", Required: false},
		},
		Severity: "warn",
	},

	// ─── Account: Profile ────────────────────────────────────────────────────
	"account:profile:updated": {
		Name: "account:profile:updated", Namespace: "account", Description: "账号资料已更新",
		Payload: []EventField{
			{Name: "profileId", Type: "string", Required: true},
			{Name: "fields", Type: "[]string", Required: false},
		},
		Severity: "info",
	},
	"account:profile:avatar-changed": {
		Name: "account:profile:avatar-changed", Namespace: "account", Description: "头像已更换",
		Payload: []EventField{
			{Name: "profileId", Type: "string", Required: true},
		},
		Severity: "info",
	},
	"account:profile:verified": {
		Name: "account:profile:verified", Namespace: "account", Description: "账号认证状态变更",
		Payload: []EventField{
			{Name: "profileId", Type: "string", Required: true},
			{Name: "verifyType", Type: "string", Required: false},
			{Name: "status", Type: "string", Required: true},
		},
		Severity: "info",
	},
	"account:profile:reported": {
		Name: "account:profile:reported", Namespace: "account", Description: "账号被举报",
		Payload: []EventField{
			{Name: "profileId", Type: "string", Required: true},
			{Name: "reportReason", Type: "string", Required: false},
		},
		Severity: "critical",
	},

	// ─── Account: Follow ─────────────────────────────────────────────────────
	"account:follow:follow": {
		Name: "account:follow:follow", Namespace: "account", Description: "关注用户",
		Payload: []EventField{
			{Name: "profileId", Type: "string", Required: true},
			{Name: "targetUserId", Type: "string", Required: false},
			{Name: "targetUserName", Type: "string", Required: false},
		},
		Severity: "info",
	},
	"account:follow:unfollow": {
		Name: "account:follow:unfollow", Namespace: "account", Description: "取消关注用户",
		Payload: []EventField{
			{Name: "profileId", Type: "string", Required: true},
			{Name: "targetUserId", Type: "string", Required: false},
		},
		Severity: "info",
	},
	"account:follow:block": {
		Name: "account:follow:block", Namespace: "account", Description: "拉黑用户",
		Payload: []EventField{
			{Name: "profileId", Type: "string", Required: true},
			{Name: "targetUserId", Type: "string", Required: false},
		},
		Severity: "info",
	},
	"account:follow:mute": {
		Name: "account:follow:mute", Namespace: "account", Description: "静音用户",
		Payload: []EventField{
			{Name: "profileId", Type: "string", Required: true},
			{Name: "targetUserId", Type: "string", Required: false},
		},
		Severity: "info",
	},

	// ─── Content: Publish ────────────────────────────────────────────────────
	"content:publish:draft": {
		Name: "content:publish:draft", Namespace: "content", Description: "笔记保存为草稿",
		Payload: []EventField{
			{Name: "profileId", Type: "string", Required: true},
			{Name: "noteId", Type: "string", Required: false},
			{Name: "title", Type: "string", Required: false},
		},
		Severity: "info",
	},
	"content:publish:scheduled": {
		Name: "content:publish:scheduled", Namespace: "content", Description: "笔记已定时发布",
		Payload: []EventField{
			{Name: "profileId", Type: "string", Required: true},
			{Name: "noteId", Type: "string", Required: false},
			{Name: "scheduledAt", Type: "string", Required: true},
		},
		Severity: "info",
	},
	"content:publish:submitted": {
		Name: "content:publish:submitted", Namespace: "content", Description: "笔记已提交发布",
		Payload: []EventField{
			{Name: "profileId", Type: "string", Required: true},
			{Name: "noteId", Type: "string", Required: false},
		},
		Severity: "info",
	},
	"content:publish:processing": {
		Name: "content:publish:processing", Namespace: "content", Description: "笔记审核中",
		Payload: []EventField{
			{Name: "profileId", Type: "string", Required: true},
			{Name: "noteId", Type: "string", Required: false},
		},
		Severity: "info",
	},
	"content:publish:published": {
		Name: "content:publish:published", Namespace: "content", Description: "笔记发布成功",
		Payload: []EventField{
			{Name: "profileId", Type: "string", Required: true},
			{Name: "noteId", Type: "string", Required: true},
			{Name: "noteUrl", Type: "string", Required: false},
		},
		Severity: "info",
	},
	"content:publish:failed": {
		Name: "content:publish:failed", Namespace: "content", Description: "笔记发布失败",
		Payload: []EventField{
			{Name: "profileId", Type: "string", Required: true},
			{Name: "noteId", Type: "string", Required: false},
			{Name: "reason", Type: "string", Required: true},
		},
		Severity: "error",
	},
	"content:publish:reviewed": {
		Name: "content:publish:reviewed", Namespace: "content", Description: "笔记审核结果",
		Payload: []EventField{
			{Name: "profileId", Type: "string", Required: true},
			{Name: "noteId", Type: "string", Required: true},
			{Name: "status", Type: "string", Required: true},
			{Name: "reviewNote", Type: "string", Required: false},
		},
		Severity: "info",
	},
	"content:publish:deleted": {
		Name: "content:publish:deleted", Namespace: "content", Description: "笔记已删除",
		Payload: []EventField{
			{Name: "profileId", Type: "string", Required: true},
			{Name: "noteId", Type: "string", Required: true},
		},
		Severity: "info",
	},

	// ─── Content: Image ──────────────────────────────────────────────────────
	"content:image:upload-start": {
		Name: "content:image:upload-start", Namespace: "content", Description: "图片开始上传",
		Payload: []EventField{
			{Name: "profileId", Type: "string", Required: true},
			{Name: "fileName", Type: "string", Required: false},
			{Name: "fileSize", Type: "int64", Required: false},
		},
		Severity: "info",
	},
	"content:image:progress": {
		Name: "content:image:progress", Namespace: "content", Description: "图片上传进度",
		Payload: []EventField{
			{Name: "profileId", Type: "string", Required: true},
			{Name: "progress", Type: "int", Required: true},
			{Name: "fileName", Type: "string", Required: false},
		},
		Severity: "info",
	},
	"content:image:done": {
		Name: "content:image:done", Namespace: "content", Description: "图片上传完成",
		Payload: []EventField{
			{Name: "profileId", Type: "string", Required: true},
			{Name: "imageUrl", Type: "string", Required: false},
			{Name: "imageId", Type: "string", Required: false},
		},
		Severity: "info",
	},
	"content:image:failed": {
		Name: "content:image:failed", Namespace: "content", Description: "图片上传失败",
		Payload: []EventField{
			{Name: "profileId", Type: "string", Required: true},
			{Name: "fileName", Type: "string", Required: false},
			{Name: "reason", Type: "string", Required: true},
		},
		Severity: "error",
	},

	// ─── Content: Comment ────────────────────────────────────────────────────
	"content:comment:post": {
		Name: "content:comment:post", Namespace: "content", Description: "发布评论",
		Payload: []EventField{
			{Name: "profileId", Type: "string", Required: true},
			{Name: "noteId", Type: "string", Required: false},
			{Name: "commentId", Type: "string", Required: false},
		},
		Severity: "info",
	},
	"content:comment:reply": {
		Name: "content:comment:reply", Namespace: "content", Description: "回复评论",
		Payload: []EventField{
			{Name: "profileId", Type: "string", Required: true},
			{Name: "noteId", Type: "string", Required: false},
			{Name: "parentCommentId", Type: "string", Required: false},
		},
		Severity: "info",
	},
	"content:comment:delete": {
		Name: "content:comment:delete", Namespace: "content", Description: "删除评论",
		Payload: []EventField{
			{Name: "profileId", Type: "string", Required: true},
			{Name: "commentId", Type: "string", Required: true},
		},
		Severity: "info",
	},
	"content:comment:report": {
		Name: "content:comment:report", Namespace: "content", Description: "举报评论",
		Payload: []EventField{
			{Name: "profileId", Type: "string", Required: true},
			{Name: "commentId", Type: "string", Required: true},
			{Name: "reason", Type: "string", Required: false},
		},
		Severity: "info",
	},

	// ─── Content: Like ───────────────────────────────────────────────────────
	"content:like:like": {
		Name: "content:like:like", Namespace: "content", Description: "点赞笔记",
		Payload: []EventField{
			{Name: "profileId", Type: "string", Required: true},
			{Name: "noteId", Type: "string", Required: false},
		},
		Severity: "info",
	},
	"content:like:unlike": {
		Name: "content:like:unlike", Namespace: "content", Description: "取消点赞",
		Payload: []EventField{
			{Name: "profileId", Type: "string", Required: true},
			{Name: "noteId", Type: "string", Required: false},
		},
		Severity: "info",
	},

	// ─── Automation: Task ────────────────────────────────────────────────────
	"automation:task:created": {
		Name: "automation:task:created", Namespace: "automation", Description: "自动化任务已创建",
		Payload: []EventField{
			{Name: "taskId", Type: "string", Required: true},
			{Name: "taskName", Type: "string", Required: true},
			{Name: "triggerType", Type: "string", Required: true},
			{Name: "profileId", Type: "string", Required: false},
		},
		Severity: "info",
	},
	"automation:task:started": {
		Name: "automation:task:started", Namespace: "automation", Description: "自动化任务开始执行",
		Payload: []EventField{
			{Name: "taskId", Type: "string", Required: true},
			{Name: "taskName", Type: "string", Required: false},
			{Name: "profileId", Type: "string", Required: false},
		},
		Severity: "info",
	},
	"automation:task:progress": {
		Name: "automation:task:progress", Namespace: "automation", Description: "自动化任务执行进度",
		Payload: []EventField{
			{Name: "taskId", Type: "string", Required: true},
			{Name: "progress", Type: "int", Required: true},
			{Name: "currentStep", Type: "string", Required: false},
		},
		Severity: "info",
	},
	"automation:task:paused": {
		Name: "automation:task:paused", Namespace: "automation", Description: "自动化任务已暂停",
		Payload: []EventField{
			{Name: "taskId", Type: "string", Required: true},
		},
		Severity: "info",
	},
	"automation:task:resumed": {
		Name: "automation:task:resumed", Namespace: "automation", Description: "自动化任务已恢复",
		Payload: []EventField{
			{Name: "taskId", Type: "string", Required: true},
		},
		Severity: "info",
	},
	"automation:task:retried": {
		Name: "automation:task:retried", Namespace: "automation", Description: "自动化任务重试",
		Payload: []EventField{
			{Name: "taskId", Type: "string", Required: true},
			{Name: "attempt", Type: "int", Required: true},
			{Name: "maxRetries", Type: "int", Required: false},
		},
		Severity: "warn",
	},
	"automation:task:completed": {
		Name: "automation:task:completed", Namespace: "automation", Description: "自动化任务执行完成",
		Payload: []EventField{
			{Name: "taskId", Type: "string", Required: true},
			{Name: "duration", Type: "int64", Required: false},
			{Name: "result", Type: "map[string]interface{}", Required: false},
		},
		Severity: "info",
	},
	"automation:task:failed": {
		Name: "automation:task:failed", Namespace: "automation", Description: "自动化任务执行失败",
		Payload: []EventField{
			{Name: "taskId", Type: "string", Required: true},
			{Name: "error", Type: "string", Required: true},
			{Name: "step", Type: "string", Required: false},
		},
		Severity: "error",
	},
	"automation:task:cancelled": {
		Name: "automation:task:cancelled", Namespace: "automation", Description: "自动化任务已取消",
		Payload: []EventField{
			{Name: "taskId", Type: "string", Required: true},
		},
		Severity: "info",
	},
	"automation:task:expired": {
		Name: "automation:task:expired", Namespace: "automation", Description: "自动化任务已过期",
		Payload: []EventField{
			{Name: "taskId", Type: "string", Required: true},
			{Name: "expiredAt", Type: "string", Required: false},
		},
		Severity: "warn",
	},

	// ─── Automation: Batch ───────────────────────────────────────────────────
	"automation:batch:started": {
		Name: "automation:batch:started", Namespace: "automation", Description: "批量任务开始",
		Payload: []EventField{
			{Name: "batchId", Type: "string", Required: true},
			{Name: "totalTasks", Type: "int", Required: true},
		},
		Severity: "info",
	},
	"automation:batch:item-complete": {
		Name: "automation:batch:item-complete", Namespace: "automation", Description: "批量任务单项完成",
		Payload: []EventField{
			{Name: "batchId", Type: "string", Required: true},
			{Name: "taskId", Type: "string", Required: true},
			{Name: "completed", Type: "int", Required: true},
			{Name: "total", Type: "int", Required: true},
		},
		Severity: "info",
	},
	"automation:batch:summary": {
		Name: "automation:batch:summary", Namespace: "automation", Description: "批量任务汇总",
		Payload: []EventField{
			{Name: "batchId", Type: "string", Required: true},
			{Name: "total", Type: "int", Required: true},
			{Name: "success", Type: "int", Required: true},
			{Name: "failed", Type: "int", Required: true},
			{Name: "duration", Type: "int64", Required: false},
		},
		Severity: "info",
	},
	"automation:batch:failed": {
		Name: "automation:batch:failed", Namespace: "automation", Description: "批量任务整体失败",
		Payload: []EventField{
			{Name: "batchId", Type: "string", Required: true},
			{Name: "error", Type: "string", Required: true},
		},
		Severity: "error",
	},

	// ─── Proxy: Quality ──────────────────────────────────────────────────────
	"proxy:quality:latency-spike": {
		Name: "proxy:quality:latency-spike", Namespace: "proxy", Description: "代理延迟突增",
		Payload: []EventField{
			{Name: "proxyId", Type: "string", Required: true},
			{Name: "previousMs", Type: "int64", Required: false},
			{Name: "currentMs", Type: "int64", Required: true},
		},
		Severity: "warn",
	},
	"proxy:quality:health-drop": {
		Name: "proxy:quality:health-drop", Namespace: "proxy", Description: "代理健康评分下降",
		Payload: []EventField{
			{Name: "proxyId", Type: "string", Required: true},
			{Name: "previousScore", Type: "int64", Required: false},
			{Name: "currentScore", Type: "int64", Required: true},
		},
		Severity: "warn",
	},
	"proxy:quality:node-rotated": {
		Name: "proxy:quality:node-rotated", Namespace: "proxy", Description: "代理节点已轮换",
		Payload: []EventField{
			{Name: "proxyId", Type: "string", Required: true},
			{Name: "oldIP", Type: "string", Required: false},
			{Name: "newIP", Type: "string", Required: false},
		},
		Severity: "info",
	},
	"proxy:quality:pool-exhausted": {
		Name: "proxy:quality:pool-exhausted", Namespace: "proxy", Description: "代理池耗尽",
		Payload: []EventField{
			{Name: "sourceId", Type: "string", Required: false},
			{Name: "availableNodes", Type: "int", Required: true},
		},
		Severity: "critical",
	},
	"proxy:quality:best-node": {
		Name: "proxy:quality:best-node", Namespace: "proxy", Description: "最佳节点已更新",
		Payload: []EventField{
			{Name: "proxyId", Type: "string", Required: true},
			{Name: "latencyMs", Type: "int64", Required: true},
		},
		Severity: "info",
	},
	"proxy:quality:score-change": {
		Name: "proxy:quality:score-change", Namespace: "proxy", Description: "代理综合评分变化",
		Payload: []EventField{
			{Name: "proxyId", Type: "string", Required: true},
			{Name: "previousScore", Type: "float64", Required: false},
			{Name: "currentScore", Type: "float64", Required: true},
		},
		Severity: "info",
	},
	"proxy:quality:node-added": {
		Name: "proxy:quality:node-added", Namespace: "proxy", Description: "新节点已加入代理池",
		Payload: []EventField{
			{Name: "proxyId", Type: "string", Required: true},
			{Name: "ip", Type: "string", Required: false},
		},
		Severity: "info",
	},
	"proxy:quality:node-removed": {
		Name: "proxy:quality:node-removed", Namespace: "proxy", Description: "节点已从代理池移除",
		Payload: []EventField{
			{Name: "proxyId", Type: "string", Required: true},
			{Name: "reason", Type: "string", Required: false},
		},
		Severity: "warn",
	},

	// ─── Proxy: Rotation ─────────────────────────────────────────────────────
	"proxy:rotation:triggered": {
		Name: "proxy:rotation:triggered", Namespace: "proxy", Description: "代理轮换已触发",
		Payload: []EventField{
			{Name: "profileId", Type: "string", Required: false},
			{Name: "reason", Type: "string", Required: false},
		},
		Severity: "info",
	},
	"proxy:rotation:completed": {
		Name: "proxy:rotation:completed", Namespace: "proxy", Description: "代理轮换完成",
		Payload: []EventField{
			{Name: "profileId", Type: "string", Required: false},
			{Name: "newProxyId", Type: "string", Required: true},
		},
		Severity: "info",
	},
	"proxy:rotation:skipped": {
		Name: "proxy:rotation:skipped", Namespace: "proxy", Description: "代理轮换跳过（无可用节点）",
		Payload: []EventField{
			{Name: "profileId", Type: "string", Required: false},
			{Name: "reason", Type: "string", Required: true},
		},
		Severity: "warn",
	},
	"proxy:rotation:failed": {
		Name: "proxy:rotation:failed", Namespace: "proxy", Description: "代理轮换失败",
		Payload: []EventField{
			{Name: "profileId", Type: "string", Required: false},
			{Name: "error", Type: "string", Required: true},
		},
		Severity: "error",
	},

	// ─── Data: Scrape ────────────────────────────────────────────────────────
	"data:scrape:started": {
		Name: "data:scrape:started", Namespace: "data", Description: "数据采集开始",
		Payload: []EventField{
			{Name: "profileId", Type: "string", Required: true},
			{Name: "target", Type: "string", Required: false},
			{Name: "expectedPages", Type: "int", Required: false},
		},
		Severity: "info",
	},
	"data:scrape:progress": {
		Name: "data:scrape:progress", Namespace: "data", Description: "数据采集进度",
		Payload: []EventField{
			{Name: "profileId", Type: "string", Required: true},
			{Name: "pagesScraped", Type: "int", Required: true},
			{Name: "itemsFound", Type: "int", Required: true},
		},
		Severity: "info",
	},
	"data:scrape:page-done": {
		Name: "data:scrape:page-done", Namespace: "data", Description: "单页数据采集完成",
		Payload: []EventField{
			{Name: "profileId", Type: "string", Required: true},
			{Name: "pageUrl", Type: "string", Required: false},
			{Name: "itemsOnPage", Type: "int", Required: true},
		},
		Severity: "info",
	},
	"data:scrape:rate-limited": {
		Name: "data:scrape:rate-limited", Namespace: "data", Description: "数据采集触发限流",
		Payload: []EventField{
			{Name: "profileId", Type: "string", Required: true},
			{Name: "retryAfter", Type: "string", Required: false},
		},
		Severity: "warn",
	},
	"data:scrape:completed": {
		Name: "data:scrape:completed", Namespace: "data", Description: "数据采集完成",
		Payload: []EventField{
			{Name: "profileId", Type: "string", Required: true},
			{Name: "totalItems", Type: "int", Required: true},
			{Name: "totalPages", Type: "int", Required: false},
			{Name: "duration", Type: "int64", Required: false},
		},
		Severity: "info",
	},
	"data:scrape:exported": {
		Name: "data:scrape:exported", Namespace: "data", Description: "采集数据已导出",
		Payload: []EventField{
			{Name: "profileId", Type: "string", Required: true},
			{Name: "format", Type: "string", Required: true},
			{Name: "filePath", Type: "string", Required: false},
			{Name: "itemCount", Type: "int", Required: true},
		},
		Severity: "info",
	},

	// ─── Browser Instance Start Phases (14) ──────────────────────────────────
	"browser:instance:start:port-allocating": {
		Name: "browser:instance:start:port-allocating", Namespace: "browser", Description: "正在分配调试端口",
		Payload:  []EventField{{Name: "profileId", Type: "string", Required: true}},
		Severity: "info",
	},
	"browser:instance:start:port-allocated": {
		Name: "browser:instance:start:port-allocated", Namespace: "browser", Description: "调试端口分配完成",
		Payload:  []EventField{{Name: "profileId", Type: "string", Required: true}, {Name: "debugPort", Type: "int", Required: true}},
		Severity: "info",
	},
	"browser:instance:start:checking-running": {
		Name: "browser:instance:start:checking-running", Namespace: "browser", Description: "检查是否已有实例在运行",
		Payload:  []EventField{{Name: "profileId", Type: "string", Required: true}},
		Severity: "info",
	},
	"browser:instance:start:core-validating": {
		Name: "browser:instance:start:core-validating", Namespace: "browser", Description: "正在校验浏览器内核",
		Payload:  []EventField{{Name: "profileId", Type: "string", Required: true}, {Name: "coreId", Type: "string", Required: false}},
		Severity: "info",
	},
	"browser:instance:start:core-ready": {
		Name: "browser:instance:start:core-ready", Namespace: "browser", Description: "浏览器内核校验完成",
		Payload:  []EventField{{Name: "profileId", Type: "string", Required: true}, {Name: "corePath", Type: "string", Required: true}},
		Severity: "info",
	},
	"browser:instance:start:proxy-starting": {
		Name: "browser:instance:start:proxy-starting", Namespace: "browser", Description: "正在启动代理桥接",
		Payload:  []EventField{{Name: "profileId", Type: "string", Required: true}, {Name: "proxyId", Type: "string", Required: false}},
		Severity: "info",
	},
	"browser:instance:start:proxy-ready": {
		Name: "browser:instance:start:proxy-ready", Namespace: "browser", Description: "代理桥接已就绪",
		Payload:  []EventField{{Name: "profileId", Type: "string", Required: true}, {Name: "proxyPort", Type: "int", Required: false}},
		Severity: "info",
	},
	"browser:instance:start:launcher-spawning": {
		Name: "browser:instance:start:launcher-spawning", Namespace: "browser", Description: "正在启动浏览器进程",
		Payload:  []EventField{{Name: "profileId", Type: "string", Required: true}},
		Severity: "info",
	},
	"browser:instance:start:launcher-spawned": {
		Name: "browser:instance:start:launcher-spawned", Namespace: "browser", Description: "浏览器进程已创建",
		Payload:  []EventField{{Name: "profileId", Type: "string", Required: true}, {Name: "pid", Type: "int", Required: true}},
		Severity: "info",
	},
	"browser:instance:start:window-opening": {
		Name: "browser:instance:start:window-opening", Namespace: "browser", Description: "正在打开浏览器窗口",
		Payload:  []EventField{{Name: "profileId", Type: "string", Required: true}},
		Severity: "info",
	},
	"browser:instance:start:window-opened": {
		Name: "browser:instance:start:window-opened", Namespace: "browser", Description: "浏览器窗口已打开",
		Payload:  []EventField{{Name: "profileId", Type: "string", Required: true}},
		Severity: "info",
	},
	"browser:instance:start:cdp-attaching": {
		Name: "browser:instance:start:cdp-attaching", Namespace: "browser", Description: "正在连接 CDP 调试协议",
		Payload:  []EventField{{Name: "profileId", Type: "string", Required: true}, {Name: "debugPort", Type: "int", Required: false}},
		Severity: "info",
	},
	"browser:instance:start:cdp-attached": {
		Name: "browser:instance:start:cdp-attached", Namespace: "browser", Description: "CDP 调试协议已连接",
		Payload:  []EventField{{Name: "profileId", Type: "string", Required: true}, {Name: "cdpUrl", Type: "string", Required: false}},
		Severity: "info",
	},
	"browser:instance:start:completed": {
		Name: "browser:instance:start:completed", Namespace: "browser", Description: "实例启动流程全部完成",
		Payload: []EventField{
			{Name: "profileId", Type: "string", Required: true},
			{Name: "durationMs", Type: "int64", Required: false},
			{Name: "cdpUrl", Type: "string", Required: false},
		},
		Severity: "info",
	},

	// ─── Browser Instance Stop Phases (6) ────────────────────────────────────
	"browser:instance:stop:dispatching": {
		Name: "browser:instance:stop:dispatching", Namespace: "browser", Description: "正在发送停止指令",
		Payload:  []EventField{{Name: "profileId", Type: "string", Required: true}},
		Severity: "info",
	},
	"browser:instance:stop:cdp-detaching": {
		Name: "browser:instance:stop:cdp-detaching", Namespace: "browser", Description: "正在断开 CDP 连接",
		Payload:  []EventField{{Name: "profileId", Type: "string", Required: true}},
		Severity: "info",
	},
	"browser:instance:stop:proxy-stopping": {
		Name: "browser:instance:stop:proxy-stopping", Namespace: "browser", Description: "正在停止代理桥接",
		Payload:  []EventField{{Name: "profileId", Type: "string", Required: true}},
		Severity: "info",
	},
	"browser:instance:stop:process-killing": {
		Name: "browser:instance:stop:process-killing", Namespace: "browser", Description: "正在终止浏览器进程",
		Payload:  []EventField{{Name: "profileId", Type: "string", Required: true}, {Name: "pid", Type: "int", Required: false}},
		Severity: "info",
	},
	"browser:instance:stop:cleaning": {
		Name: "browser:instance:stop:cleaning", Namespace: "browser", Description: "正在清理临时文件",
		Payload:  []EventField{{Name: "profileId", Type: "string", Required: true}},
		Severity: "info",
	},
	"browser:instance:stop:completed": {
		Name: "browser:instance:stop:completed", Namespace: "browser", Description: "实例停止流程全部完成",
		Payload:  []EventField{{Name: "profileId", Type: "string", Required: true}},
		Severity: "info",
	},

	// ─── Proxy Bridge Lifecycle (8) ──────────────────────────────────────────
	"proxy:bridge:xray:starting": {
		Name: "proxy:bridge:xray:starting", Namespace: "proxy", Description: "Xray 桥接正在启动",
		Payload:  []EventField{{Name: "profileId", Type: "string", Required: true}},
		Severity: "info",
	},
	"proxy:bridge:xray:started": {
		Name: "proxy:bridge:xray:started", Namespace: "proxy", Description: "Xray 桥接已启动",
		Payload:  []EventField{{Name: "profileId", Type: "string", Required: true}, {Name: "port", Type: "int", Required: true}},
		Severity: "info",
	},
	"proxy:bridge:clash:starting": {
		Name: "proxy:bridge:clash:starting", Namespace: "proxy", Description: "Clash 桥接正在启动",
		Payload:  []EventField{{Name: "profileId", Type: "string", Required: true}},
		Severity: "info",
	},
	"proxy:bridge:clash:started": {
		Name: "proxy:bridge:clash:started", Namespace: "proxy", Description: "Clash 桥接已启动",
		Payload:  []EventField{{Name: "profileId", Type: "string", Required: true}, {Name: "port", Type: "int", Required: true}},
		Severity: "info",
	},
	"proxy:bridge:singbox:starting": {
		Name: "proxy:bridge:singbox:starting", Namespace: "proxy", Description: "SingBox 桥接正在启动",
		Payload:  []EventField{{Name: "profileId", Type: "string", Required: true}},
		Severity: "info",
	},
	"proxy:bridge:singbox:started": {
		Name: "proxy:bridge:singbox:started", Namespace: "proxy", Description: "SingBox 桥接已启动",
		Payload:  []EventField{{Name: "profileId", Type: "string", Required: true}, {Name: "port", Type: "int", Required: true}},
		Severity: "info",
	},
	"proxy:bridge:reconnecting": {
		Name: "proxy:bridge:reconnecting", Namespace: "proxy", Description: "代理桥接正在重连",
		Payload: []EventField{
			{Name: "profileId", Type: "string", Required: true},
			{Name: "attempt", Type: "int", Required: false},
			{Name: "bridgeType", Type: "string", Required: false},
		},
		Severity: "warn",
	},
	"proxy:bridge:reconnected": {
		Name: "proxy:bridge:reconnected", Namespace: "proxy", Description: "代理桥接重连成功",
		Payload:  []EventField{{Name: "profileId", Type: "string", Required: true}, {Name: "attempts", Type: "int", Required: false}},
		Severity: "info",
	},

	// ─── Backup Lifecycle (8) ────────────────────────────────────────────────
	"backup:export:started": {
		Name: "backup:export:started", Namespace: "backup", Description: "备份导出已开始",
		Payload:  []EventField{{Name: "scope", Type: "string", Required: false}},
		Severity: "info",
	},
	"backup:export:collecting": {
		Name: "backup:export:collecting", Namespace: "backup", Description: "正在收集备份数据",
		Payload:  []EventField{{Name: "files", Type: "int", Required: false}},
		Severity: "info",
	},
	"backup:export:compressing": {
		Name: "backup:export:compressing", Namespace: "backup", Description: "正在压缩备份包",
		Payload:  []EventField{{Name: "sizeBytes", Type: "int64", Required: false}},
		Severity: "info",
	},
	"backup:export:completed": {
		Name: "backup:export:completed", Namespace: "backup", Description: "备份导出已完成",
		Payload:  []EventField{{Name: "path", Type: "string", Required: false}, {Name: "sizeBytes", Type: "int64", Required: false}},
		Severity: "info",
	},
	"backup:import:started": {
		Name: "backup:import:started", Namespace: "backup", Description: "备份导入已开始",
		Payload:  []EventField{{Name: "path", Type: "string", Required: false}},
		Severity: "info",
	},
	"backup:import:validating": {
		Name: "backup:import:validating", Namespace: "backup", Description: "正在验证备份包完整性",
		Payload:  []EventField{},
		Severity: "info",
	},
	"backup:import:restoring": {
		Name: "backup:import:restoring", Namespace: "backup", Description: "正在还原数据",
		Payload:  []EventField{{Name: "entries", Type: "int", Required: false}},
		Severity: "info",
	},
	"backup:import:completed": {
		Name: "backup:import:completed", Namespace: "backup", Description: "备份导入已完成",
		Payload:  []EventField{{Name: "entries", Type: "int", Required: false}},
		Severity: "info",
	},

	// ─── Core Download Phases (6) ────────────────────────────────────────────
	"core:download:started": {
		Name: "core:download:started", Namespace: "core", Description: "内核下载已开始",
		Payload:  []EventField{{Name: "version", Type: "string", Required: false}, {Name: "url", Type: "string", Required: false}},
		Severity: "info",
	},
	"core:download:progress": {
		Name: "core:download:progress", Namespace: "core", Description: "内核下载进度更新",
		Payload:  []EventField{{Name: "percent", Type: "float64", Required: true}, {Name: "downloaded", Type: "int64", Required: false}, {Name: "total", Type: "int64", Required: false}},
		Severity: "info",
	},
	"core:download:extracting": {
		Name: "core:download:extracting", Namespace: "core", Description: "正在解压内核文件",
		Payload:  []EventField{{Name: "path", Type: "string", Required: false}},
		Severity: "info",
	},
	"core:download:verifying": {
		Name: "core:download:verifying", Namespace: "core", Description: "正在校验内核文件",
		Payload:  []EventField{},
		Severity: "info",
	},
	"core:download:completed": {
		Name: "core:download:completed", Namespace: "core", Description: "内核下载已完成",
		Payload:  []EventField{{Name: "version", Type: "string", Required: false}, {Name: "path", Type: "string", Required: true}},
		Severity: "info",
	},
	"core:download:failed": {
		Name: "core:download:failed", Namespace: "core", Description: "内核下载失败",
		Payload:  []EventField{{Name: "error", Type: "string", Required: true}, {Name: "version", Type: "string", Required: false}},
		Severity: "error",
	},

	// ─── Settings Change Events (12) ─────────────────────────────────────────
	"settings:change:app-config": {
		Name: "settings:change:app-config", Namespace: "settings", Description: "应用配置已变更",
		Payload:  []EventField{{Name: "field", Type: "string", Required: false}},
		Severity: "info",
	},
	"settings:change:browser-proxy": {
		Name: "settings:change:browser-proxy", Namespace: "settings", Description: "浏览器代理配置已变更",
		Payload:  []EventField{{Name: "proxyCount", Type: "int", Required: false}},
		Severity: "info",
	},
	"settings:change:browser-core": {
		Name: "settings:change:browser-core", Namespace: "settings", Description: "浏览器内核配置已变更",
		Payload:  []EventField{{Name: "coreId", Type: "string", Required: false}},
		Severity: "info",
	},
	"settings:change:launch-defaults": {
		Name: "settings:change:launch-defaults", Namespace: "settings", Description: "默认启动参数已变更",
		Payload:  []EventField{},
		Severity: "info",
	},
	"settings:change:interceptor": {
		Name: "settings:change:interceptor", Namespace: "settings", Description: "请求拦截器配置已变更",
		Payload:  []EventField{{Name: "enabled", Type: "bool", Required: false}},
		Severity: "info",
	},
	"settings:change:launch-server": {
		Name: "settings:change:launch-server", Namespace: "settings", Description: "Launch Server 配置已变更",
		Payload:  []EventField{{Name: "port", Type: "int", Required: false}},
		Severity: "info",
	},
	"settings:change:log-level": {
		Name: "settings:change:log-level", Namespace: "settings", Description: "日志等级已变更",
		Payload:  []EventField{{Name: "from", Type: "string", Required: false}, {Name: "to", Type: "string", Required: true}},
		Severity: "info",
	},
	"settings:change:backup-scope": {
		Name: "settings:change:backup-scope", Namespace: "settings", Description: "备份范围配置已变更",
		Payload:  []EventField{},
		Severity: "info",
	},
	"settings:change:profile-defaults": {
		Name: "settings:change:profile-defaults", Namespace: "settings", Description: "默认实例配置已变更",
		Payload:  []EventField{},
		Severity: "info",
	},
	"settings:change:group-sort": {
		Name: "settings:change:group-sort", Namespace: "settings", Description: "分组排序已变更",
		Payload:  []EventField{},
		Severity: "info",
	},
	"settings:change:bookmark": {
		Name: "settings:change:bookmark", Namespace: "settings", Description: "书签配置已变更",
		Payload:  []EventField{{Name: "count", Type: "int", Required: false}},
		Severity: "info",
	},
	"settings:change:runtime-config": {
		Name: "settings:change:runtime-config", Namespace: "settings", Description: "运行时配置已变更",
		Payload:  []EventField{},
		Severity: "info",
	},

	// ─── License Events (6) ──────────────────────────────────────────────────
	"license:activated": {
		Name: "license:activated", Namespace: "license", Description: "许可证已激活",
		Payload:  []EventField{{Name: "maxLimit", Type: "int", Required: false}},
		Severity: "info",
	},
	"license:expiring": {
		Name: "license:expiring", Namespace: "license", Description: "许可证即将过期",
		Payload:  []EventField{{Name: "daysLeft", Type: "int", Required: true}},
		Severity: "warn",
	},
	"license:expired": {
		Name: "license:expired", Namespace: "license", Description: "许可证已过期",
		Payload:  []EventField{},
		Severity: "error",
	},
	"license:renewed": {
		Name: "license:renewed", Namespace: "license", Description: "许可证已续期",
		Payload:  []EventField{{Name: "newExpiry", Type: "string", Required: false}},
		Severity: "info",
	},
	"license:cdkey-redeemed": {
		Name: "license:cdkey-redeemed", Namespace: "license", Description: "CDKey 已兑换",
		Payload:  []EventField{{Name: "keyPrefix", Type: "string", Required: false}},
		Severity: "info",
	},
	"license:cdkey-invalid": {
		Name: "license:cdkey-invalid", Namespace: "license", Description: "CDKey 无效",
		Payload:  []EventField{{Name: "reason", Type: "string", Required: false}},
		Severity: "warn",
	},

	// ─── System Health (16) ──────────────────────────────────────────────────
	"system:health:memory-high": {
		Name: "system:health:memory-high", Namespace: "system", Description: "系统内存占用过高",
		Payload:  []EventField{{Name: "usedMB", Type: "int64", Required: true}, {Name: "totalMB", Type: "int64", Required: true}},
		Severity: "warn",
	},
	"system:health:memory-critical": {
		Name: "system:health:memory-critical", Namespace: "system", Description: "系统内存严重不足",
		Payload:  []EventField{{Name: "usedMB", Type: "int64", Required: true}, {Name: "availableMB", Type: "int64", Required: true}},
		Severity: "critical",
	},
	"system:health:memory-normal": {
		Name: "system:health:memory-normal", Namespace: "system", Description: "系统内存恢复正常",
		Payload:  []EventField{{Name: "usedMB", Type: "int64", Required: false}},
		Severity: "info",
	},
	"system:health:disk-low": {
		Name: "system:health:disk-low", Namespace: "system", Description: "磁盘空间不足",
		Payload:  []EventField{{Name: "freeMB", Type: "int64", Required: true}, {Name: "path", Type: "string", Required: false}},
		Severity: "warn",
	},
	"system:health:disk-critical": {
		Name: "system:health:disk-critical", Namespace: "system", Description: "磁盘空间严重不足",
		Payload:  []EventField{{Name: "freeMB", Type: "int64", Required: true}},
		Severity: "critical",
	},
	"system:health:cpu-high": {
		Name: "system:health:cpu-high", Namespace: "system", Description: "CPU 使用率过高",
		Payload:  []EventField{{Name: "percent", Type: "float64", Required: true}},
		Severity: "warn",
	},
	"system:health:cpu-normal": {
		Name: "system:health:cpu-normal", Namespace: "system", Description: "CPU 使用率恢复正常",
		Payload:  []EventField{},
		Severity: "info",
	},
	"system:health:goroutine-count": {
		Name: "system:health:goroutine-count", Namespace: "system", Description: "协程数量告警",
		Payload:  []EventField{{Name: "count", Type: "int", Required: true}},
		Severity: "warn",
	},
	"system:health:gc-triggered": {
		Name: "system:health:gc-triggered", Namespace: "system", Description: "手动触发 GC",
		Payload:  []EventField{{Name: "beforeMB", Type: "int64", Required: false}, {Name: "afterMB", Type: "int64", Required: false}},
		Severity: "info",
	},
	"system:health:db-size": {
		Name: "system:health:db-size", Namespace: "system", Description: "数据库大小告警",
		Payload:  []EventField{{Name: "sizeMB", Type: "float64", Required: true}},
		Severity: "warn",
	},
	"system:health:db-vacuum": {
		Name: "system:health:db-vacuum", Namespace: "system", Description: "数据库已执行 VACUUM",
		Payload:  []EventField{{Name: "beforeMB", Type: "float64", Required: false}, {Name: "afterMB", Type: "float64", Required: false}},
		Severity: "info",
	},
	"system:health:process-count": {
		Name: "system:health:process-count", Namespace: "system", Description: "浏览器进程数量告警",
		Payload:  []EventField{{Name: "count", Type: "int", Required: true}},
		Severity: "info",
	},
	"system:health:config-reload": {
		Name: "system:health:config-reload", Namespace: "system", Description: "配置已重载",
		Payload:  []EventField{},
		Severity: "info",
	},
	"system:health:startup-complete": {
		Name: "system:health:startup-complete", Namespace: "system", Description: "系统启动完成",
		Payload:  []EventField{{Name: "durationMs", Type: "int64", Required: false}},
		Severity: "info",
	},
	"system:health:shutdown-initiated": {
		Name: "system:health:shutdown-initiated", Namespace: "system", Description: "系统正在关闭",
		Payload:  []EventField{{Name: "reason", Type: "string", Required: false}},
		Severity: "info",
	},
	"system:health:panic-recovered": {
		Name: "system:health:panic-recovered", Namespace: "system", Description: "已从 panic 中恢复",
		Payload:  []EventField{{Name: "stack", Type: "string", Required: false}},
		Severity: "critical",
	},

	// ─── System Recovery (8) ─────────────────────────────────────────────────
	"system:recovery:crash-loop-detected": {
		Name: "system:recovery:crash-loop-detected", Namespace: "system", Description: "检测到崩溃循环",
		Payload:  []EventField{{Name: "profileId", Type: "string", Required: true}, {Name: "count", Type: "int", Required: true}},
		Severity: "error",
	},
	"system:recovery:auto-restart": {
		Name: "system:recovery:auto-restart", Namespace: "system", Description: "自动重启实例",
		Payload:  []EventField{{Name: "profileId", Type: "string", Required: true}},
		Severity: "info",
	},
	"system:recovery:auto-stop": {
		Name: "system:recovery:auto-stop", Namespace: "system", Description: "自动停止异常实例",
		Payload:  []EventField{{Name: "profileId", Type: "string", Required: true}, {Name: "reason", Type: "string", Required: true}},
		Severity: "warn",
	},
	"system:recovery:orphan-cleanup": {
		Name: "system:recovery:orphan-cleanup", Namespace: "system", Description: "清理孤儿进程",
		Payload:  []EventField{{Name: "count", Type: "int", Required: false}},
		Severity: "info",
	},
	"system:recovery:port-released": {
		Name: "system:recovery:port-released", Namespace: "system", Description: "已释放占用的端口",
		Payload:  []EventField{{Name: "port", Type: "int", Required: true}},
		Severity: "info",
	},
	"system:recovery:lockfile-cleaned": {
		Name: "system:recovery:lockfile-cleaned", Namespace: "system", Description: "已清理残留锁文件",
		Payload:  []EventField{{Name: "path", Type: "string", Required: true}},
		Severity: "info",
	},
	"system:recovery:profile-repaired": {
		Name: "system:recovery:profile-repaired", Namespace: "system", Description: "实例配置已修复",
		Payload:  []EventField{{Name: "profileId", Type: "string", Required: true}},
		Severity: "info",
	},
	"system:recovery:db-recovered": {
		Name: "system:recovery:db-recovered", Namespace: "system", Description: "数据库已从备份恢复",
		Payload:  []EventField{},
		Severity: "warn",
	},

	// ─── Automation Script Execution (12) ────────────────────────────────────
	"automation:script:started": {
		Name: "automation:script:started", Namespace: "automation", Description: "自动化脚本已开始执行",
		Payload:  []EventField{{Name: "taskId", Type: "string", Required: true}, {Name: "taskName", Type: "string", Required: false}},
		Severity: "info",
	},
	"automation:script:action-starting": {
		Name: "automation:script:action-starting", Namespace: "automation", Description: "脚本动作开始执行",
		Payload:  []EventField{{Name: "taskId", Type: "string", Required: true}, {Name: "actionType", Type: "string", Required: true}, {Name: "target", Type: "string", Required: false}},
		Severity: "info",
	},
	"automation:script:action-completed": {
		Name: "automation:script:action-completed", Namespace: "automation", Description: "脚本动作执行完成",
		Payload:  []EventField{{Name: "taskId", Type: "string", Required: true}, {Name: "actionType", Type: "string", Required: true}, {Name: "durationMs", Type: "int64", Required: false}},
		Severity: "info",
	},
	"automation:script:action-failed": {
		Name: "automation:script:action-failed", Namespace: "automation", Description: "脚本动作执行失败",
		Payload:  []EventField{{Name: "taskId", Type: "string", Required: true}, {Name: "actionType", Type: "string", Required: true}, {Name: "error", Type: "string", Required: true}},
		Severity: "error",
	},
	"automation:script:retrying": {
		Name: "automation:script:retrying", Namespace: "automation", Description: "脚本正在重试",
		Payload:  []EventField{{Name: "taskId", Type: "string", Required: true}, {Name: "attempt", Type: "int", Required: true}, {Name: "maxRetries", Type: "int", Required: false}},
		Severity: "warn",
	},
	"automation:script:completed": {
		Name: "automation:script:completed", Namespace: "automation", Description: "自动化脚本执行完成",
		Payload:  []EventField{{Name: "taskId", Type: "string", Required: true}, {Name: "durationMs", Type: "int64", Required: false}},
		Severity: "info",
	},
	"automation:script:failed": {
		Name: "automation:script:failed", Namespace: "automation", Description: "自动化脚本执行失败",
		Payload:  []EventField{{Name: "taskId", Type: "string", Required: true}, {Name: "error", Type: "string", Required: true}},
		Severity: "error",
	},
	"automation:script:cancelled": {
		Name: "automation:script:cancelled", Namespace: "automation", Description: "自动化脚本已取消",
		Payload:  []EventField{{Name: "taskId", Type: "string", Required: true}},
		Severity: "warn",
	},
	"automation:script:paused": {
		Name: "automation:script:paused", Namespace: "automation", Description: "自动化脚本已暂停",
		Payload:  []EventField{{Name: "taskId", Type: "string", Required: true}, {Name: "reason", Type: "string", Required: false}},
		Severity: "info",
	},
	"automation:script:resumed": {
		Name: "automation:script:resumed", Namespace: "automation", Description: "自动化脚本已恢复",
		Payload:  []EventField{{Name: "taskId", Type: "string", Required: true}},
		Severity: "info",
	},
	"automation:rule:triggered": {
		Name: "automation:rule:triggered", Namespace: "automation", Description: "自动响应规则已触发",
		Payload:  []EventField{{Name: "ruleId", Type: "string", Required: true}, {Name: "ruleName", Type: "string", Required: false}, {Name: "triggerEvent", Type: "string", Required: true}},
		Severity: "info",
	},
	"automation:rule:cooldown-skipped": {
		Name: "automation:rule:cooldown-skipped", Namespace: "automation", Description: "规则被冷却跳过",
		Payload:  []EventField{{Name: "ruleId", Type: "string", Required: true}, {Name: "ruleName", Type: "string", Required: false}},
		Severity: "info",
	},

	// ─── Data Export Phases (8) ──────────────────────────────────────────────
	"data:export:cookies-started": {
		Name: "data:export:cookies-started", Namespace: "data", Description: "Cookie 导出已开始",
		Payload:  []EventField{{Name: "profileId", Type: "string", Required: true}},
		Severity: "info",
	},
	"data:export:cookies-completed": {
		Name: "data:export:cookies-completed", Namespace: "data", Description: "Cookie 导出已完成",
		Payload:  []EventField{{Name: "profileId", Type: "string", Required: true}, {Name: "count", Type: "int", Required: false}},
		Severity: "info",
	},
	"data:export:profiles-started": {
		Name: "data:export:profiles-started", Namespace: "data", Description: "实例数据导出已开始",
		Payload:  []EventField{{Name: "count", Type: "int", Required: false}},
		Severity: "info",
	},
	"data:export:profiles-completed": {
		Name: "data:export:profiles-completed", Namespace: "data", Description: "实例数据导出已完成",
		Payload:  []EventField{{Name: "count", Type: "int", Required: false}, {Name: "path", Type: "string", Required: false}},
		Severity: "info",
	},
	"data:export:proxies-started": {
		Name: "data:export:proxies-started", Namespace: "data", Description: "代理数据导出已开始",
		Payload:  []EventField{{Name: "count", Type: "int", Required: false}},
		Severity: "info",
	},
	"data:export:proxies-completed": {
		Name: "data:export:proxies-completed", Namespace: "data", Description: "代理数据导出已完成",
		Payload:  []EventField{{Name: "count", Type: "int", Required: false}, {Name: "path", Type: "string", Required: false}},
		Severity: "info",
	},
	"data:export:logs-started": {
		Name: "data:export:logs-started", Namespace: "data", Description: "事件日志导出已开始",
		Payload:  []EventField{},
		Severity: "info",
	},
	"data:export:logs-completed": {
		Name: "data:export:logs-completed", Namespace: "data", Description: "事件日志导出已完成",
		Payload:  []EventField{{Name: "count", Type: "int", Required: false}},
		Severity: "info",
	},
}

// AllRegistryNames returns every event name in the registry.
func AllRegistryNames() []string {
	names := make([]string, 0, len(Registry))
	for name := range Registry {
		names = append(names, name)
	}
	return names
}

// RegistryNamesByNamespace returns event names grouped by namespace.
func RegistryNamesByNamespace() map[string][]string {
	groups := make(map[string][]string)
	for name, def := range Registry {
		groups[def.Namespace] = append(groups[def.Namespace], name)
	}
	return groups
}
