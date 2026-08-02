package database

import (
	"database/sql"
	"fmt"
	"strings"

	_ "modernc.org/sqlite"
)

// DB 数据库连接
type DB struct {
	conn *sql.DB
}

// migration 单个版本迁移
type migration struct {
	version int    // 版本号，单调递增，永不修改
	desc    string // 描述，便于日志追踪
	stmts   []string
}

// migrations 所有版本迁移，按 version 升序排列
// 规则：
//   - 只能追加新版本，绝对不能修改已有版本
//   - version 从 1 开始，每次发布新版本时递增
//   - 每个 version 对应一批幂等的 DDL 语句
var migrations = []migration{
	{
		version: 1,
		desc:    "初始化核心表结构",
		stmts: []string{
			`CREATE TABLE IF NOT EXISTS launch_codes (
				profile_id TEXT PRIMARY KEY,
				code       TEXT NOT NULL UNIQUE,
				created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
				updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
			)`,
			`CREATE UNIQUE INDEX IF NOT EXISTS idx_launch_codes_code ON launch_codes(code)`,

			`CREATE TABLE IF NOT EXISTS browser_profiles (
				profile_id       TEXT PRIMARY KEY,
				profile_name     TEXT NOT NULL,
				user_data_dir    TEXT NOT NULL DEFAULT '',
				core_id          TEXT NOT NULL DEFAULT '',
				fingerprint_args TEXT NOT NULL DEFAULT '[]',
				proxy_id         TEXT NOT NULL DEFAULT '',
				proxy_config     TEXT NOT NULL DEFAULT '',
				launch_args      TEXT NOT NULL DEFAULT '[]',
				tags             TEXT NOT NULL DEFAULT '[]',
				keywords         TEXT NOT NULL DEFAULT '[]',
				created_at       DATETIME NOT NULL,
				updated_at       DATETIME NOT NULL
			)`,
			`CREATE INDEX IF NOT EXISTS idx_browser_profiles_created_at ON browser_profiles(created_at)`,

			`CREATE TABLE IF NOT EXISTS browser_proxies (
				proxy_id     TEXT PRIMARY KEY,
				proxy_name   TEXT NOT NULL,
				proxy_config TEXT NOT NULL,
				dns_servers  TEXT NOT NULL DEFAULT '',
				sort_order   INTEGER NOT NULL DEFAULT 0,
				created_at   DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
			)`,

			`CREATE TABLE IF NOT EXISTS browser_cores (
				core_id    TEXT PRIMARY KEY,
				core_name  TEXT NOT NULL,
				core_path  TEXT NOT NULL,
				is_default INTEGER NOT NULL DEFAULT 0,
				sort_order INTEGER NOT NULL DEFAULT 0,
				created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
			)`,

			`CREATE TABLE IF NOT EXISTS browser_bookmarks (
				id         INTEGER PRIMARY KEY AUTOINCREMENT,
				name       TEXT NOT NULL,
				url        TEXT NOT NULL UNIQUE,
				sort_order INTEGER NOT NULL DEFAULT 0
			)`,
		},
	},
	{
		version: 2,
		desc:    "添加实例分组支持",
		stmts: []string{
			`CREATE TABLE IF NOT EXISTS browser_groups (
				group_id   TEXT PRIMARY KEY,
				group_name TEXT NOT NULL,
				parent_id  TEXT DEFAULT '',
				sort_order INTEGER NOT NULL DEFAULT 0,
				created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
				updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
			)`,
			`CREATE INDEX IF NOT EXISTS idx_browser_groups_parent_id ON browser_groups(parent_id)`,
			`ALTER TABLE browser_profiles ADD COLUMN group_id TEXT DEFAULT ''`,
		},
	},
	{
		version: 3,
		desc:    "代理表添加分组和测速字段",
		stmts: []string{
			`ALTER TABLE browser_proxies ADD COLUMN group_name TEXT NOT NULL DEFAULT ''`,
			`ALTER TABLE browser_proxies ADD COLUMN last_latency_ms INTEGER NOT NULL DEFAULT -1`,
			`ALTER TABLE browser_proxies ADD COLUMN last_test_ok INTEGER NOT NULL DEFAULT 0`,
			`ALTER TABLE browser_proxies ADD COLUMN last_tested_at TEXT NOT NULL DEFAULT ''`,
		},
	},
	{
		version: 4,
		desc:    "代理表添加 IP 健康结果字段",
		stmts: []string{
			`ALTER TABLE browser_proxies ADD COLUMN last_ip_health_json TEXT NOT NULL DEFAULT ''`,
		},
	},
	{
		version: 5,
		desc:    "代理表添加 URL 来源与自动刷新字段",
		stmts: []string{
			`ALTER TABLE browser_proxies ADD COLUMN source_id TEXT NOT NULL DEFAULT ''`,
			`ALTER TABLE browser_proxies ADD COLUMN source_url TEXT NOT NULL DEFAULT ''`,
			`ALTER TABLE browser_proxies ADD COLUMN source_name_prefix TEXT NOT NULL DEFAULT ''`,
			`ALTER TABLE browser_proxies ADD COLUMN source_auto_refresh INTEGER NOT NULL DEFAULT 0`,
			`ALTER TABLE browser_proxies ADD COLUMN source_refresh_interval_m INTEGER NOT NULL DEFAULT 0`,
			`ALTER TABLE browser_proxies ADD COLUMN source_last_refresh_at TEXT NOT NULL DEFAULT ''`,
		},
	},
	{
		version: 6,
		desc:    "实例表添加代理绑定快照字段",
		stmts: []string{
			`ALTER TABLE browser_profiles ADD COLUMN proxy_bind_source_id TEXT NOT NULL DEFAULT ''`,
			`ALTER TABLE browser_profiles ADD COLUMN proxy_bind_source_url TEXT NOT NULL DEFAULT ''`,
			`ALTER TABLE browser_profiles ADD COLUMN proxy_bind_name TEXT NOT NULL DEFAULT ''`,
			`ALTER TABLE browser_profiles ADD COLUMN proxy_bind_updated_at TEXT NOT NULL DEFAULT ''`,
		},
	},
	{
		version: 7,
		desc:    "事件日志表",
		stmts: []string{
			`CREATE TABLE IF NOT EXISTS event_log (
				id         INTEGER PRIMARY KEY AUTOINCREMENT,
				event_name TEXT    NOT NULL,
				namespace  TEXT    NOT NULL,
				severity   TEXT    NOT NULL DEFAULT 'info',
				payload    TEXT    NOT NULL DEFAULT '{}',
				created_at TEXT    NOT NULL
			)`,
			`CREATE INDEX IF NOT EXISTS idx_event_log_name ON event_log(event_name)`,
			`CREATE INDEX IF NOT EXISTS idx_event_log_ns ON event_log(namespace)`,
			`CREATE INDEX IF NOT EXISTS idx_event_log_time ON event_log(created_at)`,
		},
	},
	{
		version: 9,
		desc:    "行为模拟配置",
		stmts: []string{
			`ALTER TABLE browser_profiles ADD COLUMN behavior_profile_id TEXT NOT NULL DEFAULT ''`,
		},
	},

	{
		version: 8,
		desc:    "任务调度与自动化规则表",
		stmts: []string{
			`CREATE TABLE IF NOT EXISTS scheduler_tasks (
				id               TEXT PRIMARY KEY,
				name             TEXT    NOT NULL,
				trigger_type     TEXT    NOT NULL DEFAULT 'interval',
				trigger_cron     TEXT    NOT NULL DEFAULT '',
				trigger_interval TEXT    NOT NULL DEFAULT '',
				trigger_event    TEXT    NOT NULL DEFAULT '',
				actions          TEXT    NOT NULL DEFAULT '[]',
				max_retries      INTEGER NOT NULL DEFAULT 3,
				retry_delay      TEXT    NOT NULL DEFAULT '10s',
				depends_on       TEXT    NOT NULL DEFAULT '[]',
				profile_id       TEXT    NOT NULL DEFAULT '',
				enabled          INTEGER NOT NULL DEFAULT 1,
				created_at       TEXT    NOT NULL,
				updated_at       TEXT    NOT NULL
			)`,
			`CREATE INDEX IF NOT EXISTS idx_scheduler_tasks_enabled ON scheduler_tasks(enabled)`,
			`CREATE TABLE IF NOT EXISTS automation_rules (
				id            TEXT PRIMARY KEY,
				name          TEXT    NOT NULL,
				trigger_event TEXT    NOT NULL,
				condition     TEXT    NOT NULL DEFAULT '',
				action        TEXT    NOT NULL DEFAULT 'emit_event',
				action_params TEXT    NOT NULL DEFAULT '{}',
				cooldown      TEXT    NOT NULL DEFAULT '5m',
				enabled       INTEGER NOT NULL DEFAULT 1,
				created_at    TEXT    NOT NULL,
				updated_at    TEXT    NOT NULL
			)`,
		},
	},
	{
		version: 10,
		desc:    "add stable humanize seed to browser profiles",
		stmts: []string{
			`ALTER TABLE browser_profiles ADD COLUMN humanize_seed TEXT NOT NULL DEFAULT ''`,
		},
	},
	{
		version: 11,
		desc:    "workbench detection results and ui state",
		stmts: []string{
			`CREATE TABLE IF NOT EXISTS workbench_detection_results (
				id           TEXT PRIMARY KEY,
				profile_id   TEXT NOT NULL,
				profile_name TEXT NOT NULL DEFAULT '',
				kind         TEXT NOT NULL,
				score        INTEGER NOT NULL DEFAULT 0,
				level        TEXT NOT NULL DEFAULT '',
				source       TEXT NOT NULL DEFAULT '',
				summary      TEXT NOT NULL DEFAULT '[]',
				payload      TEXT NOT NULL DEFAULT '{}',
				created_at   TEXT NOT NULL
			)`,
			`CREATE INDEX IF NOT EXISTS idx_workbench_detection_profile_kind_time
				ON workbench_detection_results(profile_id, kind, created_at DESC)`,
			`CREATE TABLE IF NOT EXISTS workbench_ui_state (
				state_key  TEXT PRIMARY KEY,
				payload    TEXT NOT NULL DEFAULT '{}',
				updated_at TEXT NOT NULL
			)`,
		},
	}, {
		version: 12,
		desc:    "browser cores add kind",
		stmts: []string{
			`ALTER TABLE browser_cores ADD COLUMN kind TEXT NOT NULL DEFAULT 'chromium'`,
		},
	},
	{
		version: 13,
		desc:    "scheduler tasks persist runtime state",
		stmts: []string{
			`CREATE TABLE IF NOT EXISTS scheduler_tasks (
				id               TEXT PRIMARY KEY,
				name             TEXT    NOT NULL,
				trigger_type     TEXT    NOT NULL DEFAULT 'interval',
				trigger_cron     TEXT    NOT NULL DEFAULT '',
				trigger_interval TEXT    NOT NULL DEFAULT '',
				trigger_event    TEXT    NOT NULL DEFAULT '',
				actions          TEXT    NOT NULL DEFAULT '[]',
				max_retries      INTEGER NOT NULL DEFAULT 3,
				retry_delay      TEXT    NOT NULL DEFAULT '10s',
				depends_on       TEXT    NOT NULL DEFAULT '[]',
				profile_id       TEXT    NOT NULL DEFAULT '',
				enabled          INTEGER NOT NULL DEFAULT 1,
				created_at       TEXT    NOT NULL,
				updated_at       TEXT    NOT NULL,
				status           TEXT    NOT NULL DEFAULT 'idle',
				last_run_at      TEXT    NOT NULL DEFAULT '',
				last_error       TEXT    NOT NULL DEFAULT '',
				retry_count      INTEGER NOT NULL DEFAULT 0
			)`,
			`ALTER TABLE scheduler_tasks ADD COLUMN status TEXT NOT NULL DEFAULT 'idle'`,
			`ALTER TABLE scheduler_tasks ADD COLUMN last_run_at TEXT NOT NULL DEFAULT ''`,
			`ALTER TABLE scheduler_tasks ADD COLUMN last_error TEXT NOT NULL DEFAULT ''`,
			`ALTER TABLE scheduler_tasks ADD COLUMN retry_count INTEGER NOT NULL DEFAULT 0`,
		},
	},
	{
		version: 14,
		desc:    "profile trust bundles and asymmetric challenge log",
		stmts: []string{
			`CREATE TABLE IF NOT EXISTS profile_trust_bundles (
				profile_id   TEXT PRIMARY KEY,
				provider     TEXT NOT NULL DEFAULT 'generic',
				payload      TEXT NOT NULL DEFAULT '{}',
				updated_at   TEXT NOT NULL
			)`,
			`CREATE TABLE IF NOT EXISTS asymmetric_challenges (
				id           TEXT PRIMARY KEY,
				profile_id   TEXT NOT NULL,
				site         TEXT NOT NULL DEFAULT '',
				challenge_type TEXT NOT NULL DEFAULT '',
				payload      TEXT NOT NULL DEFAULT '{}',
				created_at   TEXT NOT NULL
			)`,
			`CREATE INDEX IF NOT EXISTS idx_asymmetric_challenges_profile_time
				ON asymmetric_challenges(profile_id, created_at DESC)`,
		},
	},
	{
		version: 15,
		desc:    "stealth state ip budget and probe scores",
		stmts: []string{
			`CREATE TABLE IF NOT EXISTS profile_stealth_state (
				profile_id    TEXT PRIMARY KEY,
				paused_until  TEXT NOT NULL DEFAULT '',
				prefer_api    INTEGER NOT NULL DEFAULT 0,
				creepjs_score REAL NOT NULL DEFAULT 0,
				last_probe_at TEXT NOT NULL DEFAULT '',
				cookies_ok    INTEGER NOT NULL DEFAULT 0,
				updated_at    TEXT NOT NULL
			)`,
			`CREATE TABLE IF NOT EXISTS profile_ip_visits (
				profile_id TEXT NOT NULL,
				exit_ip    TEXT NOT NULL,
				day_key    TEXT NOT NULL,
				visit_count INTEGER NOT NULL DEFAULT 0,
				PRIMARY KEY (profile_id, exit_ip, day_key)
			)`,
		},
	},
	{
		version: 16,
		desc:    "durable proxy subscription store",
		stmts: []string{
			`CREATE TABLE IF NOT EXISTS proxy_subscriptions (
				subscription_id   TEXT PRIMARY KEY,
				name              TEXT NOT NULL,
				source_type       TEXT NOT NULL DEFAULT 'url',
				source_url        TEXT NOT NULL DEFAULT '',
				group_name        TEXT NOT NULL DEFAULT '',
				auto_refresh      INTEGER NOT NULL DEFAULT 0,
				refresh_interval_m INTEGER NOT NULL DEFAULT 0,
				last_refresh_at   TEXT NOT NULL DEFAULT '',
				last_error        TEXT NOT NULL DEFAULT '',
				created_at        TEXT NOT NULL,
				updated_at        TEXT NOT NULL
			)`,
			`CREATE UNIQUE INDEX IF NOT EXISTS idx_proxy_subscriptions_source_url
				ON proxy_subscriptions(source_url) WHERE source_url != ''`,
			`CREATE INDEX IF NOT EXISTS idx_proxy_subscriptions_auto_refresh
				ON proxy_subscriptions(auto_refresh, last_refresh_at)`,
		},
	},
	{
		version: 17,
		desc:    "persona_id on browser_profiles + account_health_daily rollup",
		stmts: []string{
			`ALTER TABLE browser_profiles ADD COLUMN persona_id TEXT NOT NULL DEFAULT ''`,
			`CREATE TABLE IF NOT EXISTS account_health_daily (
				profile_id   TEXT NOT NULL,
				day_key      TEXT NOT NULL,
				site         TEXT NOT NULL DEFAULT '',
				challenges   INTEGER NOT NULL DEFAULT 0,
				successes    INTEGER NOT NULL DEFAULT 0,
				failures     INTEGER NOT NULL DEFAULT 0,
				detector_ok  INTEGER NOT NULL DEFAULT 0,
				detector_n   INTEGER NOT NULL DEFAULT 0,
				updated_at   TEXT NOT NULL DEFAULT '',
				PRIMARY KEY (profile_id, day_key, site)
			)`,
			`CREATE INDEX IF NOT EXISTS idx_account_health_daily_profile_day
				ON account_health_daily(profile_id, day_key DESC)`,
		},
	},
}

