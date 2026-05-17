package backend

import (
	"context"
	"encoding/json"
	"fmt"
	"math/rand"
	"os"
	"strings"
	"time"

	"personal-pilot/backend/internal/behavior"
	"personal-pilot/backend/internal/behavior/humanize"
	"personal-pilot/backend/internal/browser"
	"personal-pilot/backend/internal/email"
	"personal-pilot/backend/internal/logger"
	"personal-pilot/backend/internal/proxy"
	"personal-pilot/backend/internal/webhook"
)

// ─── Types ──────────────────────────────────────────────────────────────────────

// DeepSeekRegisterInput describes a registration request (Wails-bound).
type DeepSeekRegisterInput struct {
	ProfileID string `json:"profileId"`
	Password  string `json:"password"`
}

// DeepSeekRegisterResult is returned to the frontend after registration.
type DeepSeekRegisterResult struct {
	Success  bool   `json:"success"`
	Email    string `json:"email"`
	Password string `json:"password"`
	APIKey   string `json:"apiKey"`
	Error    string `json:"error"`
}

// DeepSeekRegisterState is the live progress state pushed to the frontend.
type DeepSeekRegisterState struct {
	Phase   string `json:"phase"`
	Message string `json:"message"`
}

// ─── Pipeline types ─────────────────────────────────────────────────────────────

// FailureType categorizes registration failures.
type FailureType string

const (
	FailureNone        FailureType = ""
	FailureProxy       FailureType = "proxy"
	FailureBrowser     FailureType = "browser"
	FailureCDP         FailureType = "cdp"
	FailureWAF         FailureType = "waf"
	FailureTurnstile   FailureType = "turnstile"
	FailureEmail       FailureType = "email"
	FailureCodeTimeout FailureType = "code_timeout"
	FailureAPI         FailureType = "api"
	FailureAPIKey      FailureType = "api_key"
)

// bizCodeAction maps DeepSeek API biz_code values to recovery strategies.
type bizCodeAction int

const (
	bizRetrySame    bizCodeAction = iota // retry with same config
	bizSwitchEmail                       // generate new email
	bizSwitchProxy                       // switch proxy and retry
	bizAbandon                           // unrecoverable, skip round
)

// bizCodeMap maps known DeepSeek business error codes to actions.
var bizCodeMap = map[int]bizCodeAction{
	0:    bizRetrySame,   // success
	200:  bizRetrySame,   // success
	1001: bizSwitchEmail, // email already registered
	1002: bizSwitchEmail, // email format invalid
	1003: bizRetrySame,   // code not yet sent (retry)
	1004: bizRetrySame,   // code expired (resend)
	1005: bizSwitchEmail, // code verify failed (wrong code)
	2001: bizSwitchProxy, // IP blocked / rate limited
	2002: bizSwitchProxy, // region not supported
	2003: bizAbandon,     // account suspended
	3001: bizAbandon,     // internal server error (retry once then abandon)
	3002: bizRetrySame,   // service busy (retry)
	4001: bizSwitchProxy, // suspected automation (proxy quality)
	4002: bizSwitchProxy, // turnstile verification failed
}

func classifyBizCode(code int) bizCodeAction {
	if action, ok := bizCodeMap[code]; ok {
		return action
	}
	if code >= 5000 {
		return bizAbandon
	}
	return bizRetrySame
}

// ProxyInfo records proxy details used for a round.
type ProxyInfo struct {
	ProxyID       string  `json:"proxyId"`
	ProxyName     string  `json:"proxyName"`
	Country       string  `json:"country"`
	IsResidential bool    `json:"isResidential"`
	FraudScore    float64 `json:"fraudScore"`
}

// RoundResult captures the outcome of a single registration round.
type RoundResult struct {
	Round     int                `json:"round"`
	Success   bool               `json:"success"`
	Email     string             `json:"email"`
	Password  string             `json:"password"`
	APIKey    string             `json:"apiKey"`
	Error     string             `json:"error"`
	ErrorType FailureType        `json:"errorType"`
	Proxy     ProxyInfo          `json:"proxy"`
	ProfileID string             `json:"profileId"`
	Duration  string             `json:"duration"`
	Tier      proxy.PriorityTier `json:"tier"`
}

// BatchConfig controls multi-round registration behavior.
type BatchConfig struct {
	TotalRounds      int
	HumanizeLevel    humanize.HumanizationLevel
	RoundDelayMin    time.Duration
	RoundDelayMax    time.Duration
	TurnstileTimeout time.Duration
	CodePollTimeout  time.Duration
	DryRun           bool
}

