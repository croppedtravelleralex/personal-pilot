package launchcode

import (
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"net/http"
	"net/url"
	"regexp"
	"strings"
	"time"

	"github.com/google/uuid"

	"personal-pilot/backend/internal/logger"
	"personal-pilot/backend/internal/proxy"
)

const (
	subscriptionStoreUnavailableCode = "subscription_store_unavailable"
	subscriptionFetchFailedCode      = "subscription_fetch_failed"
	subscriptionNotFoundCode         = "subscription_not_found"
	subscriptionExistsCode           = "subscription_already_exists"
	subscriptionNotRefreshableCode   = "subscription_not_refreshable"
	invalidSubscriptionRequestCode   = "invalid_subscription_request"
	minSubscriptionRefreshIntervalM  = 5
	maxSubscriptionRefreshIntervalM  = 7 * 24 * 60
)

var (
	errSubscriptionFetchFailed = errors.New("subscription fetch failed")
	subscriptionURLInError     = regexp.MustCompile(`https?://[^\s"'<>]+`)
)

// SubscribeRequest creates a durable URL-backed subscription.
type SubscribeRequest struct {
	Name             string `json:"name"`
	URL              string `json:"url"`
	GroupName        string `json:"groupName"`
	AutoRefresh      bool   `json:"autoRefresh"`
	RefreshIntervalM int    `json:"refreshIntervalM"`
}

type importClashSubscriptionRequest struct {
	Name      string `json:"name"`
	GroupName string `json:"groupName"`
	Raw       string `json:"raw"`
}

func (s *LaunchServer) SetProxySubscriptionStore(store ProxySubscriptionStore) {
	s.subscriptionStore = store
}

func (s *LaunchServer) SetSubscriptionFetcher(fetcher SubscriptionFetcher) {
	if fetcher == nil {
		fetcher = NewHTTPSubscriptionFetcher(nil)
	}
	s.subscriptionFetcher = fetcher
}

func (s *LaunchServer) handleSubscribe(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		writeJSON(w, http.StatusMethodNotAllowed, map[string]interface{}{"ok": false, "error": "method not allowed"})
		return
	}
	if !s.subscriptionServiceAvailable(w) {
		return
	}
	var req SubscribeRequest
	if err := decodeSubscriptionJSON(r, &req, 1<<20); err != nil {
		writeSubscriptionRequestError(w, err)
		return
	}
	subscription, err := subscriptionFromRequest(req)
	if err != nil {
		writeSubscriptionRequestError(w, err)
		return
	}
	nodes, err := s.fetchSubscriptionNodes(r.Context(), subscription.SourceURL)
	if err != nil {
		writeSubscriptionError(w, err)
		return
	}
	created, err := s.subscriptionStore.Create(r.Context(), subscription, nodes)
	if err != nil {
		writeSubscriptionError(w, err)
		return
	}
	writeJSON(w, http.StatusCreated, map[string]interface{}{
		"ok":   true,
		"item": subscriptionResponseItem(created),
	})
}

func (s *LaunchServer) handleSubscribeByID(w http.ResponseWriter, r *http.Request) {
	path := strings.Trim(strings.TrimPrefix(r.URL.Path, "/api/proxy/subscribe/"), "/")
	parts := strings.Split(path, "/")
	if path == "" || len(parts) > 2 || strings.TrimSpace(parts[0]) == "" {
		writeJSON(w, http.StatusBadRequest, map[string]interface{}{"ok": false, "code": invalidSubscriptionRequestCode, "error": "invalid subscription id"})
		return
	}
	subscriptionID := strings.TrimSpace(parts[0])
	if len(parts) == 1 {
		if r.Method != http.MethodDelete {
			writeJSON(w, http.StatusMethodNotAllowed, map[string]interface{}{"ok": false, "error": "method not allowed"})
			return
		}
		s.handleSubscribeDelete(w, r, subscriptionID)
		return
	}
	switch strings.TrimSpace(parts[1]) {
	case "refresh":
		if r.Method != http.MethodPost {
			writeJSON(w, http.StatusMethodNotAllowed, map[string]interface{}{"ok": false, "error": "method not allowed"})
			return
		}
		s.handleSubscribeRefresh(w, r, subscriptionID)
	case "validate":
		if r.Method != http.MethodPost {
			writeJSON(w, http.StatusMethodNotAllowed, map[string]interface{}{"ok": false, "error": "method not allowed"})
			return
		}
		s.handleSubscribeValidate(w, r, subscriptionID)
	case "nodes":
		if r.Method != http.MethodGet {
			writeJSON(w, http.StatusMethodNotAllowed, map[string]interface{}{"ok": false, "error": "method not allowed"})
			return
		}
		s.handleSubscribeNodes(w, r, subscriptionID)
	default:
		writeJSON(w, http.StatusNotFound, map[string]interface{}{"ok": false, "code": subscriptionNotFoundCode, "error": "subscription action not found"})
	}
}

