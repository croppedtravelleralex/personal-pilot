package launchcode

import (
	"context"
	"crypto/sha256"
	"database/sql"
	"errors"
	"fmt"
	"strings"
	"time"
)

const (
	SubscriptionSourceURL         = "url"
	SubscriptionSourceClashImport = "clash_import"
	defaultSubscriptionGroupName  = "订阅代理"
)

var (
	ErrSubscriptionNotFound       = errors.New("proxy subscription not found")
	ErrSubscriptionExists         = errors.New("proxy subscription already exists")
	ErrSubscriptionNotRefreshable = errors.New("proxy subscription is not refreshable")
)

// ProxySubscription is the durable metadata for one proxy subscription source.
type ProxySubscription struct {
	SubscriptionID   string `json:"subscriptionId"`
	Name             string `json:"name"`
	SourceType       string `json:"sourceType"`
	SourceURL        string `json:"-"`
	GroupName        string `json:"groupName"`
	AutoRefresh      bool   `json:"autoRefresh"`
	RefreshIntervalM int    `json:"refreshIntervalM"`
	LastRefreshAt    string `json:"lastRefreshAt"`
	LastError        string `json:"lastError"`
	NodeCount        int    `json:"nodeCount"`
	CreatedAt        string `json:"createdAt"`
	UpdatedAt        string `json:"updatedAt"`
}

// SubscriptionNode is a parsed or persisted node owned by a subscription.
type SubscriptionNode struct {
	ProxyID     string `json:"proxyId"`
	Name        string `json:"name"`
	ProxyConfig string `json:"-"`
	GroupName   string `json:"groupName"`
	SortOrder   int    `json:"sortOrder"`
}

// ProxySubscriptionStore persists subscription metadata and owned proxy nodes.
type ProxySubscriptionStore interface {
	Create(ctx context.Context, subscription ProxySubscription, nodes []SubscriptionNode) (ProxySubscription, error)
	List(ctx context.Context) ([]ProxySubscription, error)
	Get(ctx context.Context, subscriptionID string) (ProxySubscription, error)
	ReplaceNodes(ctx context.Context, subscriptionID string, nodes []SubscriptionNode) (ProxySubscription, error)
	ListNodes(ctx context.Context, subscriptionID string) ([]SubscriptionNode, error)
	Delete(ctx context.Context, subscriptionID string) (int, error)
	RecordRefreshError(ctx context.Context, subscriptionID, message string) error
}

// SQLiteProxySubscriptionStore stores subscriptions and their nodes in SQLite.
// Nodes remain in browser_proxies so the rest of the browser stack can use them.
type SQLiteProxySubscriptionStore struct {
	db *sql.DB
}

func NewSQLiteProxySubscriptionStore(db *sql.DB) *SQLiteProxySubscriptionStore {
	return &SQLiteProxySubscriptionStore{db: db}
}