// DefaultBatchConfig returns the recommended 5-round configuration.
func DefaultBatchConfig() BatchConfig {
	return BatchConfig{
		TotalRounds:      5,
		HumanizeLevel:    humanize.LevelHigh,
		RoundDelayMin:    30 * time.Second,
		RoundDelayMax:    120 * time.Second,
		TurnstileTimeout: 90 * time.Second,
		CodePollTimeout:  3 * time.Minute,
	}
}

// AdaptationState tracks adaptive behavior across rounds.
type AdaptationState struct {
	SuccessfulProxies []ProxyInfo
	FailedProxies     map[string]bool
	TurnstileTimeouts int
	WAFBlocks         int
	EmailTimeouts     int
	ConsecutiveFails  int
	BestTier          proxy.PriorityTier
}

func (a *AdaptationState) Update(result RoundResult) {
	if a.FailedProxies == nil {
		a.FailedProxies = make(map[string]bool)
	}
	if result.Success {
		a.SuccessfulProxies = append(a.SuccessfulProxies, result.Proxy)
		a.ConsecutiveFails = 0
		if result.Tier < a.BestTier || a.BestTier == 0 {
			a.BestTier = result.Tier
		}
	} else {
		a.ConsecutiveFails++
		switch result.ErrorType {
		case FailureProxy:
			a.FailedProxies[result.Proxy.ProxyID] = true
		case FailureTurnstile:
			a.TurnstileTimeouts++
		case FailureWAF:
			a.WAFBlocks++
		case FailureCodeTimeout:
			a.EmailTimeouts++
		}
	}
}

func (a *AdaptationState) isBlacklisted(proxyID string) bool {
	return a.FailedProxies[proxyID]
}

// ─── Constants ──────────────────────────────────────────────────────────────────

const (
	deepseekHomeURL    = "https://platform.deepseek.com/"
	deepseekSignupURL  = "https://platform.deepseek.com/signup"
	deepseekAPIKeysURL = "https://platform.deepseek.com/api_keys"
	deepseekAuthAPI    = "https://platform.deepseek.com/auth-api"
	turnstileSiteKey   = "0x4AAAAAAA1jPG9yoQG1HRmA"
)

// ─── Dependencies ───────────────────────────────────────────────────────────────

// RegisterDeps holds all dependencies needed by the registration pipeline.
type RegisterDeps struct {
	BrowserMgr       *browser.Manager
	ProxyDAO         browser.ProxyDAO
	Log              *logger.Logger
	OnProgress       func(phase, msg string)
	StartBrowser     func(profileID string) (*BrowserProfile, error)
	StopBrowser      func(profileID string) (*BrowserProfile, error)
	TempEmailAPIBase string
}

func (d *RegisterDeps) tempEmailAPIBase() string {
	if d.TempEmailAPIBase != "" {
		return d.TempEmailAPIBase
	}
	return email.DefaultTempEmailAPIBase
}

// ─── Wails-bound entry ──────────────────────────────────────────────────────────

// DeepSeekRegister runs the full DeepSeek registration flow against a running browser profile.
func (a *App) DeepSeekRegister(input DeepSeekRegisterInput) DeepSeekRegisterResult {
	log := logger.New("DeepSeekRegister")
	onProgress := func(phase, msg string) {
		log.Info(msg, logger.F("phase", phase))
		if a.ctx != nil {
			a.emit("deepseek:state", DeepSeekRegisterState{Phase: phase, Message: msg})
		}
	}

	profileID := strings.TrimSpace(input.ProfileID)
	if profileID == "" {
		return DeepSeekRegisterResult{Error: "profileId is required"}
	}

	password := strings.TrimSpace(input.Password)
	if password == "" {
		password = email.GenerateHumanPassword()
	}

	deps := RegisterDeps{
		BrowserMgr: a.browserMgr,
		Log:        log,
		OnProgress: onProgress,
		StartBrowser: func(pid string) (*BrowserProfile, error) {
			return a.BrowserInstanceStart(pid)
		},
		StopBrowser: func(pid string) (*BrowserProfile, error) {
			return a.BrowserInstanceStop(pid)
		},
		TempEmailAPIBase: os.Getenv("TEMP_EMAIL_API_BASE"),
	}

	result := runSingleRegistration(context.Background(), deps, profileID, password, 90*time.Second)
	return DeepSeekRegisterResult{
		Success:  result.Success,
		Email:    result.Email,
		Password: result.Password,
		APIKey:   result.APIKey,
		Error:    result.Error,
	}
}