// NewDB 创建新的数据库连接
func NewDB(dbPath string) (*DB, error) {
	conn, err := sql.Open("sqlite", dbPath)
	if err != nil {
		return nil, fmt.Errorf("打开数据库失败: %w", err)
	}

	conn.SetMaxOpenConns(1)
	conn.SetMaxIdleConns(1)

	if err := conn.Ping(); err != nil {
		return nil, fmt.Errorf("连接数据库失败: %w", err)
	}

	// WAL 模式：写不阻塞读
	if _, err := conn.Exec(`PRAGMA journal_mode=WAL`); err != nil {
		return nil, fmt.Errorf("设置 WAL 模式失败: %w", err)
	}
	// 开启外键约束
	if _, err := conn.Exec(`PRAGMA foreign_keys=ON`); err != nil {
		return nil, fmt.Errorf("开启外键约束失败: %w", err)
	}

	return &DB{conn: conn}, nil
}

// GetConn 获取数据库连接
func (db *DB) GetConn() *sql.DB {
	return db.conn
}

// Close 关闭数据库连接
func (db *DB) Close() error {
	if db.conn != nil {
		return db.conn.Close()
	}
	return nil
}

// Migrate 执行版本化迁移
// 原理：维护 schema_migrations 表记录已执行版本，每次启动只执行未执行的版本
func (db *DB) Migrate() error {
	// 确保版本记录表存在
	if _, err := db.conn.Exec(`
		CREATE TABLE IF NOT EXISTS schema_migrations (
			version    INTEGER PRIMARY KEY,
			desc       TEXT NOT NULL DEFAULT '',
			applied_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
		)`); err != nil {
		return fmt.Errorf("创建 schema_migrations 表失败: %w", err)
	}

	// 查询已执行的最大版本号
	var currentVersion int
	row := db.conn.QueryRow(`SELECT COALESCE(MAX(version), 0) FROM schema_migrations`)
	if err := row.Scan(&currentVersion); err != nil {
		return fmt.Errorf("查询当前 schema 版本失败: %w", err)
	}

	// 按版本顺序执行未执行的迁移
	for _, m := range migrations {
		if m.version <= currentVersion {
			continue // 已执行，跳过
		}

		// 每个版本在事务内执行，保证原子性
		if err := db.applyMigration(m); err != nil {
			return fmt.Errorf("迁移版本 %d (%s) 失败: %w", m.version, m.desc, err)
		}
	}

	if err := db.ensureBrowserCoreKindColumn(); err != nil {
		return err
	}
	if err := db.ensureSchedulerTaskRuntimeColumns(); err != nil {
		return err
	}
	if err := db.ensureProxySubscriptionSchema(); err != nil {
		return err
	}

	return nil
}