func (s *LaunchServer) handleSubscribeRefresh(w http.ResponseWriter, r *http.Request, subscriptionID string) {
	if !s.subscriptionServiceAvailable(w) {
		return
	}
	item, err := s.refreshSubscription(r.Context(), subscriptionID)
	if err != nil {
		writeSubscriptionError(w, err)
		return
	}
	writeJSON(w, http.StatusOK, map[string]interface{}{"ok": true, "item": subscriptionResponseItem(item)})
}

func (s *LaunchServer) handleSubscribeValidate(w http.ResponseWriter, r *http.Request, subscriptionID string) {
	if !s.subscriptionServiceAvailable(w) {
		return
	}
	item, err := s.subscriptionStore.Get(r.Context(), subscriptionID)
	if err != nil {
		writeSubscriptionError(w, err)
		return
	}
	if item.SourceType != SubscriptionSourceURL || item.SourceURL == "" {
		writeSubscriptionError(w, ErrSubscriptionNotRefreshable)
		return
	}
	nodes, err := s.fetchSubscriptionNodes(r.Context(), item.SourceURL)
	if err != nil {
		writeSubscriptionError(w, err)
		return
	}
	writeJSON(w, http.StatusOK, map[string]interface{}{
		"ok":        true,
		"valid":     true,
		"nodeCount": len(nodes),
		"preview":   subscriptionNodePreview(nodes, 10),
	})
}

func (s *LaunchServer) handleSubscribeNodes(w http.ResponseWriter, r *http.Request, subscriptionID string) {
	if !s.subscriptionServiceAvailable(w) {
		return
	}
	nodes, err := s.subscriptionStore.ListNodes(r.Context(), subscriptionID)
	if err != nil {
		writeSubscriptionError(w, err)
		return
	}
	items := make([]map[string]interface{}, 0, len(nodes))
	for _, node := range nodes {
		items = append(items, subscriptionNodeResponseItem(node))
	}
	writeJSON(w, http.StatusOK, map[string]interface{}{
		"ok":             true,
		"subscriptionId": subscriptionID,
		"count":          len(items),
		"items":          items,
	})
}

func (s *LaunchServer) handleSubscribeImportClash(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		writeJSON(w, http.StatusMethodNotAllowed, map[string]interface{}{"ok": false, "error": "method not allowed"})
		return
	}
	if !s.subscriptionServiceAvailable(w) {
		return
	}
	var req importClashSubscriptionRequest
	if err := decodeSubscriptionJSON(r, &req, maxSubscriptionPayloadBytes+1<<20); err != nil {
		writeSubscriptionRequestError(w, err)
		return
	}
	if strings.TrimSpace(req.Raw) == "" {
		writeSubscriptionRequestError(w, fmt.Errorf("raw Clash YAML is required"))
		return
	}
	if len(req.Raw) > maxSubscriptionPayloadBytes {
		writeSubscriptionRequestError(w, fmt.Errorf("Clash payload exceeds %d bytes", maxSubscriptionPayloadBytes))
		return
	}
	nodes, err := parseClashSubscriptionNodes(req.Raw)
	if err != nil {
		writeSubscriptionRequestError(w, err)
		return
	}
	subscriptionID := "subscription-" + uuid.NewString()
	name := strings.TrimSpace(req.Name)
	if name == "" {
		name = "Clash import"
	}
	groupName := strings.TrimSpace(req.GroupName)
	if groupName == "" {
		groupName = defaultSubscriptionGroupName
	}
	created, err := s.subscriptionStore.Create(r.Context(), ProxySubscription{
		SubscriptionID: subscriptionID,
		Name:           name,
		SourceType:     SubscriptionSourceClashImport,
		GroupName:      groupName,
	}, nodes)
	if err != nil {
		writeSubscriptionError(w, err)
		return
	}
	writeJSON(w, http.StatusCreated, map[string]interface{}{"ok": true, "item": subscriptionResponseItem(created)})
}