func (s *SQLiteProxySubscriptionStore) Create(ctx context.Context, subscription ProxySubscription, nodes []SubscriptionNode) (ProxySubscription, error) {
	if s == nil || s.db == nil {
		return ProxySubscription{}, fmt.Errorf("subscription database is unavailable")
	}
	subscription, err := normalizeProxySubscription(subscription)
	if err != nil {
		return ProxySubscription{}, err
	}
	nodes, err = normalizeSubscriptionNodes(nodes)
	if err != nil {
		return ProxySubscription{}, err
	}

	tx, err := s.db.BeginTx(ctx, nil)
	if err != nil {
		return ProxySubscription{}, fmt.Errorf("begin subscription create: %w", err)
	}
	defer tx.Rollback()

	if subscription.SourceURL != "" {
		var existingID string
		err := tx.QueryRowContext(ctx, `SELECT subscription_id FROM proxy_subscriptions WHERE source_url = ?`, subscription.SourceURL).Scan(&existingID)
		if err == nil {
			return ProxySubscription{}, ErrSubscriptionExists
		}
		if err != nil && !errors.Is(err, sql.ErrNoRows) {
			return ProxySubscription{}, fmt.Errorf("check subscription source: %w", err)
		}
	}

	now := time.Now().UTC().Format(time.RFC3339)
	subscription.CreatedAt = now
	subscription.UpdatedAt = now
	subscription.LastRefreshAt = now
	subscription.LastError = ""
	autoRefresh := boolInt(subscription.AutoRefresh)
	if _, err := tx.ExecContext(ctx, `
		INSERT INTO proxy_subscriptions (
			subscription_id, name, source_type, source_url, group_name,
			auto_refresh, refresh_interval_m, last_refresh_at, last_error,
			created_at, updated_at
		) VALUES (?, ?, ?, ?, ?, ?, ?, ?, '', ?, ?)`,
		subscription.SubscriptionID, subscription.Name, subscription.SourceType, subscription.SourceURL,
		subscription.GroupName, autoRefresh, subscription.RefreshIntervalM,
		subscription.LastRefreshAt, subscription.CreatedAt, subscription.UpdatedAt,
	); err != nil {
		if isSubscriptionUniqueError(err) {
			return ProxySubscription{}, ErrSubscriptionExists
		}
		return ProxySubscription{}, fmt.Errorf("insert proxy subscription: %w", err)
	}
	if err := replaceSubscriptionNodesTx(ctx, tx, subscription, nodes); err != nil {
		return ProxySubscription{}, err
	}
	if err := tx.Commit(); err != nil {
		return ProxySubscription{}, fmt.Errorf("commit subscription create: %w", err)
	}
	subscription.NodeCount = len(nodes)
	return subscription, nil
}

func (s *SQLiteProxySubscriptionStore) List(ctx context.Context) ([]ProxySubscription, error) {
	if s == nil || s.db == nil {
		return nil, fmt.Errorf("subscription database is unavailable")
	}
	rows, err := s.db.QueryContext(ctx, subscriptionSelectSQL+` ORDER BY s.created_at ASC`)
	if err != nil {
		return nil, fmt.Errorf("list proxy subscriptions: %w", err)
	}
	defer rows.Close()

	items := make([]ProxySubscription, 0)
	for rows.Next() {
		item, err := scanProxySubscription(rows)
		if err != nil {
			return nil, err
		}
		items = append(items, item)
	}
	if err := rows.Err(); err != nil {
		return nil, fmt.Errorf("iterate proxy subscriptions: %w", err)
	}
	return items, nil
}

func (s *SQLiteProxySubscriptionStore) Get(ctx context.Context, subscriptionID string) (ProxySubscription, error) {
	if s == nil || s.db == nil {
		return ProxySubscription{}, fmt.Errorf("subscription database is unavailable")
	}
	item, err := scanProxySubscription(s.db.QueryRowContext(ctx, subscriptionSelectSQL+` WHERE s.subscription_id = ?`, strings.TrimSpace(subscriptionID)))
	if errors.Is(err, sql.ErrNoRows) {
		return ProxySubscription{}, ErrSubscriptionNotFound
	}
	if err != nil {
		return ProxySubscription{}, fmt.Errorf("get proxy subscription: %w", err)
	}
	return item, nil
}