// applyMigration 在事务内执行单个版本的所有语句，并记录版本号
func (db *DB) applyMigration(m migration) error {
	tx, err := db.conn.Begin()
	if err != nil {
		return fmt.Errorf("开启事务失败: %w", err)
	}
	defer tx.Rollback()

	for _, stmt := range m.stmts {
		if _, err := tx.Exec(stmt); err != nil {
			// ALTER TABLE 添加已存在列时忽略（兼容从旧版本直接升级的情况）
			if isColumnExistsError(err) {
				continue
			}
			return fmt.Errorf("执行语句失败 [%s]: %w", truncate(stmt, 60), err)
		}
	}

	// 记录版本号
	if _, err := tx.Exec(
		`INSERT INTO schema_migrations (version, desc) VALUES (?, ?)`,
		m.version, m.desc,
	); err != nil {
		return fmt.Errorf("记录迁移版本失败: %w", err)
	}

	return tx.Commit()
}

func (db *DB) ensureProxySubscriptionSchema() error {
	statements := []string{
		`CREATE TABLE IF NOT EXISTS proxy_subscriptions (
			subscription_id   TEXT PRIMARY KEY,
			name              TEXT NOT NULL,
			source_type       TEXT NOT NULL DEFAULT 'url',
			source_url        TEXT NOT NULL DEFAULT '',
			group_name        TEXT NOT NULL DEFAULT '',
			auto_refresh      INTEGER NOT NULL DEFAULT 0,
			refresh_interval_m INTEGER NOT NULL DEFAULT 0,
			last_refresh_at   TEXT NOT NULL DEFAULT '',
			last_error        TEXT NOT NULL DEFAULT '',
			created_at        TEXT NOT NULL,
			updated_at        TEXT NOT NULL
		)`,
		`CREATE UNIQUE INDEX IF NOT EXISTS idx_proxy_subscriptions_source_url
			ON proxy_subscriptions(source_url) WHERE source_url != ''`,
		`CREATE INDEX IF NOT EXISTS idx_proxy_subscriptions_auto_refresh
			ON proxy_subscriptions(auto_refresh, last_refresh_at)`,
	}
	for _, statement := range statements {
		if _, err := db.conn.Exec(statement); err != nil {
			return fmt.Errorf("修复代理订阅 schema 失败: %w", err)
		}
	}
	hasSourceID, err := db.tableHasColumn("browser_proxies", "source_id")
	if err != nil {
		return fmt.Errorf("检查 browser_proxies.source_id 失败: %w", err)
	}
	if hasSourceID {
		if _, err := db.conn.Exec(`CREATE INDEX IF NOT EXISTS idx_browser_proxies_source_id ON browser_proxies(source_id)`); err != nil {
			return fmt.Errorf("修复 browser_proxies.source_id 索引失败: %w", err)
		}
	}
	return nil
}