func (s *LaunchServer) handleSubscribeDelete(w http.ResponseWriter, r *http.Request, subscriptionID string) {
	if !s.subscriptionServiceAvailable(w) {
		return
	}
	deletedNodeCount, err := s.subscriptionStore.Delete(r.Context(), subscriptionID)
	if err != nil {
		writeSubscriptionError(w, err)
		return
	}
	writeJSON(w, http.StatusOK, map[string]interface{}{
		"ok":               true,
		"subscriptionId":   subscriptionID,
		"deletedNodeCount": deletedNodeCount,
	})
}

func (s *LaunchServer) handleSubscribeList(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodGet {
		writeJSON(w, http.StatusMethodNotAllowed, map[string]interface{}{"ok": false, "error": "method not allowed"})
		return
	}
	if !s.subscriptionServiceAvailable(w) {
		return
	}
	list, err := s.subscriptionStore.List(r.Context())
	if err != nil {
		writeSubscriptionError(w, err)
		return
	}
	items := make([]map[string]interface{}, 0, len(list))
	for _, item := range list {
		items = append(items, subscriptionResponseItem(item))
	}
	writeJSON(w, http.StatusOK, map[string]interface{}{"ok": true, "count": len(items), "items": items})
}

func (s *LaunchServer) subscriptionServiceAvailable(w http.ResponseWriter) bool {
	if s.subscriptionStore == nil || s.subscriptionFetcher == nil {
		writeJSON(w, http.StatusServiceUnavailable, map[string]interface{}{
			"ok":        false,
			"code":      subscriptionStoreUnavailableCode,
			"error":     "proxy subscription store is unavailable",
			"retryable": true,
		})
		return false
	}
	return true
}

func subscriptionFromRequest(req SubscribeRequest) (ProxySubscription, error) {
	sourceURL, err := normalizeSubscriptionSourceURL(req.URL)
	if err != nil {
		return ProxySubscription{}, err
	}
	refreshInterval := req.RefreshIntervalM
	if req.AutoRefresh {
		if refreshInterval == 0 {
			refreshInterval = 60
		}
		if refreshInterval < minSubscriptionRefreshIntervalM || refreshInterval > maxSubscriptionRefreshIntervalM {
			return ProxySubscription{}, fmt.Errorf("refreshIntervalM must be between %d and %d", minSubscriptionRefreshIntervalM, maxSubscriptionRefreshIntervalM)
		}
	} else {
		refreshInterval = 0
	}
	name := strings.TrimSpace(req.Name)
	if name == "" {
		parsed, _ := url.Parse(sourceURL)
		name = parsed.Hostname()
	}
	groupName := strings.TrimSpace(req.GroupName)
	if groupName == "" {
		groupName = defaultSubscriptionGroupName
	}
	return ProxySubscription{
		SubscriptionID:   "subscription-" + uuid.NewString(),
		Name:             name,
		SourceType:       SubscriptionSourceURL,
		SourceURL:        sourceURL,
		GroupName:        groupName,
		AutoRefresh:      req.AutoRefresh,
		RefreshIntervalM: refreshInterval,
	}, nil
}

func (s *LaunchServer) fetchSubscriptionNodes(parent context.Context, sourceURL string) ([]SubscriptionNode, error) {
	ctx, cancel := context.WithTimeout(parent, subscriptionRequestTimeout)
	defer cancel()
	nodes, err := s.subscriptionFetcher.Fetch(ctx, sourceURL)
	if err != nil {
		safeMessage := sanitizeSubscriptionFetchError(sourceURL, err)
		return nil, fmt.Errorf("%w: %s", errSubscriptionFetchFailed, safeMessage)
	}
	return normalizeSubscriptionNodes(nodes)
}