// ─── Pipeline: single registration ──────────────────────────────────────────────

func runSingleRegistration(ctx context.Context, deps RegisterDeps, profileID, password string, turnstileTimeout time.Duration) RoundResult {
	start := time.Now()
	state := deps.OnProgress
	if state == nil {
		state = func(string, string) {}
	}

	// Phase 1: Create temp email via Cloudflare Worker (catch-all routes all domain email to Worker)
	state("email", "Creating temp email via Cloudflare Worker...")
	mailClient, err := email.NewCloudflareTempClient(deps.tempEmailAPIBase())
	if err != nil {
		return RoundResult{Error: fmt.Sprintf("temp email: %v", err), ErrorType: FailureEmail}
	}
	emailAddr := mailClient.Address()
	state("email", "Temp email created: "+emailAddr)

	// Phase 2: Connect to browser CDP
	state("cdp", "Connecting to browser CDP...")
	debugPort, err := resolveProfileCDPPort(deps.BrowserMgr, profileID)
	if err != nil {
		return RoundResult{Error: fmt.Sprintf("resolve CDP port: %v", err), ErrorType: FailureCDP, Email: emailAddr}
	}

	executor, err := connectCDPExecutor(debugPort)
	if err != nil {
		return RoundResult{Error: fmt.Sprintf("CDP connect: %v", err), ErrorType: FailureCDP, Email: emailAddr}
	}
	defer executor.Close()

	// Phase 3a: Navigate to homepage first (natural browsing path, not direct signup)
	state("navigate", "Navigating to DeepSeek homepage...")
	if err := executor.Navigate(deepseekHomeURL); err != nil {
		return RoundResult{Error: fmt.Sprintf("navigate home: %v", err), ErrorType: FailureCDP, Email: emailAddr}
	}
	_ = waitForNavigation(executor, 15*time.Second)

	// Phase 3b: Natural browsing — scroll, read, move mouse like a real visitor
	state("browse", "Simulating natural homepage browsing...")
	_ = executor.SimulateNaturalBrowsing(6 * time.Second)

	// Phase 3c: Navigate to signup (like clicking "Sign Up" after browsing)
	state("navigate", "Navigating to signup page...")
	if err := executor.Navigate(deepseekSignupURL); err != nil {
		return RoundResult{Error: fmt.Sprintf("navigate signup: %v", err), ErrorType: FailureCDP, Email: emailAddr}
	}
	_ = waitForPageReady(executor, 30*time.Second)

	// Phase 3d: Scroll through signup form before filling (natural "reading the form")
	_ = executor.SimulateNaturalBrowsing(3 * time.Second)

	// Phase 4: Fill signup form with natural pauses between fields
	state("fill_form", "Filling email with humanized typing...")
	if err := executor.ExecuteHumanizedType("#email", emailAddr); err != nil {
		return RoundResult{Error: fmt.Sprintf("type email: %v", err), ErrorType: FailureCDP, Email: emailAddr}
	}
	// Natural pause — human checks email is correct, moves mouse
	_ = executor.MoveMouseRandom()
	sleepCtx(ctx, 800+time.Duration(rand.Intn(1200))*time.Millisecond)

		pageInfo, _ := executor.EvaluateJS("(function() { var btns = document.querySelectorAll(\"button\"); var r = \"BUTTONS:\"; for (var i = 0; i < btns.length; i++) { var b = btns[i]; r += \"[\" + i + \"] txt=\" + (b.textContent||\"\").trim().substring(0,30) + \" vis=\"+ (b.offsetParent!==null) + \";\"; } var inp = document.querySelectorAll(\"input\"); r += \" | INP:\"; for (var j = 0; j < inp.length; j++) { r += inp[j].type + \"/\" + (inp[j].name||\"\") + \"/\" + (inp[j].placeholder||\"\") + \";\"; } var ts = document.querySelector(\"[name=cf-turnstile-response]\"); r += \" | TS:\" + (ts?(ts.value?\"has-tok\":\"no-tok\"):\"absent\"); var cf = document.querySelector(\"iframe[src*=challenges]\"); r += \" | CF_iframe:\" + (cf?\"yes\":\"no\"); return r; })()")
		deps.Log.Info("page diag", logger.F("info", pageInfo))

		// Click the primary form button to trigger Turnstile + send verification code.
		if err := clickFormButton(executor); err != nil {
			deps.Log.Warn("click form button", logger.F("error", err))
		}
		sleepCtx(ctx, 1*time.Second)

	// Phase 5: Handle Turnstile
	state("captcha", "Handling Turnstile captcha...")
	turnstileToken, err := waitForTurnstileToken(executor, turnstileTimeout)
	if err != nil {
		return RoundResult{Error: fmt.Sprintf("turnstile: %v", err), ErrorType: FailureTurnstile, Email: emailAddr}
	}
	state("captcha", "Turnstile token obtained")

	// Phase 6: Send verification code
	state("send_code", "Requesting email verification code...")
	if err := sendVerificationCode(executor, emailAddr, turnstileToken); err != nil {
		return RoundResult{Error: fmt.Sprintf("send code: %v", err), ErrorType: FailureAPI, Email: emailAddr}
	}

	// Phase 7: Poll email for code via Cloudflare Worker
	state("poll_email", "Waiting for verification code in inbox...")
	otp, err := mailClient.WaitForCode(ctx, 3*time.Minute)
	if err != nil {
		return RoundResult{Error: fmt.Sprintf("email poll: %v", err), ErrorType: FailureCodeTimeout, Email: emailAddr}
	}
	state("poll_email", "Verification code received: "+otp)

	if err := checkEmailCode(executor, emailAddr, otp); err != nil {
		deps.Log.Warn("check email code", logger.F("error", err))
	}

		// Phase 8: Complete registration — fill password, code, then submit.
		state("verify", "Entering password and verification code...")
		if err := executor.ExecuteHumanizedType("input[type=password]", password); err != nil {
			deps.Log.Warn("type password", logger.F("error", err))
		}
		time.Sleep(300 + time.Duration(rand.Intn(500))*time.Millisecond)

		if err := executor.ExecuteHumanizedType("input[placeholder*='code' i], input[name*='code' i]", otp); err != nil {
			deps.Log.Warn("type code", logger.F("error", err))
		}
		time.Sleep(300 + time.Duration(rand.Intn(500))*time.Millisecond)

		// Click the final Sign Up / Register button
		if err := clickFormButton(executor); err != nil {
			deps.Log.Warn("click signup button", logger.F("error", err))
		}
		_ = waitForNavigation(executor, 20*time.Second)

		// Also call the register API as fallback
		if err := registerUser(executor, emailAddr, password, otp); err != nil {
			go func() {
				webhook.Send(context.Background(), "deepseek.register.failed", map[string]interface{}{
					"email": emailAddr,
					"error": err.Error(),
				})
			}()
			return RoundResult{Error: fmt.Sprintf("register: %v", err), ErrorType: FailureAPI, Email: emailAddr}
		}
	time.Sleep(1 * time.Second)

	// Phase 9: Extract API key — browse naturally before navigating
	state("api_key", "Browsing before API keys...")
	_ = executor.SimulateNaturalBrowsing(2 * time.Second)
	_ = executor.Navigate(deepseekAPIKeysURL)
	_ = waitForPageReady(executor, 20*time.Second)
	// Natural scroll before clicking
	_ = executor.SimulateNaturalBrowsing(2 * time.Second)

	_ = clickByTextJS(executor, "Create new key")
	time.Sleep(1 * time.Second)
	_ = clickByTextJS(executor, "Create")
	time.Sleep(2 * time.Second)

	apiKey, _ := extractAPIKey(executor)
	if apiKey == "" {
		apiKey, _ = extractAPIKeyFromBody(executor)
	}

	state("done", "Registration complete: "+emailAddr)
	go func() {
		webhook.Send(context.Background(), "deepseek.register.success", map[string]interface{}{
			"email":    emailAddr,
			"duration": time.Since(start).String(),
		})
	}()
	return RoundResult{
		Success:   true,
		Email:     emailAddr,
		Password:  password,
		APIKey:    apiKey,
		ProfileID: profileID,
		Duration:  time.Since(start).Round(time.Second).String(),
	}
}