func (db *DB) ensureBrowserCoreKindColumn() error {
	hasKind, err := db.tableHasColumn("browser_cores", "kind")
	if err != nil {
		return fmt.Errorf("检查 browser_cores.kind 失败: %w", err)
	}
	if hasKind {
		return nil
	}
	if _, err := db.conn.Exec(`ALTER TABLE browser_cores ADD COLUMN kind TEXT NOT NULL DEFAULT 'chromium'`); err != nil && !isColumnExistsError(err) {
		return fmt.Errorf("修复 browser_cores.kind 失败: %w", err)
	}
	if _, err := db.conn.Exec(
		`INSERT OR IGNORE INTO schema_migrations (version, desc) VALUES (?, ?)`,
		12, "browser cores add kind",
	); err != nil {
		return fmt.Errorf("记录 browser_cores.kind 修复版本失败: %w", err)
	}
	return nil
}

func (db *DB) ensureSchedulerTaskRuntimeColumns() error {
	if _, err := db.conn.Exec(`CREATE TABLE IF NOT EXISTS scheduler_tasks (
		id               TEXT PRIMARY KEY,
		name             TEXT    NOT NULL,
		trigger_type     TEXT    NOT NULL DEFAULT 'interval',
		trigger_cron     TEXT    NOT NULL DEFAULT '',
		trigger_interval TEXT    NOT NULL DEFAULT '',
		trigger_event    TEXT    NOT NULL DEFAULT '',
		actions          TEXT    NOT NULL DEFAULT '[]',
		max_retries      INTEGER NOT NULL DEFAULT 3,
		retry_delay      TEXT    NOT NULL DEFAULT '10s',
		depends_on       TEXT    NOT NULL DEFAULT '[]',
		profile_id       TEXT    NOT NULL DEFAULT '',
		enabled          INTEGER NOT NULL DEFAULT 1,
		created_at       TEXT    NOT NULL,
		updated_at       TEXT    NOT NULL,
		status           TEXT    NOT NULL DEFAULT 'idle',
		last_run_at      TEXT    NOT NULL DEFAULT '',
		last_error       TEXT    NOT NULL DEFAULT '',
		retry_count      INTEGER NOT NULL DEFAULT 0
	)`); err != nil {
		return fmt.Errorf("确保 scheduler_tasks 表存在失败: %w", err)
	}

	columns := []struct {
		name string
		stmt string
	}{
		{"status", `ALTER TABLE scheduler_tasks ADD COLUMN status TEXT NOT NULL DEFAULT 'idle'`},
		{"last_run_at", `ALTER TABLE scheduler_tasks ADD COLUMN last_run_at TEXT NOT NULL DEFAULT ''`},
		{"last_error", `ALTER TABLE scheduler_tasks ADD COLUMN last_error TEXT NOT NULL DEFAULT ''`},
		{"retry_count", `ALTER TABLE scheduler_tasks ADD COLUMN retry_count INTEGER NOT NULL DEFAULT 0`},
	}

	for _, column := range columns {
		hasColumn, err := db.tableHasColumn("scheduler_tasks", column.name)
		if err != nil {
			return fmt.Errorf("检查 scheduler_tasks.%s 失败: %w", column.name, err)
		}
		if hasColumn {
			continue
		}
		if _, err := db.conn.Exec(column.stmt); err != nil && !isColumnExistsError(err) {
			return fmt.Errorf("修复 scheduler_tasks.%s 失败: %w", column.name, err)
		}
	}

	if _, err := db.conn.Exec(
		`INSERT OR IGNORE INTO schema_migrations (version, desc) VALUES (?, ?)`,
		13, "scheduler tasks persist runtime state",
	); err != nil {
		return fmt.Errorf("记录 scheduler_tasks runtime state 修复版本失败: %w", err)
	}
	if _, err := db.conn.Exec(`CREATE INDEX IF NOT EXISTS idx_scheduler_tasks_profile_enabled ON scheduler_tasks(profile_id, enabled)`); err != nil {
		return fmt.Errorf("创建 scheduler_tasks 复合索引失败: %w", err)
	}
	if _, err := db.conn.Exec(
		`INSERT OR IGNORE INTO schema_migrations (version, desc) VALUES (?, ?)`,
		14, "scheduler tasks profile enabled index",
	); err != nil {
		return fmt.Errorf("记录 scheduler_tasks 索引迁移失败: %w", err)
	}
	return nil
}