func (s *SQLiteProxySubscriptionStore) ReplaceNodes(ctx context.Context, subscriptionID string, nodes []SubscriptionNode) (ProxySubscription, error) {
	if s == nil || s.db == nil {
		return ProxySubscription{}, fmt.Errorf("subscription database is unavailable")
	}
	nodes, err := normalizeSubscriptionNodes(nodes)
	if err != nil {
		return ProxySubscription{}, err
	}
	tx, err := s.db.BeginTx(ctx, nil)
	if err != nil {
		return ProxySubscription{}, fmt.Errorf("begin subscription refresh: %w", err)
	}
	defer tx.Rollback()

	subscription, err := getProxySubscriptionTx(ctx, tx, strings.TrimSpace(subscriptionID))
	if err != nil {
		return ProxySubscription{}, err
	}
	if subscription.SourceType != SubscriptionSourceURL || subscription.SourceURL == "" {
		return ProxySubscription{}, ErrSubscriptionNotRefreshable
	}
	now := time.Now().UTC().Format(time.RFC3339)
	subscription.LastRefreshAt = now
	subscription.LastError = ""
	subscription.UpdatedAt = now
	if err := replaceSubscriptionNodesTx(ctx, tx, subscription, nodes); err != nil {
		return ProxySubscription{}, err
	}
	if _, err := tx.ExecContext(ctx, `
		UPDATE proxy_subscriptions
		SET last_refresh_at = ?, last_error = '', updated_at = ?
		WHERE subscription_id = ?`, now, now, subscription.SubscriptionID); err != nil {
		return ProxySubscription{}, fmt.Errorf("update subscription refresh state: %w", err)
	}
	if err := tx.Commit(); err != nil {
		return ProxySubscription{}, fmt.Errorf("commit subscription refresh: %w", err)
	}
	subscription.NodeCount = len(nodes)
	return subscription, nil
}

func (s *SQLiteProxySubscriptionStore) ListNodes(ctx context.Context, subscriptionID string) ([]SubscriptionNode, error) {
	if s == nil || s.db == nil {
		return nil, fmt.Errorf("subscription database is unavailable")
	}
	if _, err := s.Get(ctx, subscriptionID); err != nil {
		return nil, err
	}
	rows, err := s.db.QueryContext(ctx, `
		SELECT proxy_id, proxy_name, proxy_config, COALESCE(group_name, ''), sort_order
		FROM browser_proxies WHERE source_id = ?
		ORDER BY sort_order ASC, created_at ASC`, strings.TrimSpace(subscriptionID))
	if err != nil {
		return nil, fmt.Errorf("list subscription nodes: %w", err)
	}
	defer rows.Close()
	items := make([]SubscriptionNode, 0)
	for rows.Next() {
		var item SubscriptionNode
		if err := rows.Scan(&item.ProxyID, &item.Name, &item.ProxyConfig, &item.GroupName, &item.SortOrder); err != nil {
			return nil, fmt.Errorf("scan subscription node: %w", err)
		}
		items = append(items, item)
	}
	if err := rows.Err(); err != nil {
		return nil, fmt.Errorf("iterate subscription nodes: %w", err)
	}
	return items, nil
}

func (s *SQLiteProxySubscriptionStore) Delete(ctx context.Context, subscriptionID string) (int, error) {
	if s == nil || s.db == nil {
		return 0, fmt.Errorf("subscription database is unavailable")
	}
	subscriptionID = strings.TrimSpace(subscriptionID)
	tx, err := s.db.BeginTx(ctx, nil)
	if err != nil {
		return 0, fmt.Errorf("begin subscription delete: %w", err)
	}
	defer tx.Rollback()
	var exists int
	if err := tx.QueryRowContext(ctx, `SELECT COUNT(*) FROM proxy_subscriptions WHERE subscription_id = ?`, subscriptionID).Scan(&exists); err != nil {
		return 0, fmt.Errorf("check proxy subscription: %w", err)
	}
	if exists == 0 {
		return 0, ErrSubscriptionNotFound
	}
	var nodeCount int
	if err := tx.QueryRowContext(ctx, `SELECT COUNT(*) FROM browser_proxies WHERE source_id = ?`, subscriptionID).Scan(&nodeCount); err != nil {
		return 0, fmt.Errorf("count subscription nodes: %w", err)
	}
	if _, err := tx.ExecContext(ctx, `DELETE FROM browser_proxies WHERE source_id = ?`, subscriptionID); err != nil {
		return 0, fmt.Errorf("delete subscription nodes: %w", err)
	}
	if _, err := tx.ExecContext(ctx, `DELETE FROM proxy_subscriptions WHERE subscription_id = ?`, subscriptionID); err != nil {
		return 0, fmt.Errorf("delete proxy subscription: %w", err)
	}
	if err := tx.Commit(); err != nil {
		return 0, fmt.Errorf("commit subscription delete: %w", err)
	}
	return nodeCount, nil
}