// ─── Full pipeline: multi-round batch ───────────────────────────────────────────

// DeepSeekRegisterPipeline runs the complete multi-round batch registration.
func DeepSeekRegisterPipeline(ctx context.Context, deps RegisterDeps, cfg BatchConfig) []RoundResult {
	if cfg.TotalRounds <= 0 {
		cfg.TotalRounds = 5
	}

	results := make([]RoundResult, 0, cfg.TotalRounds)
	adapt := &AdaptationState{BestTier: proxy.Tier1TaiwanResidential}
	selector := proxy.NewProxySelector(deps.ProxyDAO)

	for round := 1; round <= cfg.TotalRounds; round++ {
		deps.OnProgress("round_start", fmt.Sprintf("=== Round %d / %d ===", round, cfg.TotalRounds))

		tier := determineTier(round, adapt)
		selectedProxy, err := selector.SelectBest(tier)
		if err != nil {
			deps.Log.Error("no proxy available", logger.F("round", round), logger.F("tier", int(tier)), logger.F("error", err))
			results = append(results, RoundResult{
				Round: round, Error: fmt.Sprintf("proxy select: %v", err), ErrorType: FailureProxy, Tier: tier,
			})
			continue
		}

		proxyInfo := ProxyInfo{
			ProxyID: selectedProxy.ProxyId, ProxyName: selectedProxy.ProxyName,
		}
		if health := parseIPHealth(selectedProxy.LastIPHealthJSON); health != nil {
			proxyInfo.Country = health.CountryCode
			proxyInfo.IsResidential = health.IsResidential
			proxyInfo.FraudScore = health.FraudScore
		}

		// Create browser profile with proxy
		profile, err := deps.BrowserMgr.Create(browser.ProfileInput{
			ProfileName: fmt.Sprintf("DS-Batch-R%d-%d", round, time.Now().Unix()),
			ProxyId:     selectedProxy.ProxyId,
		})
		if err != nil {
			deps.Log.Error("profile create failed", logger.F("round", round), logger.F("error", err))
			results = append(results, RoundResult{Round: round, Error: fmt.Sprintf("profile: %v", err), ErrorType: FailureBrowser, Proxy: proxyInfo, Tier: tier})
			continue
		}

		// Start browser with proxy bridge (Xray/Sing-Box auto-detected)
		startedProfile, err := deps.StartBrowser(profile.ProfileId)
		if err != nil {
			deps.Log.Error("browser start failed", logger.F("round", round), logger.F("error", err))
			cleanupProfile(deps.BrowserMgr, profile)
			results = append(results, RoundResult{Round: round, Error: fmt.Sprintf("start: %v", err), ErrorType: FailureBrowser, Proxy: proxyInfo, Tier: tier})
			continue
		}
		profileID := startedProfile.ProfileId

		// Run registration
		result := runSingleRegistration(ctx, deps, profileID, email.GenerateHumanPassword(), cfg.TurnstileTimeout)
		result.Round = round
		result.Proxy = proxyInfo
		result.Tier = tier

		// Cleanup: stop browser and delete profile
		if _, err := deps.StopBrowser(profileID); err != nil {
			deps.Log.Warn("browser stop failed", logger.F("profileId", profileID), logger.F("error", err))
		}
		cleanupProfile(deps.BrowserMgr, profile)
		selector.MarkUsed(selectedProxy.ProxyId)

		if !result.Success && (result.ErrorType == FailureProxy ||
			result.ErrorType == FailureWAF ||
			result.ErrorType == FailureTurnstile) {
			selector.Blacklist(selectedProxy.ProxyId)
		}
		adapt.Update(result)
		results = append(results, result)

		// Inter-round delay
		if round < cfg.TotalRounds {
			delay := computeRoundDelay(adapt, cfg)
			deps.OnProgress("wait", fmt.Sprintf("Waiting %v before next round...", delay.Round(time.Second)))
			select {
			case <-time.After(delay):
			case <-ctx.Done():
				return results
			}
		}
	}

	return results
}

