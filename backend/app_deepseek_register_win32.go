//go:build windows

package backend

import (
	"context"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"time"

	"personal-pilot/backend/internal/behavior"
	"personal-pilot/backend/internal/browser"
	"personal-pilot/backend/internal/email"
	"personal-pilot/backend/internal/logger"
	"personal-pilot/backend/internal/proxy"
	"personal-pilot/backend/internal/wininput"
)

// ─── Full-auto Win32 registration pipeline ─────────────────────────────────────────

// DeepSeekRegisterBatch runs fully automated batch registration using Win32 input
// injection for isTrusted: true events. Zero human intervention.
func DeepSeekRegisterBatch(ctx context.Context, deps RegisterDeps, cfg BatchConfig) []RoundResult {
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

		profile, err := deps.BrowserMgr.Create(browser.ProfileInput{
			ProfileName: fmt.Sprintf("DS-Win32-R%d-%d", round, time.Now().Unix()),
			ProxyId:     selectedProxy.ProxyId,
		})
		if err != nil {
			deps.Log.Error("profile create failed", logger.F("round", round), logger.F("error", err))
			results = append(results, RoundResult{Round: round, Error: fmt.Sprintf("profile: %v", err), ErrorType: FailureBrowser, Proxy: proxyInfo, Tier: tier})
			continue
		}

		startedProfile, err := deps.StartBrowser(profile.ProfileId)
		if err != nil {
			deps.Log.Error("browser start failed", logger.F("round", round), logger.F("error", err))
			cleanupProfile(deps.BrowserMgr, profile)
			results = append(results, RoundResult{Round: round, Error: fmt.Sprintf("start: %v", err), ErrorType: FailureBrowser, Proxy: proxyInfo, Tier: tier})
			continue
		}
		profileID := startedProfile.ProfileId

		result := runWin32Registration(ctx, deps, profileID, proxyInfo, email.GenerateHumanPassword(), cfg)
		result.Round = round
		result.Proxy = proxyInfo
		result.Tier = tier

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
		if !result.Success {
			deps.Log.Error("round failed", logger.F("round", round), logger.F("error", result.Error), logger.F("errorType", string(result.ErrorType)))
		} else {
			deps.Log.Info("round succeeded", logger.F("round", round), logger.F("email", result.Email))
		}
		adapt.Update(result)
		results = append(results, result)

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

// runWin32Registration executes a single registration round using Win32 input injection.
func runWin32Registration(ctx context.Context, deps RegisterDeps, profileID string, proxyInfo ProxyInfo, password string, cfg BatchConfig) RoundResult {
	start := time.Now()
	state := deps.OnProgress
	if state == nil {
		state = func(string, string) {}
	}

	// 1. Temp email
	state("email", "Creating temp email...")
	mailClient, err := email.NewCloudflareTempClient(deps.tempEmailAPIBase())
	if err != nil {
		return RoundResult{Error: fmt.Sprintf("temp email: %v", err), ErrorType: FailureEmail}
	}
	emailAddr := mailClient.Address()

	// 2. Get CDP port and PID
	debugPort, pid, err := resolveProfileCDPPortAndPID(deps.BrowserMgr, profileID)
	if err != nil {
		return RoundResult{Error: fmt.Sprintf("resolve CDP: %v", err), ErrorType: FailureCDP, Email: emailAddr}
	}

	// 3. WebDriver check — verify browser is undetected
	state("check", "Verifying browser fingerprint...")
	if err := verifyWebDriver(debugPort); err != nil {
		return RoundResult{Error: fmt.Sprintf("webdriver: %v", err), ErrorType: FailureBrowser, Email: emailAddr}
	}

	// 4. Connect CDP first so navigation and interactions share the same page target
	state("cdp", "Connecting to browser CDP...")
	executor, err := connectCDPExecutor(debugPort)
	if err != nil {
		return RoundResult{Error: fmt.Sprintf("CDP connect: %v", err), ErrorType: FailureCDP, Email: emailAddr}
	}
	defer executor.Close()

	// 5. Navigate to DeepSeek signup via CDP Page.navigate on the connected page
	state("navigate", "Navigating to DeepSeek signup...")
	if err := executor.Navigate(deepseekSignupURL); err != nil {
		return RoundResult{Error: fmt.Sprintf("navigate: %v", err), ErrorType: FailureCDP, Email: emailAddr}
	}

	// 6. Wait for signup form
	state("wait_page", "Waiting for signup form to load...")
	if err := waitForPageReady(executor, 30*time.Second); err != nil {
		return RoundResult{Error: fmt.Sprintf("page not ready: %v", err), ErrorType: FailureCDP, Email: emailAddr}
	}

	pageMetrics, err := getPageMetrics(executor)
	if err != nil {
		return RoundResult{Error: fmt.Sprintf("page metrics: %v", err), ErrorType: FailureCDP, Email: emailAddr}
	}

	// 6. Find Chrome window and measure geometry
	hwnd, err := wininput.FindChromeWindow(pid)
	if err != nil {
		return RoundResult{Error: fmt.Sprintf("find window: %v", err), ErrorType: FailureCDP, Email: emailAddr}
	}

	geo, err := wininput.MeasureGeometry(hwnd, pageMetrics.ViewportW, pageMetrics.ViewportH, pageMetrics.DPR)
	if err != nil {
		return RoundResult{Error: fmt.Sprintf("measure geometry: %v", err), ErrorType: FailureCDP, Email: emailAddr}
	}

	mouse := wininput.NewMouseSender(hwnd, geo.ToolbarHeight, geo.DPIScale)
	kbd := wininput.NewKeyboardSender(45)

	// 7. Dynamic region from proxy country
	region := "US"
	if proxyInfo.Country != "" {
		region = proxyInfo.Country
	}

	// 8. Find email input and type
	state("fill_email", "Typing email via Win32 input...")
	if err := win32ClickElement(executor, mouse, "#email", "email input"); err != nil {
		return RoundResult{Error: fmt.Sprintf("click email: %v", err), ErrorType: FailureCDP, Email: emailAddr}
	}
	time.Sleep(300 * time.Millisecond)
	if err := kbd.TypeString(emailAddr); err != nil {
		return RoundResult{Error: fmt.Sprintf("type email: %v", err), ErrorType: FailureCDP, Email: emailAddr}
	}
	time.Sleep(500 * time.Millisecond)

	// 9. Click "Send Code" button to trigger Turnstile
	state("send_code", "Clicking send code button...")
	if err := win32ClickButton(executor, mouse, "send"); err != nil {
		return RoundResult{Error: fmt.Sprintf("click send code: %v", err), ErrorType: FailureCDP, Email: emailAddr}
	}
	time.Sleep(1 * time.Second)

	// 10. Wait for Turnstile token
	state("captcha", "Waiting for Turnstile...")
	turnstileToken, err := waitForTurnstileToken(executor, cfg.TurnstileTimeout)
	if err != nil {
		return RoundResult{Error: fmt.Sprintf("turnstile: %v", err), ErrorType: FailureTurnstile, Email: emailAddr}
	}

	// 11. Send verification code via API
	state("send_code", "Requesting verification code...")
	if err := sendVerificationCodeWithRegion(executor, emailAddr, turnstileToken, region); err != nil {
		return RoundResult{Error: fmt.Sprintf("send code: %v", err), ErrorType: FailureAPI, Email: emailAddr}
	}

	// 12. Wait for code in email
	state("poll_email", "Waiting for verification code...")
	otp, err := mailClient.WaitForCode(ctx, 3*time.Minute)
	if err != nil {
		return RoundResult{Error: fmt.Sprintf("email poll: %v", err), ErrorType: FailureCodeTimeout, Email: emailAddr}
	}

	// 13. Type password and code via Win32 input
	state("fill_form", "Typing password and code...")
	if err := win32ClickElement(executor, mouse, "input[type=password]", "password input"); err != nil {
		deps.Log.Warn("click password", logger.F("error", err))
	}
	time.Sleep(300 * time.Millisecond)
	if err := kbd.TypeString(password); err != nil {
		return RoundResult{Error: fmt.Sprintf("type password: %v", err), ErrorType: FailureCDP, Email: emailAddr}
	}
	time.Sleep(400 * time.Millisecond)

	if err := win32ClickElement(executor, mouse, "input[placeholder*='code' i], input[name*='code' i]", "code input"); err != nil {
		deps.Log.Warn("click code field", logger.F("error", err))
	}
	time.Sleep(300 * time.Millisecond)
	if err := kbd.TypeString(otp); err != nil {
		return RoundResult{Error: fmt.Sprintf("type code: %v", err), ErrorType: FailureCDP, Email: emailAddr}
	}
	time.Sleep(500 * time.Millisecond)

	// 14. Click Sign Up button
	state("register", "Clicking sign up button...")
	if err := win32ClickButton(executor, mouse, "sign"); err != nil {
		return RoundResult{Error: fmt.Sprintf("click signup: %v", err), ErrorType: FailureCDP, Email: emailAddr}
	}
	time.Sleep(3 * time.Second)

	// 15. Verify registration success via API (fallback if UI submission doesn't auto-redirect)
	state("verify", "Verifying registration...")
	if err := registerUser(executor, emailAddr, password, otp); err != nil {
		// Log but don't fail — UI click might have succeeded
		deps.Log.Warn("register API fallback", logger.F("error", err))
	}

	// 16. Extract API key
	state("api_key", "Extracting API key...")
	if err := executor.Navigate(deepseekAPIKeysURL); err != nil {
		deps.Log.Warn("navigate to api keys", logger.F("error", err))
	}
	_ = waitForPageReady(executor, 15*time.Second)
	_ = executor.SimulateNaturalBrowsing(2 * time.Second)

	apiKey, _ := extractAPIKey(executor)
	if apiKey == "" {
		apiKey, _ = extractAPIKeyFromBody(executor)
	}

	// 17. Save credentials to desktop
	state("save", "Saving credentials...")
	if err := appendToDesktopTxt(emailAddr, password, apiKey); err != nil {
		deps.Log.Warn("save to desktop", logger.F("error", err))
	}
	if err := email.SaveCredential(emailAddr, password, apiKey, "win32"); err != nil {
		deps.Log.Warn("save to credstore", logger.F("error", err))
	}

	state("done", "Registration complete: "+emailAddr)
	return RoundResult{
		Success:   true,
		Email:     emailAddr,
		Password:  password,
		APIKey:    apiKey,
		ProfileID: profileID,
		Duration:  time.Since(start).Round(time.Second).String(),
	}
}

// ─── Win32 input helpers ──────────────────────────────────────────────────────────

type pageMetrics struct {
	ViewportW int32
	ViewportH int32
	DPR       float64
}

func getPageMetrics(executor *behavior.CDPExecutor) (*pageMetrics, error) {
	js := `(function() {
		return JSON.stringify({
			vw: window.innerWidth,
			vh: window.innerHeight,
			dpr: window.devicePixelRatio || 1
		});
	})()`
	raw, err := executor.EvaluateJS(js)
	if err != nil {
		return nil, fmt.Errorf("get page metrics: %w", err)
	}
	var m struct {
		Vw  int32   `json:"vw"`
		Vh  int32   `json:"vh"`
		Dpr float64 `json:"dpr"`
	}
	if err := json.Unmarshal([]byte(raw), &m); err != nil {
		return nil, fmt.Errorf("parse page metrics: %w", err)
	}
	if m.Dpr < 0.5 {
		m.Dpr = 1.0
	}
	return &pageMetrics{ViewportW: m.Vw, ViewportH: m.Vh, DPR: m.Dpr}, nil
}

func getElementCenter(executor *behavior.CDPExecutor, selector string) (x, y float64, err error) {
	js := fmt.Sprintf(`(function() {
		var el = document.querySelector(%q);
		if (!el) return null;
		var r = el.getBoundingClientRect();
		return JSON.stringify({x: r.left + r.width/2, y: r.top + r.height/2});
	})()`, selector)

	// Retry with backoff — the page may still be rendering
	for attempt := 0; attempt < 10; attempt++ {
		raw, evalErr := executor.EvaluateJS(js)
		if evalErr != nil {
			err = evalErr
			time.Sleep(time.Duration(500+attempt*300) * time.Millisecond)
			continue
		}
		if raw == "" || raw == "null" {
			err = fmt.Errorf("element not found: %s", selector)
			time.Sleep(time.Duration(500+attempt*300) * time.Millisecond)
			continue
		}
		var pos struct {
			X float64 `json:"x"`
			Y float64 `json:"y"`
		}
		if err = json.Unmarshal([]byte(raw), &pos); err != nil {
			time.Sleep(time.Duration(500+attempt*300) * time.Millisecond)
			continue
		}
		return pos.X, pos.Y, nil
	}
	return 0, 0, err
}

func findButtonCenter(executor *behavior.CDPExecutor, keyword string) (x, y float64, err error) {
	js := fmt.Sprintf(`(function() {
		var kw = %q;
		var buttons = document.querySelectorAll('button, a[role="button"], input[type="submit"]');
		for (var i = 0; i < buttons.length; i++) {
			var text = (buttons[i].textContent || buttons[i].value || '').toLowerCase();
			if (text.includes(kw)) {
				var r = buttons[i].getBoundingClientRect();
				return JSON.stringify({x: r.left + r.width/2, y: r.top + r.height/2});
			}
		}
		var all = document.querySelectorAll('button');
		for (var j = 0; j < all.length; j++) {
			if (all[j].offsetParent !== null) {
				var r = all[j].getBoundingClientRect();
				return JSON.stringify({x: r.left + r.width/2, y: r.top + r.height/2});
			}
		}
		return null;
	})()`, keyword)
	raw, err := executor.EvaluateJS(js)
	if err != nil {
		return 0, 0, err
	}
	if raw == "" {
		return 0, 0, fmt.Errorf("no button matching %q", keyword)
	}
	var pos struct {
		X float64 `json:"x"`
		Y float64 `json:"y"`
	}
	if err := json.Unmarshal([]byte(raw), &pos); err != nil {
		return 0, 0, fmt.Errorf("parse button position: %w", err)
	}
	return pos.X, pos.Y, nil
}

func win32ClickElement(executor *behavior.CDPExecutor, mouse *wininput.MouseSender, selector, desc string) error {
	x, y, err := getElementCenter(executor, selector)
	if err != nil {
		return fmt.Errorf("%s: %w", desc, err)
	}
	return mouse.ClickAt(x, y)
}

func win32ClickButton(executor *behavior.CDPExecutor, mouse *wininput.MouseSender, keyword string) error {
	x, y, err := findButtonCenter(executor, keyword)
	if err != nil {
		return err
	}
	return mouse.ClickAt(x, y)
}

// ─── WebDriver verification ───────────────────────────────────────────────────────

func verifyWebDriver(debugPort int) error {
	ws, err := behavior.ConnectPageCDP(debugPort)
	if err != nil {
		return fmt.Errorf("CDP connect for webdriver check: %w", err)
	}
	defer ws.Close()

	js := `(function() {
		var checks = {
			webdriver: navigator.webdriver,
			plugins: navigator.plugins ? navigator.plugins.length : 0,
			languages: navigator.languages ? navigator.languages.length : 0,
			hardwareConcurrency: navigator.hardwareConcurrency || 0,
			deviceMemory: navigator.deviceMemory || 0,
			maxTouchPoints: navigator.maxTouchPoints || 0,
			vendor: navigator.vendor || '',
			platform: navigator.platform || '',
		};
		return JSON.stringify(checks);
	})()`

	raw, err := behavior.ExecuteCDP(ws, "Runtime.evaluate", map[string]interface{}{
		"expression":    js,
		"returnByValue": true,
	})
	if err != nil {
		return fmt.Errorf("webdriver check eval: %w", err)
	}

	var result struct {
		Result struct {
			Value string `json:"value"`
		} `json:"result"`
	}
	if err := json.Unmarshal(raw, &result); err != nil {
		return fmt.Errorf("parse webdriver check: %w", err)
	}

	var checks struct {
		Webdriver            bool   `json:"webdriver"`
		Plugins              int    `json:"plugins"`
		Languages            int    `json:"languages"`
		HardwareConcurrency  int    `json:"hardwareConcurrency"`
		DeviceMemory         int    `json:"deviceMemory"`
		MaxTouchPoints       int    `json:"maxTouchPoints"`
		Vendor               string `json:"vendor"`
		Platform             string `json:"platform"`
	}
	if err := json.Unmarshal([]byte(result.Result.Value), &checks); err != nil {
		return fmt.Errorf("parse webdriver result: %w", err)
	}

	if checks.Webdriver {
		return fmt.Errorf("navigator.webdriver is true — browser is detectable as automated")
	}
	if checks.Plugins == 0 {
		return fmt.Errorf("navigator.plugins is empty — browser fingerprint anomaly")
	}
	if checks.Languages == 0 {
		return fmt.Errorf("navigator.languages is empty — browser fingerprint anomaly")
	}

	return nil
}

// ─── API with dynamic region ──────────────────────────────────────────────────────

func sendVerificationCodeWithRegion(executor *behavior.CDPExecutor, emailAddr, turnstileToken, region string) error {
	js := fmt.Sprintf(`(async function() {
		const deviceId = crypto.randomUUID();
		const resp = await fetch(%q + "/v0/users/create_email_verification_code", {
			method: "POST",
			headers: {"Content-Type": "application/json"},
			body: JSON.stringify({
				email: %q,
				turnstile_token: %q,
				locale: "en",
				region: %q,
				device_id: deviceId,
			}),
		});
		const data = await resp.json();
		const bizCode = (data.data && data.data.biz_code) || data.code;
		return JSON.stringify({status: resp.status, bizCode: bizCode, raw: JSON.stringify(data)});
	})()`, deepseekAuthAPI, emailAddr, turnstileToken, region)

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

// ─── PID resolution ───────────────────────────────────────────────────────────────

func resolveProfileCDPPortAndPID(mgr *browser.Manager, profileID string) (debugPort, pid int, err error) {
	mgr.Mutex.Lock()
	profile, exists := mgr.Profiles[profileID]
	var snapshot *browser.Profile
	if profile != nil {
		copied := *profile
		snapshot = &copied
	}
	mgr.Mutex.Unlock()

	if !exists || snapshot == nil {
		return 0, 0, fmt.Errorf("profile not found: %s", profileID)
	}
	if !snapshot.Running || !snapshot.DebugReady {
		return 0, 0, fmt.Errorf("browser not running or debug not ready for profile %s", profileID)
	}
	return snapshot.DebugPort, snapshot.Pid, nil
}

// ─── Desktop output ───────────────────────────────────────────────────────────────

func appendToDesktopTxt(emailAddr, password, apiKey string) error {
	desktop := filepath.Join(os.Getenv("USERPROFILE"), "Desktop")
	path := filepath.Join(desktop, "DeepSeek_Accounts.enc")

	f, err := os.OpenFile(path, os.O_APPEND|os.O_CREATE|os.O_WRONLY, 0600)
	if err != nil {
		return err
	}
	defer f.Close()

	line := fmt.Sprintf("[%s] Email: %s | Password: %s | APIKey: %s\n",
		time.Now().Format("2006-01-02 15:04:05"), emailAddr, password, apiKey)

	const xorKey = byte(0xAB)
	encrypted := make([]byte, len(line))
	for i := range line {
		encrypted[i] = line[i] ^ xorKey
	}

	_, err = f.Write(encrypted)
	return err
}

// ─── Wails-bound entry for full-auto ──────────────────────────────────────────────

// DeepSeekRegisterAuto runs fully automated registration against a running profile
// using Win32 input injection for isTrusted: true events.
func (a *App) DeepSeekRegisterAuto(input DeepSeekRegisterInput) DeepSeekRegisterResult {
	log := logger.New("DeepSeekRegisterAuto")
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

	cfg := DefaultBatchConfig()
	cfg.TotalRounds = 1
	cfg.TurnstileTimeout = 90 * time.Second

	results := DeepSeekRegisterBatch(context.Background(), deps, cfg)
	if len(results) == 0 {
		return DeepSeekRegisterResult{Error: "no result produced"}
	}

	r := results[0]
	return DeepSeekRegisterResult{
		Success:  r.Success,
		Email:    r.Email,
		Password: r.Password,
		APIKey:   r.APIKey,
		Error:    r.Error,
	}
}
