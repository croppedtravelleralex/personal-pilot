package browser

import (
	"database/sql"
	"encoding/json"
	"errors"
	"fmt"
	"strings"
	"time"
)

// ProfileDAO 实例配置持久化接口
type ProfileDAO interface {
	List() ([]*Profile, error)
	GetById(profileId string) (*Profile, error)
	Upsert(profile *Profile) error
	Delete(profileId string) error
}

// SQLiteProfileDAO 基于 SQLite 的 ProfileDAO 实现
type SQLiteProfileDAO struct {
	db *sql.DB
}

// NewSQLiteProfileDAO 创建 SQLiteProfileDAO
func NewSQLiteProfileDAO(db *sql.DB) *SQLiteProfileDAO {
	return &SQLiteProfileDAO{db: db}
}

// List 查询所有实例配置，按创建时间升序
func (d *SQLiteProfileDAO) List() ([]*Profile, error) {
	if err := d.ensureProfileIdentityColumns(); err != nil {
		return nil, err
	}
	rows, err := d.db.Query(`
		SELECT profile_id, profile_name, user_data_dir, core_id,
		       fingerprint_args, proxy_id, proxy_config,
		       COALESCE(proxy_bind_source_id, ''), COALESCE(proxy_bind_source_url, ''),
		       COALESCE(proxy_bind_name, ''), COALESCE(proxy_bind_updated_at, ''),
		       launch_args,
		       tags, keywords, group_id, COALESCE(behavior_profile_id, ''),
		       COALESCE(humanize_seed, ''), COALESCE(persona_id, ''), created_at, updated_at
		FROM browser_profiles ORDER BY created_at ASC`)
	if err != nil {
		return nil, fmt.Errorf("查询实例列表失败: %w", err)
	}
	defer rows.Close()

	var list []*Profile
	for rows.Next() {
		p, err := scanProfile(rows)
		if err != nil {
			return nil, err
		}
		list = append(list, p)
	}
	return list, rows.Err()
}

// GetById 根据 profileId 查询单个实例
func (d *SQLiteProfileDAO) GetById(profileId string) (*Profile, error) {
	if err := d.ensureProfileIdentityColumns(); err != nil {
		return nil, err
	}
	row := d.db.QueryRow(`
		SELECT profile_id, profile_name, user_data_dir, core_id,
		       fingerprint_args, proxy_id, proxy_config,
		       COALESCE(proxy_bind_source_id, ''), COALESCE(proxy_bind_source_url, ''),
		       COALESCE(proxy_bind_name, ''), COALESCE(proxy_bind_updated_at, ''),
		       launch_args,
		       tags, keywords, group_id, COALESCE(behavior_profile_id, ''),
		       COALESCE(humanize_seed, ''), COALESCE(persona_id, ''), created_at, updated_at
		FROM browser_profiles WHERE profile_id = ?`, profileId)
	p, err := scanProfile(row)
	if errors.Is(err, sql.ErrNoRows) {
		return nil, fmt.Errorf("实例不存在: %s", profileId)
	}
	return p, err
}