// determineTier picks the proxy tier for a given round.
func determineTier(round int, adapt *AdaptationState) proxy.PriorityTier {
	table := []proxy.PriorityTier{
		proxy.Tier1TaiwanResidential,
		proxy.Tier2LowLatency,
		proxy.Tier2LowLatency,
		proxy.Tier1TaiwanResidential,
		proxy.Tier1TaiwanResidential,
	}
	if round-1 < len(table) {
		tier := table[round-1]
		if adapt.TurnstileTimeouts >= 2 && tier != proxy.Tier1TaiwanResidential {
			return proxy.Tier1TaiwanResidential
		}
		if round >= 4 && adapt.BestTier > 0 && len(adapt.SuccessfulProxies) > 0 {
			return adapt.BestTier
		}
		return tier
	}
	return proxy.Tier4AnyNonCN
}

func computeRoundDelay(adapt *AdaptationState, cfg BatchConfig) time.Duration {
	base := cfg.RoundDelayMin
	jitter := time.Duration(rand.Int63n(int64(cfg.RoundDelayMax - cfg.RoundDelayMin)))
	delay := base + jitter
	if adapt.ConsecutiveFails >= 2 {
		delay *= 2
	}
	if adapt.WAFBlocks > 0 {
		delay = delay * 3 / 2
	}
	if delay > cfg.RoundDelayMax*2 {
		delay = cfg.RoundDelayMax * 2
	}
	return delay
}