func (s *LaunchServer) refreshSubscription(ctx context.Context, subscriptionID string) (ProxySubscription, error) {
	s.subscriptionRefreshMu.Lock()
	defer s.subscriptionRefreshMu.Unlock()
	item, err := s.subscriptionStore.Get(ctx, subscriptionID)
	if err != nil {
		return ProxySubscription{}, err
	}
	if item.SourceType != SubscriptionSourceURL || item.SourceURL == "" {
		return ProxySubscription{}, ErrSubscriptionNotRefreshable
	}
	nodes, err := s.fetchSubscriptionNodes(ctx, item.SourceURL)
	if err != nil {
		recordCtx, cancel := context.WithTimeout(context.Background(), 2*time.Second)
		_ = s.subscriptionStore.RecordRefreshError(recordCtx, subscriptionID, strings.TrimPrefix(err.Error(), errSubscriptionFetchFailed.Error()+": "))
		cancel()
		return ProxySubscription{}, err
	}
	return s.subscriptionStore.ReplaceNodes(ctx, subscriptionID, nodes)
}

func (s *LaunchServer) refreshDueSubscriptions(ctx context.Context, now time.Time) error {
	if s.subscriptionStore == nil || s.subscriptionFetcher == nil {
		return nil
	}
	items, err := s.subscriptionStore.List(ctx)
	if err != nil {
		return err
	}
	var refreshErrors []error
	for _, item := range items {
		if !subscriptionRefreshDue(item, now) {
			continue
		}
		if _, err := s.refreshSubscription(ctx, item.SubscriptionID); err != nil {
			refreshErrors = append(refreshErrors, fmt.Errorf("subscription %s: %w", item.SubscriptionID, err))
		}
	}
	return errors.Join(refreshErrors...)
}

func subscriptionRefreshDue(item ProxySubscription, now time.Time) bool {
	if !item.AutoRefresh || item.SourceType != SubscriptionSourceURL || item.RefreshIntervalM <= 0 {
		return false
	}
	lastRefresh, err := time.Parse(time.RFC3339, strings.TrimSpace(item.LastRefreshAt))
	if err != nil {
		return true
	}
	return !now.Before(lastRefresh.Add(time.Duration(item.RefreshIntervalM) * time.Minute))
}

func (s *LaunchServer) startSubscriptionRefreshLoop() {
	if s.subscriptionStore == nil || s.subscriptionFetcher == nil {
		return
	}
	s.subscriptionLoopMu.Lock()
	if s.subscriptionLoopCancel != nil {
		s.subscriptionLoopCancel()
	}
	ctx, cancel := context.WithCancel(context.Background())
	s.subscriptionLoopCancel = cancel
	s.subscriptionLoopMu.Unlock()

	go func() {
		ticker := time.NewTicker(time.Minute)
		defer ticker.Stop()
		log := logger.New("ProxySubscription")
		for {
			select {
			case <-ctx.Done():
				return
			case now := <-ticker.C:
				if err := s.refreshDueSubscriptions(ctx, now.UTC()); err != nil {
					log.Warn("订阅自动刷新存在失败", logger.F("error", err.Error()))
				}
			}
		}
	}()
}

func (s *LaunchServer) stopSubscriptionRefreshLoop() {
	s.subscriptionLoopMu.Lock()
	cancel := s.subscriptionLoopCancel
	s.subscriptionLoopCancel = nil
	s.subscriptionLoopMu.Unlock()
	if cancel != nil {
		cancel()
	}
}

func subscriptionResponseItem(item ProxySubscription) map[string]interface{} {
	return map[string]interface{}{
		"subscriptionId":   item.SubscriptionID,
		"name":             item.Name,
		"sourceType":       item.SourceType,
		"sourceUrl":        redactSubscriptionSourceURL(item.SourceURL),
		"groupName":        item.GroupName,
		"autoRefresh":      item.AutoRefresh,
		"refreshIntervalM": item.RefreshIntervalM,
		"lastRefreshAt":    item.LastRefreshAt,
		"lastError":        item.LastError,
		"nodeCount":        item.NodeCount,
		"createdAt":        item.CreatedAt,
		"updatedAt":        item.UpdatedAt,
	}
}