// Upsert 新增或更新实例配置
func (d *SQLiteProfileDAO) Upsert(profile *Profile) error {
	if profile == nil {
		return fmt.Errorf("profile is nil")
	}
	if err := d.ensureProfileIdentityColumns(); err != nil {
		return err
	}
	if err := d.prepareHumanizeSeedForUpsert(profile); err != nil {
		return err
	}
	if err := d.preparePersonaIDForUpsert(profile); err != nil {
		return err
	}
	fingerprintArgs, _ := json.Marshal(profile.FingerprintArgs)
	launchArgs, _ := json.Marshal(profile.LaunchArgs)
	tags, _ := json.Marshal(profile.Tags)
	keywords, _ := json.Marshal(profile.Keywords)

	now := time.Now().Format(time.RFC3339)
	if profile.CreatedAt == "" {
		profile.CreatedAt = now
	}
	if profile.UpdatedAt == "" {
		profile.UpdatedAt = now
	}

	_, err := d.db.Exec(`
		INSERT INTO browser_profiles
		  (profile_id, profile_name, user_data_dir, core_id, fingerprint_args,
		   proxy_id, proxy_config, proxy_bind_source_id, proxy_bind_source_url, proxy_bind_name, proxy_bind_updated_at,
		   launch_args, tags, keywords, group_id, behavior_profile_id, humanize_seed, persona_id, created_at, updated_at)
		VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
		ON CONFLICT(profile_id) DO UPDATE SET
		  profile_name     = excluded.profile_name,
		  user_data_dir    = excluded.user_data_dir,
		  core_id          = excluded.core_id,
		  fingerprint_args = excluded.fingerprint_args,
		  proxy_id         = excluded.proxy_id,
		  proxy_config     = excluded.proxy_config,
		  proxy_bind_source_id = excluded.proxy_bind_source_id,
		  proxy_bind_source_url = excluded.proxy_bind_source_url,
		  proxy_bind_name = excluded.proxy_bind_name,
		  proxy_bind_updated_at = excluded.proxy_bind_updated_at,
		  launch_args      = excluded.launch_args,
		  tags             = excluded.tags,
		  keywords         = excluded.keywords,
		  group_id         = excluded.group_id,
		  behavior_profile_id = excluded.behavior_profile_id,
		  humanize_seed    = excluded.humanize_seed,
		  persona_id       = excluded.persona_id,
		  updated_at       = excluded.updated_at`,
		profile.ProfileId, profile.ProfileName, profile.UserDataDir, profile.CoreId,
		string(fingerprintArgs), profile.ProxyId, profile.ProxyConfig,
		profile.ProxyBindSourceID, profile.ProxyBindSourceURL, profile.ProxyBindName, profile.ProxyBindUpdatedAt,
		string(launchArgs), string(tags), string(keywords), profile.GroupId,
		profile.BehaviorProfileID, profile.HumanizeSeed, profile.PersonaID,
		profile.CreatedAt, profile.UpdatedAt,
	)
	if err != nil {
		return fmt.Errorf("保存实例配置失败: %w", err)
	}
	return nil
}

// Delete 删除实例配置
func (d *SQLiteProfileDAO) prepareHumanizeSeedForUpsert(profile *Profile) error {
	seed := strings.TrimSpace(profile.HumanizeSeed)
	if seed != "" {
		profile.HumanizeSeed = seed
		return nil
	}
	existingSeed, err := d.getExistingHumanizeSeed(profile.ProfileId)
	if err != nil {
		return err
	}
	if existingSeed != "" {
		profile.HumanizeSeed = existingSeed
		return nil
	}
	ensureProfileHumanizeSeed(profile)
	return nil
}

func (d *SQLiteProfileDAO) getExistingHumanizeSeed(profileId string) (string, error) {
	if strings.TrimSpace(profileId) == "" {
		return "", nil
	}
	var seed string
	err := d.db.QueryRow(`SELECT COALESCE(humanize_seed, '') FROM browser_profiles WHERE profile_id = ?`, profileId).Scan(&seed)
	if errors.Is(err, sql.ErrNoRows) {
		return "", nil
	}
	if err != nil {
		return "", fmt.Errorf("read existing humanize seed: %w", err)
	}
	return strings.TrimSpace(seed), nil
}

func (d *SQLiteProfileDAO) preparePersonaIDForUpsert(profile *Profile) error {
	id := strings.TrimSpace(profile.PersonaID)
	if id != "" {
		profile.PersonaID = id
		return nil
	}
	existing, err := d.getExistingPersonaID(profile.ProfileId)
	if err != nil {
		return err
	}
	if existing != "" {
		profile.PersonaID = existing
		return nil
	}
	ensureProfilePersonaID(profile)
	return nil
}

func (d *SQLiteProfileDAO) getExistingPersonaID(profileId string) (string, error) {
	if strings.TrimSpace(profileId) == "" {
		return "", nil
	}
	var id string
	err := d.db.QueryRow(`SELECT COALESCE(persona_id, '') FROM browser_profiles WHERE profile_id = ?`, profileId).Scan(&id)
	if errors.Is(err, sql.ErrNoRows) {
		return "", nil
	}
	if err != nil {
		// Column may not exist yet on very old DBs before ensure runs.
		msg := strings.ToLower(err.Error())
		if strings.Contains(msg, "no such column") {
			return "", nil
		}
		return "", fmt.Errorf("read existing persona id: %w", err)
	}
	return strings.TrimSpace(id), nil
}