// ─── Browser helpers ────────────────────────────────────────────────────────────

func resolveProfileCDPPort(mgr *browser.Manager, profileID string) (int, error) {
	mgr.Mutex.Lock()
	profile, exists := mgr.Profiles[profileID]
	var snapshot *browser.Profile
	if profile != nil {
		copied := *profile
		snapshot = &copied
	}
	mgr.Mutex.Unlock()

	if !exists || snapshot == nil {
		return 0, fmt.Errorf("profile not found: %s", profileID)
	}
	if !snapshot.Running || !snapshot.DebugReady {
		return 0, fmt.Errorf("browser not running or debug not ready for profile %s", profileID)
	}
	return snapshot.DebugPort, nil
}

func cleanupProfile(mgr *browser.Manager, profile *browser.Profile) {
	if profile.ProfileId != "" {
		_ = mgr.Delete(profile.ProfileId)
	}
	if profile.UserDataDir != "" {
		userDataPath := mgr.ResolveRelativePath(profile.UserDataDir)
		_ = os.RemoveAll(userDataPath)
	}
}

// ─── CDP helpers ────────────────────────────────────────────────────────────────

func connectCDPExecutor(debugPort int) (*behavior.CDPExecutor, error) {
	ws, err := behavior.ConnectPageCDP(debugPort)
	if err != nil {
		return nil, err
	}
	cfg := humanize.ConfigForLevel(humanize.LevelHigh)
	return behavior.NewCDPExecutor(ws, cfg), nil
}

func navigatePageCDP(debugPort int, url string) error {
	ws, err := behavior.ConnectPageCDP(debugPort)
	if err != nil {
		return err
	}
	defer ws.Close()
	_, err = behavior.ExecuteCDP(ws, "Page.navigate", map[string]interface{}{"url": url})
	return err
}

func waitForTurnstileToken(executor *behavior.CDPExecutor, timeout time.Duration) (string, error) {
	deadline := time.Now().Add(timeout)
	for {
		if time.Now().After(deadline) {
			return "", fmt.Errorf("turnstile token timeout after %v", timeout)
		}

		token, err := executor.EvaluateJS(`(function() {
			var input = document.querySelector('[name="cf-turnstile-response"]');
			if (input && input.value) return input.value;
			var frames = document.querySelectorAll('iframe');
			for (var i = 0; i < frames.length; i++) {
				try {
					var doc = frames[i].contentWindow.document;
					var inp = doc.querySelector('[name="cf-turnstile-response"]');
					if (inp && inp.value) return inp.value;
				} catch(e) {}
			}
			return '';
		})()`)
		if err == nil && token != "" {
			return token, nil
		}
		time.Sleep(2 * time.Second)
	}
}

func sendVerificationCode(executor *behavior.CDPExecutor, emailAddr, turnstileToken string) error {
	js := fmt.Sprintf(`(async function() {
		const deviceId = crypto.randomUUID();
		const resp = await fetch(%q + "/v0/users/create_email_verification_code", {
			method: "POST",
			headers: {"Content-Type": "application/json"},
			body: JSON.stringify({
				email: %q,
				turnstile_token: %q,
				locale: "en",
				device_id: deviceId,
			}),
		});
		const data = await resp.json();
		const bizCode = (data.data && data.data.biz_code) || data.code;
		return JSON.stringify({status: resp.status, bizCode: bizCode, raw: JSON.stringify(data)});
	})()`, deepseekAuthAPI, emailAddr, turnstileToken)

	raw, err := executor.EvaluateJS(js)
	if err != nil {
		return fmt.Errorf("send code eval: %w", err)
	}

	var result struct {
		Status  int    `json:"status"`
		BizCode int    `json:"bizCode"`
		Raw     string `json:"raw"`
	}
	if err := json.Unmarshal([]byte(raw), &result); err != nil {
		return fmt.Errorf("parse send code result: %w (raw=%s)", err, raw)
	}
	if result.Status != 200 {
		return fmt.Errorf("send code HTTP %d: %s", result.Status, result.Raw)
	}
	if result.BizCode != 0 {
		return fmt.Errorf("send code biz_code=%d: %s", result.BizCode, result.Raw)
	}
	return nil
}