func (s *SQLiteProxySubscriptionStore) RecordRefreshError(ctx context.Context, subscriptionID, message string) error {
	if s == nil || s.db == nil {
		return fmt.Errorf("subscription database is unavailable")
	}
	message = strings.TrimSpace(message)
	if len(message) > 1024 {
		message = message[:1024]
	}
	result, err := s.db.ExecContext(ctx, `
		UPDATE proxy_subscriptions SET last_error = ?, updated_at = ? WHERE subscription_id = ?`,
		message, time.Now().UTC().Format(time.RFC3339), strings.TrimSpace(subscriptionID))
	if err != nil {
		return fmt.Errorf("record subscription refresh error: %w", err)
	}
	affected, err := result.RowsAffected()
	if err != nil {
		return fmt.Errorf("read refresh error update result: %w", err)
	}
	if affected == 0 {
		return ErrSubscriptionNotFound
	}
	return nil
}

const subscriptionSelectSQL = `
	SELECT s.subscription_id, s.name, s.source_type, s.source_url, s.group_name,
	       s.auto_refresh, s.refresh_interval_m, s.last_refresh_at, s.last_error,
	       s.created_at, s.updated_at,
	       (SELECT COUNT(*) FROM browser_proxies p WHERE p.source_id = s.subscription_id)
	FROM proxy_subscriptions s
`

type subscriptionScanner interface {
	Scan(dest ...interface{}) error
}

func scanProxySubscription(scanner subscriptionScanner) (ProxySubscription, error) {
	var item ProxySubscription
	var autoRefresh int
	if err := scanner.Scan(
		&item.SubscriptionID, &item.Name, &item.SourceType, &item.SourceURL, &item.GroupName,
		&autoRefresh, &item.RefreshIntervalM, &item.LastRefreshAt, &item.LastError,
		&item.CreatedAt, &item.UpdatedAt, &item.NodeCount,
	); err != nil {
		return ProxySubscription{}, err
	}
	item.AutoRefresh = autoRefresh != 0
	return item, nil
}

func getProxySubscriptionTx(ctx context.Context, tx *sql.Tx, subscriptionID string) (ProxySubscription, error) {
	var item ProxySubscription
	var autoRefresh int
	err := tx.QueryRowContext(ctx, `
		SELECT subscription_id, name, source_type, source_url, group_name,
		       auto_refresh, refresh_interval_m, last_refresh_at, last_error,
		       created_at, updated_at
		FROM proxy_subscriptions WHERE subscription_id = ?`, subscriptionID).Scan(
		&item.SubscriptionID, &item.Name, &item.SourceType, &item.SourceURL, &item.GroupName,
		&autoRefresh, &item.RefreshIntervalM, &item.LastRefreshAt, &item.LastError,
		&item.CreatedAt, &item.UpdatedAt,
	)
	if errors.Is(err, sql.ErrNoRows) {
		return ProxySubscription{}, ErrSubscriptionNotFound
	}
	if err != nil {
		return ProxySubscription{}, fmt.Errorf("get proxy subscription: %w", err)
	}
	item.AutoRefresh = autoRefresh != 0
	return item, nil
}