func (db *DB) tableHasColumn(tableName, columnName string) (bool, error) {
	if !isSafeIdentifier(tableName) {
		return false, fmt.Errorf("invalid table name: %s", tableName)
	}
	rows, err := db.conn.Query(fmt.Sprintf(`PRAGMA table_info(%s)`, tableName))
	if err != nil {
		return false, err
	}
	defer rows.Close()

	for rows.Next() {
		var cid int
		var name, typ string
		var notNull int
		var defaultValue sql.NullString
		var pk int
		if err := rows.Scan(&cid, &name, &typ, &notNull, &defaultValue, &pk); err != nil {
			return false, err
		}
		if strings.EqualFold(name, columnName) {
			return true, nil
		}
	}
	return false, rows.Err()
}

func isSafeIdentifier(s string) bool {
	if s == "" {
		return false
	}
	for _, r := range s {
		if (r >= 'a' && r <= 'z') || (r >= 'A' && r <= 'Z') || (r >= '0' && r <= '9') || r == '_' {
			continue
		}
		return false
	}
	return true
}

// isColumnExistsError 检查是否是列已存在的错误（SQLite 错误信息）
func isColumnExistsError(err error) bool {
	if err == nil {
		return false
	}
	s := err.Error()
	return strings.Contains(s, "duplicate column") || strings.Contains(s, "already exists")
}

// truncate 截断字符串用于日志展示
func truncate(s string, n int) string {
	s = strings.TrimSpace(s)
	if len(s) <= n {
		return s
	}
	return s[:n] + "..."
}