func checkEmailCode(executor *behavior.CDPExecutor, emailAddr, code string) error {
	js := fmt.Sprintf(`(async function() {
		const resp = await fetch(%q + "/v0/users/check_email_code", {
			method: "POST",
			headers: {"Content-Type": "application/json"},
			body: JSON.stringify({
				email: %q,
				email_verification_code: %q,
			}),
		});
		const data = await resp.json();
		const bizCode = (data.data && data.data.biz_code) || data.code;
		return JSON.stringify({status: resp.status, bizCode: bizCode});
	})()`, deepseekAuthAPI, emailAddr, code)

	raw, err := executor.EvaluateJS(js)
	if err != nil {
		return fmt.Errorf("check code eval: %w", err)
	}

	var result struct {
		Status  int `json:"status"`
		BizCode int `json:"bizCode"`
	}
	if err := json.Unmarshal([]byte(raw), &result); err != nil {
		return fmt.Errorf("parse check code result: %w", err)
	}
	if result.Status != 200 {
		return fmt.Errorf("check code HTTP %d", result.Status)
	}
	if result.BizCode != 0 {
		return fmt.Errorf("check code biz_code=%d", result.BizCode)
	}
	return nil
}

func registerUser(executor *behavior.CDPExecutor, emailAddr, password, code string) error {
	js := fmt.Sprintf(`(async function() {
		const deviceId = crypto.randomUUID();
		const resp = await fetch(%q + "/v0/users/register", {
			method: "POST",
			headers: {"Content-Type": "application/json"},
			body: JSON.stringify({
				locale: "en",
				region: "US",
				payload: {
					email: %q,
					email_verification_code: %q,
					password: %q,
				},
				device_id: deviceId,
				os: "web",
			}),
		});
		const data = await resp.json();
		const bizCode = (data.data && data.data.biz_code) || data.code;
		const token = (data.data && data.data.token) || data.token || '';
		return JSON.stringify({status: resp.status, bizCode: bizCode, token: token});
	})()`, deepseekAuthAPI, emailAddr, code, password)

	raw, err := executor.EvaluateJS(js)
	if err != nil {
		return fmt.Errorf("register eval: %w", err)
	}

	var result struct {
		Status  int    `json:"status"`
		BizCode int    `json:"bizCode"`
		Token   string `json:"token"`
	}
	if err := json.Unmarshal([]byte(raw), &result); err != nil {
		return fmt.Errorf("parse register result: %w", err)
	}
	if result.Status != 200 && result.Status != 201 {
		return fmt.Errorf("register HTTP %d", result.Status)
	}
	if result.BizCode != 0 && result.BizCode != 200 {
		return fmt.Errorf("register biz_code=%d", result.BizCode)
	}
	return nil
}

func extractAPIKey(executor *behavior.CDPExecutor) (string, error) {
	js := `(function() {
		var t = document.body.innerText || '';
		var m = t.match(/sk-[a-zA-Z0-9]{20,60}/);
		if (m) return m[0];
		try {
			var ls = JSON.parse(localStorage.getItem('deepseek_api_key') || '""');
			if (ls && ls.startsWith('sk-')) return ls;
		} catch(e) {}
		try {
			var ls2 = JSON.parse(localStorage.getItem('api_keys') || '[]');
			if (ls2.length && ls2[0].key && ls2[0].key.startsWith('sk-')) return ls2[0].key;
		} catch(e) {}
		var els = document.querySelectorAll('code, pre, .api-key, [data-key], input[readonly]');
		for (var i = 0; i < els.length; i++) {
			var t = (els[i].textContent || els[i].value || '').trim();
			if (t.startsWith('sk-') && t.length > 20) return t;
		}
		return '';
	})()`
	return executor.EvaluateJS(js)
}

func extractAPIKeyFromBody(executor *behavior.CDPExecutor) (string, error) {
	js := `(function() {
		var t = document.body.innerText || '';
		var m = t.match(/sk-[a-zA-Z0-9]{20,60}/);
		return m ? m[0] : '';
	})()`
	return executor.EvaluateJS(js)
}

