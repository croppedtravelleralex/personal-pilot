package main

import (
	"context"
	"database/sql"
	"encoding/json"
	"flag"
	"fmt"
	"os"
	"os/signal"
	"path/filepath"
	"strings"
	"syscall"
	"time"

	_ "modernc.org/sqlite"

	"personal-pilot/backend"
	"personal-pilot/backend/internal/browser"
	"personal-pilot/backend/internal/logger"
	"personal-pilot/backend/internal/proxy"
)

func main() {
	rounds := flag.Int("rounds", 5, "number of registration rounds")
	dryRun := flag.Bool("dry-run", false, "only validate proxy availability, do not register")
	dataDir := flag.String("data-dir", "./data", "path to data directory")
	flag.Parse()

	log := logger.New("deepseek-register")

	// Derive appRoot from dataDir (dataDir is typically <appRoot>/data).
	appRoot := filepath.Dir(*dataDir)
	log.Info("DeepSeek Registration Orchestrator",
		logger.F("rounds", *rounds),
		logger.F("dry_run", *dryRun),
		logger.F("data_dir", *dataDir),
		logger.F("app_root", appRoot),
	)

	if err := os.MkdirAll(*dataDir, 0755); err != nil {
		log.Error("create data dir failed", logger.F("error", err))
		os.Exit(1)
	}

	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()

	// Create App and initialize infrastructure (browserMgr, proxy managers, DB, etc.).
	app := backend.NewApp(appRoot, "deepseek-register-cli")
	backend.Start(app, ctx)
	defer backend.Stop(app, ctx)

	// Separate DB connection for proxy DAO (reads from the same SQLite DB).
	dbPath := filepath.Join(*dataDir, "app.db")
	db, err := sql.Open("sqlite", dbPath)
	if err != nil {
		log.Error("open database failed", logger.F("path", dbPath), logger.F("error", err))
		os.Exit(1)
	}
	defer db.Close()

	proxyDAO := browser.NewSQLiteProxyDAO(db)
	proxySelector := proxy.NewProxySelector(proxyDAO)

	proxies, err := proxyDAO.List()
	if err != nil {
		log.Error("list proxies failed", logger.F("error", err))
		os.Exit(1)
	}
	log.Info("proxies loaded", logger.F("count", len(proxies)))

	if *dryRun {
		runDryRun(log, proxySelector, proxies)
		saveResults(*dataDir, nil)
		return
	}

	// Full registration mode — App already initialized above.
	sigCh := make(chan os.Signal, 1)
	signal.Notify(sigCh, syscall.SIGINT, syscall.SIGTERM)

	cfg2 := backend.DefaultBatchConfig()
	cfg2.TotalRounds = *rounds

	var results []backend.RoundResult

	deps := backend.RegisterDeps{
		BrowserMgr:       app.BrowserMgr(),
		ProxyDAO:         proxyDAO,
		Log:              log,
		OnProgress: func(phase, msg string) {
			log.Info(msg, logger.F("phase", phase))
		},
		StartBrowser: func(pid string) (*backend.BrowserProfile, error) {
			return app.BrowserInstanceStart(pid)
		},
		StopBrowser: func(pid string) (*backend.BrowserProfile, error) {
			return app.BrowserInstanceStop(pid)
		},
		TempEmailAPIBase: os.Getenv("TEMP_EMAIL_API_BASE"),
	}

	done := make(chan struct{})
	go func() {
		defer close(done)
		results = backend.DeepSeekRegisterBatch(ctx, deps, cfg2)
	}()

	select {
	case <-sigCh:
		log.Info("received interrupt signal, finishing current round...")
		cancel()
		<-done
	case <-done:
	}

	printFinalReport(results)
	saveResults(*dataDir, results)
	log.Info("results saved", logger.F("path", filepath.Join(*dataDir, "deepseek_register_results.json")))
}

func determineTier(round int) proxy.PriorityTier {
	// NOTE: This is a simplified version. The canonical implementation
	// in backend/app_deepseek_register.go includes adaptive logic for
	// Turnstile timeouts and best-tier tracking.
	// Consider refactoring into exported API when adaptive CLI is needed.
	table := []proxy.PriorityTier{
		proxy.Tier1TaiwanResidential,
		proxy.Tier2LowLatency,
		proxy.Tier2LowLatency,
		proxy.Tier1TaiwanResidential,
		proxy.Tier1TaiwanResidential,
	}
	if round-1 < len(table) {
		return table[round-1]
	}
	return proxy.Tier4AnyNonCN
}

func classifyError(err error) backend.FailureType {
	if err == nil {
		return backend.FailureNone
	}
	// Use the backend's built-in error classification from RoundResult
	// This is a simplified mapping for the CLI report
	msg := err.Error()
	switch {
	case strings.Contains(msg, "proxy"):
		return backend.FailureProxy
	case strings.Contains(msg, "email") || strings.Contains(msg, "mail"):
		return backend.FailureEmail
	case strings.Contains(msg, "turnstile") || strings.Contains(msg, "captcha"):
		return backend.FailureTurnstile
	case strings.Contains(msg, "cdp") || strings.Contains(msg, "connect"):
		return backend.FailureCDP
	case strings.Contains(msg, "browser"):
		return backend.FailureBrowser
	case strings.Contains(msg, "api") || strings.Contains(msg, "register"):
		return backend.FailureAPI
	case strings.Contains(msg, "code") || strings.Contains(msg, "verification"):
		return backend.FailureCodeTimeout
	default:
		return "unknown"
	}
}