func (d *SQLiteProfileDAO) Delete(profileId string) error {
	_, err := d.db.Exec(`DELETE FROM browser_profiles WHERE profile_id = ?`, profileId)
	if err != nil {
		return fmt.Errorf("删除实例配置失败: %w", err)
	}
	return nil
}

func (d *SQLiteProfileDAO) ensureProfileIdentityColumns() error {
	if err := d.ensureHumanizeSeedColumn(); err != nil {
		return err
	}
	return d.ensurePersonaIDColumn()
}

func (d *SQLiteProfileDAO) ensurePersonaIDColumn() error {
	rows, err := d.db.Query(`PRAGMA table_info(browser_profiles)`)
	if err != nil {
		return fmt.Errorf("inspect browser_profiles schema: %w", err)
	}
	defer rows.Close()

	hasColumn := false
	for rows.Next() {
		var cid int
		var name, columnType string
		var notNull, pk int
		var defaultValue sql.NullString
		if err := rows.Scan(&cid, &name, &columnType, &notNull, &defaultValue, &pk); err != nil {
			return fmt.Errorf("scan browser_profiles schema: %w", err)
		}
		if strings.EqualFold(name, "persona_id") {
			hasColumn = true
			break
		}
	}
	if err := rows.Err(); err != nil {
		return fmt.Errorf("inspect browser_profiles schema rows: %w", err)
	}
	if hasColumn {
		return nil
	}
	if _, err := d.db.Exec(`ALTER TABLE browser_profiles ADD COLUMN persona_id TEXT NOT NULL DEFAULT ''`); err != nil {
		msg := strings.ToLower(err.Error())
		if strings.Contains(msg, "duplicate column") || strings.Contains(msg, "already exists") {
			return nil
		}
		return fmt.Errorf("add browser_profiles.persona_id: %w", err)
	}
	return nil
}

func (d *SQLiteProfileDAO) ensureHumanizeSeedColumn() error {
	rows, err := d.db.Query(`PRAGMA table_info(browser_profiles)`)
	if err != nil {
		return fmt.Errorf("inspect browser_profiles schema: %w", err)
	}
	defer rows.Close()

	hasColumn := false
	for rows.Next() {
		var cid int
		var name, columnType string
		var notNull, pk int
		var defaultValue sql.NullString
		if err := rows.Scan(&cid, &name, &columnType, &notNull, &defaultValue, &pk); err != nil {
			return fmt.Errorf("scan browser_profiles schema: %w", err)
		}
		if strings.EqualFold(name, "humanize_seed") {
			hasColumn = true
			break
		}
	}
	if err := rows.Err(); err != nil {
		return fmt.Errorf("inspect browser_profiles schema rows: %w", err)
	}
	if hasColumn {
		return nil
	}
	if _, err := d.db.Exec(`ALTER TABLE browser_profiles ADD COLUMN humanize_seed TEXT NOT NULL DEFAULT ''`); err != nil {
		msg := strings.ToLower(err.Error())
		if strings.Contains(msg, "duplicate column") || strings.Contains(msg, "already exists") {
			return nil
		}
		return fmt.Errorf("add browser_profiles.humanize_seed: %w", err)
	}
	return nil
}

