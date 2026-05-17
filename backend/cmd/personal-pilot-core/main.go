package main

import (
	"context"
	"crypto/rand"
	"crypto/subtle"
	"encoding/hex"
	"encoding/json"
	"flag"
	"fmt"
	"log"
	"net"
	"net/http"
	"os"
	"os/signal"
	"path/filepath"
	"personal-pilot/backend"
	"personal-pilot/backend/internal/events"
	"reflect"
	"strconv"
	"strings"
	"sync"
	"syscall"
	"time"
)

const (
	bridgeTokenHeader = "X-Personal-Pilot-Bridge-Token"
	eventTokenHeader  = "X-Personal-Pilot-Event-Token"
)

type eventPayload struct {
	EventName string        `json:"eventName"`
	Data      []interface{} `json:"data"`
}

type shutdownRequest struct {
	Mode string `json:"mode"`
}

type eventHub struct {
	mu      sync.Mutex
	clients map[chan eventPayload]struct{}
}

func newEventHub() *eventHub {
	return &eventHub{clients: make(map[chan eventPayload]struct{})}
}

func (h *eventHub) emit(eventName string, data ...interface{}) {
	h.mu.Lock()
	defer h.mu.Unlock()
	payload := eventPayload{EventName: eventName, Data: data}
	for client := range h.clients {
		select {
		case client <- payload:
		default:
		}
	}
}

func (h *eventHub) subscribe() chan eventPayload {
	ch := make(chan eventPayload, 64)
	h.mu.Lock()
	h.clients[ch] = struct{}{}
	h.mu.Unlock()
	return ch
}

func (h *eventHub) unsubscribe(ch chan eventPayload) {
	h.mu.Lock()
	delete(h.clients, ch)
	h.mu.Unlock()
	close(ch)
}

type rpcRequest struct {
	Method string            `json:"method"`
	Args   []json.RawMessage `json:"args"`
}

type rpcResponse struct {
	OK     bool        `json:"ok"`
	Result interface{} `json:"result,omitempty"`
	Error  string      `json:"error,omitempty"`
}