// clickFormButton finds the primary submit button and clicks it using CDP mouse
// events (not JS .click()) so that Cloudflare Turnstile properly receives the
// user interaction signal it requires.
func clickFormButton(executor *behavior.CDPExecutor) error {
	// Step 1: find the button and its coordinates.
	js := `(function() {
		var labels = ['发送验证码', 'send code', 'get code', '注册', 'sign up', 'register',
			       'verify', 'create account', 'next', 'continue'];
		var buttons = document.querySelectorAll('button, a[role="button"]');
		for (var i = 0; i < buttons.length; i++) {
			var text = (buttons[i].textContent || buttons[i].value || '').toLowerCase().trim();
			for (var j = 0; j < labels.length; j++) {
				if (text.includes(labels[j])) {
					var r = buttons[i].getBoundingClientRect();
					return JSON.stringify({
						x: r.left + r.width / 2,
						y: r.top + r.height / 2,
						label: labels[j]
					});
				}
			}
		}
		// Fallback: first visible button
		var all = document.querySelectorAll('button');
		for (var k = 0; k < all.length; k++) {
			if (all[k].offsetParent !== null) {
				var r = all[k].getBoundingClientRect();
				return JSON.stringify({
					x: r.left + r.width / 2,
					y: r.top + r.height / 2,
					label: 'visible-btn-' + k
				});
			}
		}
		return 'none';
	})()`

	raw, err := executor.EvaluateJS(js)
	if err != nil {
		return fmt.Errorf("find button: %w", err)
	}
	if raw == "none" {
		return fmt.Errorf("no clickable button found on page")
	}

	var pos struct {
		X     float64 `json:"x"`
		Y     float64 `json:"y"`
		Label string  `json:"label"`
	}
	if err := json.Unmarshal([]byte(raw), &pos); err != nil {
		return fmt.Errorf("parse button position: %w (raw=%s)", err, raw)
	}

	// Step 2: move mouse to the button center with human-like trajectory.
	if err := executor.MoveMouseTo(pos.X, pos.Y); err != nil {
		return fmt.Errorf("move to button: %w", err)
	}

	// Step 3: click using CDP mouse events (mousePressed + mouseReleased).
	if err := executor.Click(); err != nil {
		return fmt.Errorf("click button: %w", err)
	}

	return nil
}

func clickByTextJS(executor *behavior.CDPExecutor, text string) error {
	js := fmt.Sprintf(`(function() {
		var all = document.querySelectorAll('button, a, span, div');
		for (var i = 0; i < all.length; i++) {
			if (all[i].textContent && all[i].textContent.includes(%q)) {
				all[i].click();
				return true;
			}
		}
		return false;
	})()`, text)
	_, err := executor.EvaluateJS(js)
	return err
}

// ─── Context-aware sleep ─────────────────────────────────────────────────────────

func sleepCtx(ctx context.Context, d time.Duration) bool {
	select {
	case <-time.After(d):
		return true
	case <-ctx.Done():
		return false
	}
}

// ─── Wait helpers ────────────────────────────────────────────────────────────────

func waitForNavigation(executor *behavior.CDPExecutor, timeout time.Duration) error {
	deadline := time.Now().Add(timeout)
	js := `document.readyState === 'complete'`
	for {
		if time.Now().After(deadline) {
			return fmt.Errorf("navigation timeout after %v", timeout)
		}
		raw, err := executor.EvaluateJS(js)
		if err == nil && raw == "true" {
			return nil
		}
		time.Sleep(300 * time.Millisecond)
	}
}

func waitForPageReady(executor *behavior.CDPExecutor, timeout time.Duration) error {
	deadline := time.Now().Add(timeout)
	js := `(function() {
		if (document.readyState !== 'complete') return false;
		if (document.querySelectorAll('input[type=email], #email, input[name=email]').length === 0) return false;
		return true;
	})()`
	for {
		if time.Now().After(deadline) {
			url, _ := executor.EvaluateJS("window.location.href")
			return fmt.Errorf("page not ready after %v, current URL: %s", timeout, strings.Trim(url, "\""))
		}
		raw, err := executor.EvaluateJS(js)
		if err == nil && raw == "true" {
			return nil
		}
		time.Sleep(500 * time.Millisecond)
	}
}

// ─── Helpers ────────────────────────────────────────────────────────────────────

func parseIPHealth(jsonStr string) *proxy.IPHealthData {
	if jsonStr == "" || jsonStr == "{}" {
		return nil
	}
	var data proxy.IPHealthData
	if err := json.Unmarshal([]byte(jsonStr), &data); err != nil {
		return nil
	}
	return &data
}