func (d *SQLiteProfileDAO) ListByGroup(groupId string, includeChildren bool, childGroupIds []string) ([]*Profile, error) {
	if err := d.ensureProfileIdentityColumns(); err != nil {
		return nil, err
	}
	var rows *sql.Rows
	var err error

	if includeChildren && len(childGroupIds) > 0 {
		allIds := append([]string{groupId}, childGroupIds...)
		inClause := ""
		args := make([]interface{}, len(allIds))
		for i, id := range allIds {
			if i > 0 {
				inClause += ","
			}
			inClause += "?"
			args[i] = id
		}
		rows, err = d.db.Query(fmt.Sprintf(`
			SELECT profile_id, profile_name, user_data_dir, core_id,
			       fingerprint_args, proxy_id, proxy_config,
			       COALESCE(proxy_bind_source_id, ''), COALESCE(proxy_bind_source_url, ''),
			       COALESCE(proxy_bind_name, ''), COALESCE(proxy_bind_updated_at, ''),
			       launch_args,
			       tags, keywords, group_id, COALESCE(behavior_profile_id, ''),
			       COALESCE(humanize_seed, ''), COALESCE(persona_id, ''), created_at, updated_at
			FROM browser_profiles WHERE group_id IN (%s) ORDER BY created_at ASC`, inClause), args...)
	} else {
		rows, err = d.db.Query(`
			SELECT profile_id, profile_name, user_data_dir, core_id,
			       fingerprint_args, proxy_id, proxy_config,
			       COALESCE(proxy_bind_source_id, ''), COALESCE(proxy_bind_source_url, ''),
			       COALESCE(proxy_bind_name, ''), COALESCE(proxy_bind_updated_at, ''),
			       launch_args,
			       tags, keywords, group_id, COALESCE(behavior_profile_id, ''),
			       COALESCE(humanize_seed, ''), COALESCE(persona_id, ''), created_at, updated_at
			FROM browser_profiles WHERE group_id = ? ORDER BY created_at ASC`, groupId)
	}

	if err != nil {
		return nil, fmt.Errorf("按分组查询实例失败: %w", err)
	}
	defer rows.Close()

	var list []*Profile
	for rows.Next() {
		p, err := scanProfile(rows)
		if err != nil {
			return nil, err
		}
		list = append(list, p)
	}
	return list, rows.Err()
}

// MoveToGroup 批量移动实例到分组
func (d *SQLiteProfileDAO) MoveToGroup(profileIds []string, groupId string) error {
	if len(profileIds) == 0 {
		return nil
	}
	inClause := ""
	args := make([]interface{}, len(profileIds)+1)
	args[0] = groupId
	for i, id := range profileIds {
		if i > 0 {
			inClause += ","
		}
		inClause += "?"
		args[i+1] = id
	}
	_, err := d.db.Exec(fmt.Sprintf(`UPDATE browser_profiles SET group_id = ? WHERE profile_id IN (%s)`, inClause), args...)
	if err != nil {
		return fmt.Errorf("批量移动实例失败: %w", err)
	}
	return nil
}

// scanner 统一扫描接口，兼容 *sql.Row 和 *sql.Rows
type scanner interface {
	Scan(dest ...any) error
}

func scanProfile(s scanner) (*Profile, error) {
	var (
		fingerprintArgsJSON, launchArgsJSON, tagsJSON, keywordsJSON string
		p                                                           Profile
	)
	err := s.Scan(
		&p.ProfileId, &p.ProfileName, &p.UserDataDir, &p.CoreId,
		&fingerprintArgsJSON, &p.ProxyId, &p.ProxyConfig,
		&p.ProxyBindSourceID, &p.ProxyBindSourceURL, &p.ProxyBindName, &p.ProxyBindUpdatedAt,
		&launchArgsJSON, &tagsJSON, &keywordsJSON, &p.GroupId,
		&p.BehaviorProfileID,
		&p.HumanizeSeed,
		&p.PersonaID,
		&p.CreatedAt, &p.UpdatedAt,
	)
	if err != nil {
		return nil, err
	}
	_ = json.Unmarshal([]byte(fingerprintArgsJSON), &p.FingerprintArgs)
	_ = json.Unmarshal([]byte(launchArgsJSON), &p.LaunchArgs)
	_ = json.Unmarshal([]byte(tagsJSON), &p.Tags)
	_ = json.Unmarshal([]byte(keywordsJSON), &p.Keywords)
	if p.FingerprintArgs == nil {
		p.FingerprintArgs = []string{}
	}
	if p.LaunchArgs == nil {
		p.LaunchArgs = []string{}
	}
	if p.Tags == nil {
		p.Tags = []string{}
	}
	if p.Keywords == nil {
		p.Keywords = []string{}
	}
	ensureProfileHumanizeSeed(&p)
	ensureProfilePersonaID(&p)
	return &p, nil
}