var allowedRPCMethods = map[string]struct{}{
	"ActiveRecordingStatus":                {},
	"AutomationRuleCreate":                 {},
	"AutomationRuleDelete":                 {},
	"AutomationRuleList":                   {},
	"AutomationRuleTestFire":               {},
	"AutomationRuleToggle":                 {},
	"AutomationRuleUpdate":                 {},
	"BackupExportPackage":                  {},
	"BackupExportPackageToPath":            {},
	"BackupGetManifestTemplate":            {},
	"BackupGetScopeDefinition":             {},
	"BackupImportPackageFromPathConfirmed": {},
	"BackupImportPackagePreflightFromPath": {},
	"BackupImportPackage":                  {},
	"BackupImportPackageFromPath":          {},
	"BackupInitializeSystem":               {},
	"BackupInitializeSystemConfirmed":      {},
	"BackupInitializeSystemPreflight":      {},
	"BehaviorGetRecording":                 {},
	"BehaviorGetRecordingDetail":           {},
	"BehaviorPlayRecording":                {},
	"BehaviorPlaybackReview":               {},
	"BehaviorPresetList":                   {},
	"BehaviorQuickRecord":                  {},
	"BehaviorRecordingCopy":                {},
	"BehaviorRecordingDelete":              {},
	"BehaviorRecordingExport":              {},
	"BehaviorRecordingImport":              {},
	"BehaviorRecordingList":                {},
	"BehaviorRecordingRename":              {},
	"BehaviorRecordingStatus":              {},
	"BehaviorRecordingSummaryList":         {},
	"BehaviorRecordingTrim":                {},
	"BehaviorStartRecording":               {},
	"BehaviorStopPlayback":                 {},
	"BehaviorStopRecording":                {},
	"BookmarkList":                         {},
	"BookmarkReset":                        {},
	"BookmarkSave":                         {},
	"BrowserClearCookies":                  {},
	"BrowserCoreDelete":                    {},
	"BrowserCoreDownload":                  {},
	"BrowserCoreExtendedInfo":              {},
	"BrowserCoreList":                      {},
	"BrowserCoreSave":                      {},
	"BrowserCoreScan":                      {},
	"BrowserCoreSetDefault":                {},
	"BrowserCoreValidate":                  {},
	"BrowserExportCookies":                 {},
	"BrowserGetAllTags":                    {},
	"BrowserGetCookies":                    {},
	"BrowserInstanceGetTabs":               {},
	"BrowserInstanceExecAction":            {},
	"BrowserInstanceOpenUrl":               {},
	"BrowserInstanceRestart":               {},
	"BrowserInstanceStart":                 {},
	"BrowserInstanceStartByCode":           {},
	"BrowserInstanceStartWithParams":       {},
	"BrowserInstanceStatus":                {},
	"BrowserInstanceStop":                  {},
	"BrowserProfileBatchRemoveTags":        {},
	"BrowserProfileBatchSetTags":           {},
	"BrowserProfileCopy":                   {},
	"BrowserProfileCreate":                 {},
	"BrowserProfileDelete":                 {},
	"BrowserProfileGetCode":                {},
	"BrowserProfileList":                   {},
	"BrowserProfileListByTag":              {},
	"BrowserProfileRegenerateCode":         {},
	"BrowserProfileSetCode":                {},
	"BrowserProfileSetKeywords":            {},
	"BrowserProfileUpdate":                 {},
	"BrowserProxyBatchCheckIPHealth":       {},
	"BrowserProxyBatchTestSpeed":           {},
	"BrowserProxyCheckIPHealth":            {},
	"BrowserProxyFetchClashByURL":          {},
	"BrowserProxyFixNames":                 {},
	"BrowserProxyImportSubscriptionByURL":  {},
	"BrowserProxyList":                     {},
	"BrowserProxyListByGroup":              {},
	"BrowserProxyListGroups":               {},
	"BrowserProxyTestSpeed":                {},
	"BrowserRenameTag":                     {},
	"BrowserSnapshotCreate":                {},
	"BrowserSnapshotDelete":                {},
	"BrowserSnapshotList":                  {},
	"BrowserSnapshotRestore":               {},
	"BrowserSnapshotRestoreConfirmed":      {},
	"BrowserSnapshotRestorePreflight":      {},
	"CleanupStaleRecordingSessions":        {},
	"CleanupStaleSessions":                 {},
	"ClearAppLogs":                         {},
	"CreateGroup":                          {},
	"DeleteGroup":                          {},
	"DeleteRecording":                      {},
	"DeepSeekRegister":                     {},
	"EventLogCount":                        {},
	"EventLogExport":                       {},
	"EventLogPrune":                        {},
	"EventLogQuery":                        {},
	"FetchRemoteAuthorProfile":             {},
	"ForceQuit":                            {},
	"GenerateCDKeys":                       {},
	"GetAppConfig":                         {},
	"GetAppLogs":                           {},
	"GetBehaviorPresets":                   {},
	"GetBrowserSettings":                   {},
	"GetDashboardStats":                    {},
	"GetLaunchServerInfo":                  {},
	"GetLicenseStatus":                     {},
	"GetLogLevel":                          {},
	"GetMemoryStats":                       {},
	"GetRecording":                         {},
	"GetRecordingDetail":                   {},
	"GetRunningInstances":                  {},
	"IdentityReportProfile":                {},
	"ListGroups":                           {},
	"ListRecordings":                       {},
	"ListRecordingSummaries":               {},
	"MoveInstancesToGroup":                 {},
	"OpenCorePath":                         {},
	"OpenUserDataDir":                      {},
	"PlayRecording":                        {},
	"QuickRecord":                          {},
	"QuitAppOnly":                          {},
	"RedeemCDKey":                          {},
	"RedeemGithubStar":                     {},
	"ReloadConfig":                         {},
	"SaveBrowserProxies":                   {},
	"SaveBrowserSettings":                  {},
	"SchedulerAddTask":                     {},
	"SchedulerGetEventNames":               {},
	"SchedulerListTasks":                   {},
	"SchedulerRemoveTask":                  {},
	"SchedulerRunTaskNow":                  {},
	"SchedulerTriggerEvent":                {},
	"SetLogLevel":                          {},
	"StartInstance":                        {},
	"StartInstanceWithParams":              {},
	"StartRecording":                       {},
	"StopInstance":                         {},
	"StopPlayback":                         {},
	"StopRecording":                        {},
	"SynchronizerActivateProfile":          {},
	"SynchronizerArrangeProfiles":          {},
	"SynchronizerBroadcastNavigate":        {},
	"SynchronizerBroadcastRefresh":         {},
	"SynchronizerCaptureScreenshot":        {},
	"SynchronizerGetOperationLog":          {},
	"SynchronizerListGroups":               {},
	"SynchronizerListTasks":                {},
	"SynchronizerNavigateProfile":          {},
	"SynchronizerRefreshProfile":           {},
	"SynchronizerSaveTasks":                {},
	"TestProxyConnectivity":                {},
	"TestProxyRealConnectivity":            {},
	"TriggerGC":                            {},
	"UpdateGroup":                          {},
	"ValidateProxyConfig":                  {},
	"WorkbenchActivateProfile":             {},
	"WorkbenchArrangeProfiles":             {},
	"WorkbenchClickElement":                {},
	"WorkbenchExecuteActions":              {},
	"WorkbenchTypeText":                    {},
	"WorkbenchScrollPage":                  {},
	"WorkbenchCaptureScreenshot":           {},
	"WorkbenchFingerprintHealthProfile":    {},
	"WorkbenchFingerprintProfile":          {},
	"WorkbenchGetUiState":                  {},
	"WorkbenchListDetectionResults":        {},
	"WorkbenchListDetectorSites":           {},
	"WorkbenchNavigateProfile":             {},
	"WorkbenchRefreshProfile":              {},
	"WorkbenchRunDetectorSite":             {},
	"WorkbenchSaveDetectionResult":         {},
	"WorkbenchSaveUiState":                 {},
}