func replaceSubscriptionNodesTx(ctx context.Context, tx *sql.Tx, subscription ProxySubscription, nodes []SubscriptionNode) error {
	if _, err := tx.ExecContext(ctx, `DELETE FROM browser_proxies WHERE source_id = ?`, subscription.SubscriptionID); err != nil {
		return fmt.Errorf("clear old subscription nodes: %w", err)
	}
	var maxSort int
	if err := tx.QueryRowContext(ctx, `SELECT COALESCE(MAX(sort_order), -1) FROM browser_proxies`).Scan(&maxSort); err != nil {
		return fmt.Errorf("read proxy sort order: %w", err)
	}
	now := time.Now().UTC().Format(time.RFC3339)
	for index, node := range nodes {
		proxyID := stableSubscriptionNodeID(subscription.SubscriptionID, node.ProxyConfig)
		if _, err := tx.ExecContext(ctx, `
			INSERT INTO browser_proxies (
				proxy_id, proxy_name, proxy_config, group_name,
				source_id, source_url, source_name_prefix,
				source_auto_refresh, source_refresh_interval_m, source_last_refresh_at,
				sort_order, created_at
			) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
			proxyID, node.Name, node.ProxyConfig, subscription.GroupName,
			subscription.SubscriptionID, redactSubscriptionSourceURL(subscription.SourceURL), "SUB",
			boolInt(subscription.AutoRefresh), subscription.RefreshIntervalM, subscription.LastRefreshAt,
			maxSort+1+index, now,
		); err != nil {
			return fmt.Errorf("insert subscription node %d: %w", index, err)
		}
	}
	return nil
}

func normalizeProxySubscription(subscription ProxySubscription) (ProxySubscription, error) {
	subscription.SubscriptionID = strings.TrimSpace(subscription.SubscriptionID)
	subscription.Name = strings.TrimSpace(subscription.Name)
	subscription.SourceType = strings.TrimSpace(subscription.SourceType)
	subscription.SourceURL = strings.TrimSpace(subscription.SourceURL)
	subscription.GroupName = strings.TrimSpace(subscription.GroupName)
	if subscription.SubscriptionID == "" {
		return ProxySubscription{}, fmt.Errorf("subscription id is required")
	}
	if subscription.Name == "" {
		subscription.Name = subscription.SubscriptionID
	}
	if subscription.GroupName == "" {
		subscription.GroupName = defaultSubscriptionGroupName
	}
	switch subscription.SourceType {
	case SubscriptionSourceURL:
		if subscription.SourceURL == "" {
			return ProxySubscription{}, fmt.Errorf("subscription URL is required")
		}
	case SubscriptionSourceClashImport:
		subscription.SourceURL = ""
		subscription.AutoRefresh = false
		subscription.RefreshIntervalM = 0
	default:
		return ProxySubscription{}, fmt.Errorf("unsupported subscription source type %q", subscription.SourceType)
	}
	if subscription.AutoRefresh && subscription.RefreshIntervalM <= 0 {
		return ProxySubscription{}, fmt.Errorf("refresh interval is required when auto refresh is enabled")
	}
	return subscription, nil
}

func normalizeSubscriptionNodes(nodes []SubscriptionNode) ([]SubscriptionNode, error) {
	if len(nodes) == 0 {
		return nil, fmt.Errorf("subscription contains no proxy nodes")
	}
	out := make([]SubscriptionNode, 0, len(nodes))
	seen := make(map[string]struct{}, len(nodes))
	for index, node := range nodes {
		node.Name = strings.TrimSpace(node.Name)
		node.ProxyConfig = strings.TrimSpace(node.ProxyConfig)
		if node.ProxyConfig == "" {
			return nil, fmt.Errorf("subscription node %d has empty proxy config", index)
		}
		key := node.ProxyConfig
		if _, exists := seen[key]; exists {
			continue
		}
		seen[key] = struct{}{}
		if node.Name == "" {
			node.Name = fmt.Sprintf("subscription-node-%d", index+1)
		}
		out = append(out, node)
	}
	if len(out) == 0 {
		return nil, fmt.Errorf("subscription contains no unique proxy nodes")
	}
	return out, nil
}

func stableSubscriptionNodeID(subscriptionID, proxyConfig string) string {
	sum := sha256.Sum256([]byte(strings.TrimSpace(subscriptionID) + "\x00" + strings.TrimSpace(proxyConfig)))
	return fmt.Sprintf("subnode-%x", sum[:10])
}

func boolInt(value bool) int {
	if value {
		return 1
	}
	return 0
}

func isSubscriptionUniqueError(err error) bool {
	if err == nil {
		return false
	}
	message := strings.ToLower(err.Error())
	return strings.Contains(message, "unique constraint") || strings.Contains(message, "constraint failed")
}