func printRoundBanner(round, total int) {
	bar := strings.Repeat("═", 60)
	fmt.Printf("\n%s\n", bar)
	fmt.Printf("  Round %d / %d\n", round, total)
	fmt.Printf("%s\n\n", bar)
}

func printFinalReport(results []backend.RoundResult) {
	success := 0
	for _, r := range results {
		if r.Success {
			success++
		}
	}

	bar := strings.Repeat("═", 62)
	total := len(results)
	rate := 0
	if total > 0 {
		rate = success * 100 / total
	}

	var totalDur time.Duration
	for _, r := range results {
		if d, err := time.ParseDuration(r.Duration); err == nil {
			totalDur += d
		}
	}

	fmt.Printf("\n╔%s╗\n", bar)
	fmt.Printf("║  DeepSeek Auto Register — %d-Round Batch Report%*s║\n", total, max(0, 62-46-len(fmt.Sprintf("%d", total))), "")
	fmt.Printf("╠%s╣\n", bar)
	fmt.Printf("║  Total: %d   Success: %d/%d (%d%%)   Duration: %s%*s║\n",
		total, success, total, rate, totalDur.Round(time.Second).String(), max(0, 62-60), "")
	fmt.Printf("╠%s╣\n", bar)

	for _, r := range results {
		status := "FAIL"
		if r.Success {
			status = "OK"
		}
		email := r.Email
		if len(email) > 35 {
			email = email[:35]
		}
		dur := r.Duration
		if dur == "" {
			dur = "N/A"
		}
		fmt.Printf("║  R%d: %-4s  %-35s  %-10s║\n", r.Round, status, email, dur)
	}
	fmt.Printf("╠%s╣\n", bar)

	failures := make(map[backend.FailureType]int)
	failureCount := 0
	for _, r := range results {
		if !r.Success {
			failureCount++
			failures[r.ErrorType]++
		}
	}
	if failureCount > 0 {
		fmt.Printf("║  Failure Analysis:%*s║\n", 62-20, "")
		for ft, count := range failures {
			fmt.Printf("║    %-20s: %d%*s║\n", string(ft), count, max(0, 62-30), "")
		}
	}
	fmt.Printf("╚%s╝\n\n", bar)
}

func saveResults(dataDir string, results []backend.RoundResult) {
	outPath := filepath.Join(dataDir, "deepseek_register_results.json")
	f, err := os.Create(outPath)
	if err != nil {
		return
	}
	defer f.Close()
	enc := json.NewEncoder(f)
	enc.SetIndent("", "  ")
	enc.Encode(results)
}

func runDryRun(log *logger.Logger, sel *proxy.ProxySelector, proxies []browser.Proxy) {
	log.Info("=== DRY RUN — proxy validation only ===")
	log.Info("total proxies loaded", logger.F("count", len(proxies)))

	tiers := []proxy.PriorityTier{
		proxy.Tier1TaiwanResidential,
		proxy.Tier2LowLatency,
		proxy.Tier3EuropeBackup,
		proxy.Tier4AnyNonCN,
	}

	for _, tier := range tiers {
		p, err := sel.SelectBest(tier)
		if err != nil {
			log.Warn("no proxy for tier", logger.F("tier", int(tier)), logger.F("error", err))
		} else {
			health := parseIPHealthJSON(p.LastIPHealthJSON)
			log.Info("tier proxy found",
				logger.F("tier", int(tier)),
				logger.F("name", p.ProxyName),
				logger.F("country", health.CountryCode),
				logger.F("fraudScore", health.FraudScore),
				logger.F("isResidential", health.IsResidential),
			)
		}
	}

	log.Info("=== DRY RUN complete ===")
}

func parseIPHealthJSON(raw string) proxy.IPHealthData {
	if raw == "" || raw == "{}" {
		return proxy.IPHealthData{}
	}

	var d proxy.IPHealthData
	// Try first with json tags
	if err := json.Unmarshal([]byte(raw), &d); err != nil || (d.Country == "" && d.CountryCode == "") {
		// Fallback: try map-based extraction
		var wrapper map[string]interface{}
		if err := json.Unmarshal([]byte(raw), &wrapper); err != nil {
			return proxy.IPHealthData{}
		}
		if v, ok := wrapper["country"]; ok {
			d.Country = fmt.Sprint(v)
		}
		if v, ok := wrapper["countryCode"]; ok {
			d.CountryCode = fmt.Sprint(v)
		} else if v, ok := wrapper["country_code"]; ok {
			d.CountryCode = fmt.Sprint(v)
		}
		if v, ok := wrapper["fraudScore"]; ok {
			d.FraudScore = toFloat(v)
		} else if v, ok := wrapper["fraud_score"]; ok {
			d.FraudScore = toFloat(v)
		}
		if v, ok := wrapper["hosting"]; ok {
			d.IsResidential = !isTrue(v)
		}
	}
	return d
}

func toFloat(v interface{}) float64 {
	switch n := v.(type) {
	case float64:
		return n
	case json.Number:
		f, _ := n.Float64()
		return f
	case int:
		return float64(n)
	default:
		return 0
	}
}

func isTrue(v interface{}) bool {
	switch b := v.(type) {
	case bool:
		return b
	case string:
		return strings.EqualFold(b, "true") || b == "1"
	default:
		return false
	}
}

func randInt63(n int64) int64 {
	if n <= 0 {
		return 0
	}
	return int64(time.Now().UnixNano() % n)
}

func max(a, b int) int {
	if a > b {
		return a
	}
	return b
}