func main() {
	var appRoot string
	var version string
	flag.StringVar(&appRoot, "app-root", "", "application root used for config, data, profiles and browser cores")
	flag.StringVar(&version, "version", "unknown", "application version")
	flag.Parse()

	root, err := resolveAppRoot(appRoot)
	if err != nil {
		log.Fatalf("resolve app root: %v", err)
	}
	if err := os.Chdir(root); err != nil {
		log.Fatalf("chdir app root: %v", err)
	}
	if err := backend.EnsureRuntimeLayout(root); err != nil {
		log.Printf("ensure runtime layout: %v", err)
	}

	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()

	hub := newEventHub()
	events.SetFrontendEmitter(hub.emit)
	defer events.SetFrontendEmitter(nil)

	app := backend.NewApp(root, version)
	bridgeToken, err := generateBridgeToken()
	if err != nil {
		log.Fatalf("generate bridge token: %v", err)
	}
	eventToken, err := generateBridgeToken()
	if err != nil {
		log.Fatalf("generate event token: %v", err)
	}
	server, bridgeURL, eventURL, err := startBridgeServer(ctx, app, hub, bridgeToken, eventToken, cancel)
	if err != nil {
		log.Fatalf("start bridge server: %v", err)
	}
	defer func() {
		shutdownCtx, shutdownCancel := context.WithTimeout(context.Background(), 5*time.Second)
		defer shutdownCancel()
		_ = server.Shutdown(shutdownCtx)
	}()

	backend.Start(app, ctx)
	defer backend.Stop(app, ctx)

	ready := map[string]interface{}{
		"bridgeUrl":    bridgeURL,
		"eventUrl":     eventURL,
		"bridgeToken":  bridgeToken,
		"launchServer": app.GetLaunchServerInfo(),
		"pid":          os.Getpid(),
	}
	readyJSON, _ := json.Marshal(ready)
	fmt.Printf("PERSONAL_PILOT_CORE_READY %s\n", readyJSON)

	sigCh := make(chan os.Signal, 2)
	signal.Notify(sigCh, os.Interrupt, syscall.SIGTERM)
	select {
	case <-sigCh:
	case <-ctx.Done():
	}
}

func resolveAppRoot(raw string) (string, error) {
	if strings.TrimSpace(raw) != "" {
		return filepath.Abs(raw)
	}
	cwd, err := os.Getwd()
	if err != nil {
		return "", err
	}
	return filepath.Abs(cwd)
}

func generateBridgeToken() (string, error) {
	var raw [32]byte
	if _, err := rand.Read(raw[:]); err != nil {
		return "", err
	}
	return hex.EncodeToString(raw[:]), nil
}