func subscriptionNodeResponseItem(node SubscriptionNode) map[string]interface{} {
	return map[string]interface{}{
		"proxyId":     node.ProxyID,
		"name":        node.Name,
		"proxyConfig": redactSubscriptionNodeConfig(node.ProxyConfig),
		"groupName":   node.GroupName,
		"sortOrder":   node.SortOrder,
	}
}

func subscriptionNodePreview(nodes []SubscriptionNode, limit int) []map[string]interface{} {
	if limit <= 0 || limit > len(nodes) {
		limit = len(nodes)
	}
	items := make([]map[string]interface{}, 0, limit)
	for _, node := range nodes[:limit] {
		items = append(items, map[string]interface{}{
			"name":        node.Name,
			"proxyConfig": redactSubscriptionNodeConfig(node.ProxyConfig),
		})
	}
	return items
}

func redactSubscriptionNodeConfig(raw string) string {
	trimmed := strings.TrimSpace(raw)
	if strings.Contains(trimmed, "\n") || strings.Contains(strings.ToLower(trimmed), "type:") {
		return "clash://redacted"
	}
	parsed, err := url.Parse(trimmed)
	if err != nil {
		return "proxy://redacted"
	}
	switch strings.ToLower(parsed.Scheme) {
	case "http", "https", "socks5", "socks5h":
		return proxy.RedactProxyURL(trimmed)
	case "":
		return "proxy://redacted"
	default:
		return strings.ToLower(parsed.Scheme) + "://redacted"
	}
}

func sanitizeSubscriptionFetchError(sourceURL string, err error) string {
	if err == nil {
		return "subscription fetch failed"
	}
	message := strings.TrimSpace(err.Error())
	message = subscriptionURLInError.ReplaceAllStringFunc(message, func(rawURL string) string {
		trimmed := strings.TrimRight(rawURL, ".,;:)]}")
		suffix := rawURL[len(trimmed):]
		return redactSubscriptionSourceURL(trimmed) + suffix
	})
	if sourceURL != "" {
		message = strings.ReplaceAll(message, sourceURL, redactSubscriptionSourceURL(sourceURL))
	}
	if message == "" {
		return "subscription fetch failed"
	}
	return message
}

func decodeSubscriptionJSON(r *http.Request, dst interface{}, maxBytes int64) error {
	decoder := json.NewDecoder(io.LimitReader(r.Body, maxBytes))
	decoder.DisallowUnknownFields()
	if err := decoder.Decode(dst); err != nil {
		return fmt.Errorf("invalid JSON: %w", err)
	}
	var extra interface{}
	if err := decoder.Decode(&extra); !errors.Is(err, io.EOF) {
		return fmt.Errorf("request body must contain one JSON object")
	}
	return nil
}

func writeSubscriptionRequestError(w http.ResponseWriter, err error) {
	writeJSON(w, http.StatusBadRequest, map[string]interface{}{
		"ok":        false,
		"code":      invalidSubscriptionRequestCode,
		"error":     err.Error(),
		"retryable": false,
	})
}

func writeSubscriptionError(w http.ResponseWriter, err error) {
	status := http.StatusInternalServerError
	code := "subscription_store_error"
	retryable := false
	message := "proxy subscription operation failed"
	switch {
	case errors.Is(err, ErrSubscriptionNotFound):
		status, code, message = http.StatusNotFound, subscriptionNotFoundCode, err.Error()
	case errors.Is(err, ErrSubscriptionExists):
		status, code, message = http.StatusConflict, subscriptionExistsCode, err.Error()
	case errors.Is(err, ErrSubscriptionNotRefreshable):
		status, code, message = http.StatusConflict, subscriptionNotRefreshableCode, err.Error()
	case errors.Is(err, errSubscriptionFetchFailed):
		status, code, retryable, message = http.StatusBadGateway, subscriptionFetchFailedCode, true, err.Error()
	default:
		logger.New("ProxySubscription").Error("订阅存储操作失败", logger.F("error", err.Error()))
	}
	writeJSON(w, status, map[string]interface{}{
		"ok":        false,
		"code":      code,
		"error":     message,
		"retryable": retryable,
	})
}