func startBridgeServer(ctx context.Context, app *backend.App, hub *eventHub, bridgeToken string, eventToken string, cancel context.CancelFunc) (*http.Server, string, string, error) {
	rpcLimiter := newBridgeRateLimiter()
	mux := http.NewServeMux()
	mux.HandleFunc("/health", withCORS(func(w http.ResponseWriter, _ *http.Request) {
		writeJSON(w, http.StatusOK, map[string]interface{}{
			"ok":           true,
			"launchServer": app.GetLaunchServerInfo(),
		})
	}))
	mux.HandleFunc("/rpc", withCORS(requireBridgeToken(bridgeToken, func(w http.ResponseWriter, r *http.Request) {
		if !rpcLimiter.allow(r.RemoteAddr, 60) {
			writeJSON(w, http.StatusTooManyRequests, rpcResponse{OK: false, Error: "rate limit exceeded"})
			return
		}
		if r.Method != http.MethodPost {
			writeJSON(w, http.StatusMethodNotAllowed, rpcResponse{OK: false, Error: "method not allowed"})
			return
		}
		var req rpcRequest
		if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
			writeJSON(w, http.StatusBadRequest, rpcResponse{OK: false, Error: err.Error()})
			return
		}
		result, err := callAppMethod(app, req.Method, req.Args)
		if err != nil {
			writeJSON(w, http.StatusOK, rpcResponse{OK: false, Error: err.Error()})
			return
		}
		writeJSON(w, http.StatusOK, rpcResponse{OK: true, Result: result})
	})))
	mux.HandleFunc("/events", func(w http.ResponseWriter, r *http.Request) {
		setCORS(w)
		if r.Method == http.MethodOptions {
			w.WriteHeader(http.StatusNoContent)
			return
		}
		if !validEventToken(r, eventToken) {
			w.WriteHeader(http.StatusUnauthorized)
			return
		}
		if r.Method != http.MethodGet {
			w.WriteHeader(http.StatusMethodNotAllowed)
			return
		}
		flusher, ok := w.(http.Flusher)
		if !ok {
			http.Error(w, "streaming unsupported", http.StatusInternalServerError)
			return
		}
		w.Header().Set("Content-Type", "text/event-stream")
		w.Header().Set("Cache-Control", "no-cache")
		w.Header().Set("Connection", "keep-alive")
		client := hub.subscribe()
		defer hub.unsubscribe(client)
		fmt.Fprint(w, ": connected\n\n")
		flusher.Flush()
		for {
			select {
			case evt := <-client:
				b, _ := json.Marshal(evt)
				fmt.Fprintf(w, "data: %s\n\n", b)
				flusher.Flush()
			case <-r.Context().Done():
				return
			case <-ctx.Done():
				return
			}
		}
	})
	mux.HandleFunc("/shutdown", withCORS(requireBridgeToken(bridgeToken, func(w http.ResponseWriter, r *http.Request) {
		var req shutdownRequest
		_ = json.NewDecoder(r.Body).Decode(&req)
		switch strings.ToLower(strings.TrimSpace(req.Mode)) {
		case "app-only":
			app.PrepareQuitAppOnly()
		case "full":
			app.PrepareQuitFull()
		}
		writeJSON(w, http.StatusOK, map[string]bool{"ok": true})
		go cancel()
	})))

	ln, err := net.Listen("tcp", "127.0.0.1:0")
	if err != nil {
		return nil, "", "", err
	}
	server := &http.Server{Handler: mux}
	go func() {
		if err := server.Serve(ln); err != nil && err != http.ErrServerClosed {
			log.Printf("bridge server exited: %v", err)
		}
	}()
	bridgeURL := "http://" + ln.Addr().String()
	return server, bridgeURL, bridgeURL + "/events", nil
}

func requireBridgeToken(expected string, next http.HandlerFunc) http.HandlerFunc {
	return func(w http.ResponseWriter, r *http.Request) {
		if !validBridgeToken(r, expected) {
			writeJSON(w, http.StatusUnauthorized, rpcResponse{OK: false, Error: "unauthorized"})
			return
		}
		next(w, r)
	}
}

func validBridgeToken(r *http.Request, expected string) bool {
	if expected == "" {
		return false
	}
	token := strings.TrimSpace(r.Header.Get(bridgeTokenHeader))
	return subtle.ConstantTimeCompare([]byte(token), []byte(expected)) == 1
}

func validEventToken(r *http.Request, expected string) bool {
	if expected == "" {
		return false
	}
	token := strings.TrimSpace(r.Header.Get(eventTokenHeader))
	return subtle.ConstantTimeCompare([]byte(token), []byte(expected)) == 1
}

func callAppMethod(app *backend.App, methodName string, rawArgs []json.RawMessage) (interface{}, error) {
	methodName = strings.TrimSpace(methodName)
	if methodName == "" {
		return nil, fmt.Errorf("method is required")
	}
	if _, ok := allowedRPCMethods[methodName]; !ok {
		return nil, fmt.Errorf("method is not allowed: %s", methodName)
	}
	method := reflect.ValueOf(app).MethodByName(methodName)
	if !method.IsValid() {
		return nil, fmt.Errorf("unknown method: %s", methodName)
	}
	methodType := method.Type()
	if len(rawArgs) != methodType.NumIn() {
		return nil, fmt.Errorf("%s expects %d args, got %d", methodName, methodType.NumIn(), len(rawArgs))
	}
	args := make([]reflect.Value, methodType.NumIn())
	for i := 0; i < methodType.NumIn(); i++ {
		argType := methodType.In(i)
		argPtr := reflect.New(argType)
		if len(rawArgs[i]) == 0 || string(rawArgs[i]) == "undefined" {
			args[i] = reflect.Zero(argType)
			continue
		}
		if err := json.Unmarshal(rawArgs[i], argPtr.Interface()); err != nil {
			return nil, fmt.Errorf("decode arg %d for %s: %w", i+1, methodName, err)
		}
		args[i] = argPtr.Elem()
	}

	var values []reflect.Value
	var panicValue interface{}
	func() {
		defer func() {
			panicValue = recover()
		}()
		values = method.Call(args)
	}()
	if panicValue != nil {
		return nil, fmt.Errorf("%s panicked: %v", methodName, panicValue)
	}
	return normalizeMethodReturns(values)
}

func normalizeMethodReturns(values []reflect.Value) (interface{}, error) {
	if len(values) == 0 {
		return nil, nil
	}
	errorType := reflect.TypeOf((*error)(nil)).Elem()
	last := values[len(values)-1]
	if last.IsValid() && last.Type().Implements(errorType) {
		if !last.IsNil() {
			return nil, last.Interface().(error)
		}
		values = values[:len(values)-1]
	}
	if len(values) == 0 {
		return nil, nil
	}
	if len(values) == 1 {
		if values[0].Kind() == reflect.Pointer && values[0].IsNil() {
			return nil, nil
		}
		return values[0].Interface(), nil
	}
	out := make([]interface{}, 0, len(values))
	for _, value := range values {
		if value.Kind() == reflect.Pointer && value.IsNil() {
			out = append(out, nil)
			continue
		}
		out = append(out, value.Interface())
	}
	return out, nil
}

func withCORS(next http.HandlerFunc) http.HandlerFunc {
	return func(w http.ResponseWriter, r *http.Request) {
		setCORS(w)
		if r.Method == http.MethodOptions {
			w.WriteHeader(http.StatusNoContent)
			return
		}
		next(w, r)
	}
}

func setCORS(w http.ResponseWriter) {
	w.Header().Set("Access-Control-Allow-Origin", "*")
	w.Header().Set("Access-Control-Allow-Headers", "content-type, "+bridgeTokenHeader+", "+eventTokenHeader)
	w.Header().Set("Access-Control-Allow-Methods", "GET, POST, OPTIONS")
}

type bridgeRateLimiter struct {
	mu     sync.Mutex
	counts map[string]int
}

func newBridgeRateLimiter() *bridgeRateLimiter {
	brl := &bridgeRateLimiter{counts: make(map[string]int)}
	go func() {
		ticker := time.NewTicker(time.Second)
		for range ticker.C {
			brl.mu.Lock()
			brl.counts = make(map[string]int)
			brl.mu.Unlock()
		}
	}()
	return brl
}

func (brl *bridgeRateLimiter) allow(ip string, limit int) bool {
	brl.mu.Lock()
	defer brl.mu.Unlock()
	brl.counts[ip]++
	return brl.counts[ip] <= limit
}

func writeJSON(w http.ResponseWriter, status int, payload interface{}) {
	body, err := json.Marshal(payload)
	if err != nil {
		status = http.StatusInternalServerError
		body = []byte(`{"ok":false,"error":"json marshal failed"}`)
	}
	w.Header().Set("Content-Type", "application/json")
	w.Header().Set("Content-Length", strconv.Itoa(len(body)))
	w.WriteHeader(status)
	_, _ = w.Write(body)
}
