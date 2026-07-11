package browser

import (
	"crypto/sha256"
	"encoding/hex"
	"fmt"
	"os/exec"
	"personal-pilot/backend/internal/logger"
	"sort"
	"strings"
	"time"

	"github.com/google/uuid"
)

// KnownIneffectiveFingerprintFlags lists flags that fingerprint-chromium silently ignores
// because it uses a seed-based spoofing model rather than individual override flags.
var KnownIneffectiveFingerprintFlags = []string{
	"--fingerprint-color-depth",
	"--fingerprint-device-memory",
	"--fingerprint-canvas-noise",
	"--fingerprint-audio-noise",
	"--fingerprint-fonts",
	"--fingerprint-do-not-track",
	"--fingerprint-media-devices",
	"--fingerprint-touch-points",
	"--fingerprint-webgl-vendor",
	"--fingerprint-webgl-renderer",
}

// ValidateFingerprintArgs checks fingerprint args for known-ineffective flags and
// returns warnings. It does not block or modify -- these are informational only.
func ValidateFingerprintArgs(args []string) []string {
	var warnings []string
	ineffectiveSet := make(map[string]bool, len(KnownIneffectiveFingerprintFlags))
	for _, f := range KnownIneffectiveFingerprintFlags {
		ineffectiveSet[f] = true
	}

	for _, arg := range args {
		eqIdx := strings.Index(arg, "=")
		if eqIdx == -1 {
			continue
		}
		key := arg[:eqIdx]
		if ineffectiveSet[key] {
			warnings = append(warnings, fmt.Sprintf(
				"fingerprint flag %s has no effect in fingerprint-chromium (seed-based model)",
				key,
			))
		}
	}
	return warnings
}

// DropIneffectiveFingerprintFlags removes args whose key (before "=") is listed in
// KnownIneffectiveFingerprintFlags. ValidateFingerprintArgs remains for warnings on
// profile storage paths; materialization should call this to strip dead flags.
func DropIneffectiveFingerprintFlags(args []string) (kept []string, dropped []string) {
	ineffectiveSet := make(map[string]bool, len(KnownIneffectiveFingerprintFlags))
	for _, f := range KnownIneffectiveFingerprintFlags {
		ineffectiveSet[f] = true
	}
	for _, arg := range args {
		eqIdx := strings.Index(arg, "=")
		if eqIdx == -1 {
			kept = append(kept, arg)
			continue
		}
		key := arg[:eqIdx]
		if ineffectiveSet[key] {
			dropped = append(dropped, arg)
			continue
		}
		kept = append(kept, arg)
	}
	return kept, dropped
}

// InitData 初始化浏览器数据
const humanizeSeedSalt = "personal-pilot-humanize-seed-v1:"

func generateHumanizeSeed(profileId string) string {
	id := strings.TrimSpace(profileId)
	if id == "" {
		id = uuid.NewString()
	}
	sum := sha256.Sum256([]byte(humanizeSeedSalt + id))
	return hex.EncodeToString(sum[:])
}

func ensureProfileHumanizeSeed(profile *Profile) bool {
	if profile == nil {
		return false
	}
	seed := strings.TrimSpace(profile.HumanizeSeed)
	if seed != "" {
		if seed != profile.HumanizeSeed {
			profile.HumanizeSeed = seed
			return true
		}
		return false
	}
	profile.HumanizeSeed = generateHumanizeSeed(profile.ProfileId)
	return true
}

// InitData initializes browser data.
func (m *Manager) InitData() {
	m.Mutex.Lock()
	defer m.Mutex.Unlock()
	if m.Profiles == nil {
		m.Profiles = make(map[string]*Profile)
	}
	if m.BrowserProcesses == nil {
		m.BrowserProcesses = make(map[string]*exec.Cmd)
	}
	if m.XrayBridges == nil {
		m.XrayBridges = make(map[string]*XrayBridge)
	}
	// 执行配置迁移
	m.MigrateConfig()
	if len(m.Profiles) > 0 {
		return
	}
	m.loadProfiles()
}

func (m *Manager) loadProfiles() {
	log := logger.New("Browser")

	// 优先从 DAO（SQLite）加载
	if m.ProfileDAO != nil {
		profiles, err := m.ProfileDAO.List()
		if err != nil {
			log.Error("从数据库加载实例配置失败", logger.F("error", err))
		} else {
			// SQLite 模式：无论是否为空都直接使用，不自动创建默认实例
			for _, p := range profiles {
				p.CoreId = normalizeProfileCoreID(p.CoreId)
				ensureProfileHumanizeSeed(p)
				m.Profiles[p.ProfileId] = p
			}
			if len(profiles) > 0 {
				log.Info("实例配置从数据库加载完成", logger.F("count", len(profiles)))
			} else {
				log.Info("实例表为空，用户可手动创建新实例")
			}
			return
		}
	}

	// 降级：从 config.yaml 加载（仅在无 SQLite 时使用）
	if len(m.Config.Browser.Profiles) == 0 {
		// 不自动创建默认实例，保持空列表
		log.Info("实例配置为空，用户可手动创建新实例")
		return
	}
	now := time.Now().Format(time.RFC3339)
	for _, item := range m.Config.Browser.Profiles {
		profileId := strings.TrimSpace(item.ProfileId)
		if profileId == "" {
			continue
		}
		createdAt := strings.TrimSpace(item.CreatedAt)
		if createdAt == "" {
			createdAt = now
		}
		updatedAt := strings.TrimSpace(item.UpdatedAt)
		if updatedAt == "" {
			updatedAt = createdAt
		}
		profile := &Profile{
			ProfileId:          profileId,
			ProfileName:        item.ProfileName,
			UserDataDir:        item.UserDataDir,
			CoreId:             normalizeProfileCoreID(item.CoreId),
			FingerprintArgs:    append([]string{}, item.FingerprintArgs...),
			ProxyId:            item.ProxyId,
			ProxyConfig:        item.ProxyConfig,
			ProxyBindSourceID:  item.ProxyBindSourceID,
			ProxyBindSourceURL: item.ProxyBindSourceURL,
			ProxyBindName:      item.ProxyBindName,
			ProxyBindUpdatedAt: item.ProxyBindUpdatedAt,
			LaunchArgs:         append([]string{}, item.LaunchArgs...),
			Tags:               append([]string{}, item.Tags...),
			Keywords:           append([]string{}, item.Keywords...),
			BehaviorProfileID:  item.BehaviorProfileID,
			HumanizeSeed:       item.HumanizeSeed,
			Running:            false,
			DebugPort:          0,
			Pid:                0,
			LastError:          "",
			CreatedAt:          createdAt,
			UpdatedAt:          updatedAt,
		}
		ensureProfileHumanizeSeed(profile)
		m.Profiles[profileId] = profile
	}
	log.Info("浏览器配置从文件加载完成", logger.F("count", len(m.Profiles)))
}

// SaveProfiles 保存所有实例配置（DAO 模式：逐条 upsert）
func (m *Manager) SaveProfiles() error {
	log := logger.New("Browser")
	if err := m.NormalizeProfileIdentityBindings(); err != nil {
		return err
	}
	if m.ProfileDAO != nil {
		for _, profile := range m.Profiles {
			ensureProfileHumanizeSeed(profile)
			profile.CoreId = normalizeProfileCoreID(profile.CoreId)
			if err := m.ProfileDAO.Upsert(profile); err != nil {
				log.Error("实例配置持久化失败", logger.F("profile_id", profile.ProfileId), logger.F("error", err))
				return err
			}
		}
		log.Info("实例配置持久化成功", logger.F("count", len(m.Profiles)))
		return nil
	}

	// 降级：写回 config.yaml
	profiles := make([]ProfileConfig, 0, len(m.Profiles))
	for _, profile := range m.Profiles {
		ensureProfileHumanizeSeed(profile)
		profiles = append(profiles, ProfileConfig{
			ProfileId:          profile.ProfileId,
			ProfileName:        profile.ProfileName,
			UserDataDir:        profile.UserDataDir,
			CoreId:             normalizeProfileCoreID(profile.CoreId),
			FingerprintArgs:    append([]string{}, profile.FingerprintArgs...),
			ProxyId:            profile.ProxyId,
			ProxyConfig:        profile.ProxyConfig,
			ProxyBindSourceID:  profile.ProxyBindSourceID,
			ProxyBindSourceURL: profile.ProxyBindSourceURL,
			ProxyBindName:      profile.ProxyBindName,
			ProxyBindUpdatedAt: profile.ProxyBindUpdatedAt,
			LaunchArgs:         append([]string{}, profile.LaunchArgs...),
			Tags:               append([]string{}, profile.Tags...),
			Keywords:           append([]string{}, profile.Keywords...),
			BehaviorProfileID:  profile.BehaviorProfileID,
			HumanizeSeed:       profile.HumanizeSeed,
			CreatedAt:          profile.CreatedAt,
			UpdatedAt:          profile.UpdatedAt,
		})
	}
	m.Config.Browser.Profiles = profiles
	if err := m.Config.Save(m.ResolveRelativePath("config.yaml")); err != nil {
		log.Error("浏览器配置持久化失败", logger.F("error", err))
		return err
	}
	log.Info("浏览器配置持久化成功（文件）", logger.F("count", len(profiles)))
	return nil
}

// List 获取配置列表
func (m *Manager) List() []Profile {
	log := logger.New("Browser")
	m.InitData()
	m.Mutex.Lock()
	defer m.Mutex.Unlock()
	list := make([]Profile, 0, len(m.Profiles))
	for _, profile := range m.Profiles {
		p := *profile
		if m.CodeProvider != nil {
			if code, err := m.CodeProvider.EnsureCode(p.ProfileId); err == nil {
				p.LaunchCode = code
			}
		}
		list = append(list, p)
	}
	// 按 ProfileId 排序，保持稳定顺序
	sort.Slice(list, func(i, j int) bool {
		return list[i].ProfileId < list[j].ProfileId
	})
	log.Info("浏览器配置列表查询", logger.F("count", len(list)))
	return list
}

// ListByTag 按标签筛选配置列表
func (m *Manager) ListByTag(tag string) []Profile {
	tag = strings.TrimSpace(tag)
	all := m.List()
	if tag == "" {
		return all
	}
	result := make([]Profile, 0)
	for _, p := range all {
		for _, t := range p.Tags {
			if strings.EqualFold(t, tag) {
				result = append(result, p)
				break
			}
		}
	}
	return result
}

// GetAllTags 获取所有已使用的标签（去重排序）
func (m *Manager) GetAllTags() []string {
	m.InitData()
	m.Mutex.Lock()
	defer m.Mutex.Unlock()
	seen := make(map[string]struct{})
	for _, p := range m.Profiles {
		for _, t := range p.Tags {
			t = strings.TrimSpace(t)
			if t != "" {
				seen[t] = struct{}{}
			}
		}
	}
	tags := make([]string, 0, len(seen))
	for t := range seen {
		tags = append(tags, t)
	}
	sort.Strings(tags)
	return tags
}

// Create 创建配置
func (m *Manager) Create(input ProfileInput) (*Profile, error) {
	log := logger.New("Browser")
	m.InitData()
	m.Mutex.Lock()
	defer m.Mutex.Unlock()
	limitEnforced := false // Local edition: profile quota is not enforced.

	// Check Profile Limit
	if limitEnforced && m.Config.App.MaxProfileLimit > 0 && len(m.Profiles) >= m.Config.App.MaxProfileLimit {
		return nil, fmt.Errorf("实例数量已达上限 (%d个)，无法创建新的实例。请兑换额度后重试！", m.Config.App.MaxProfileLimit)
	}

	now := time.Now().Format(time.RFC3339)
	profileId := uuid.NewString()
	userDataDir := strings.TrimSpace(input.UserDataDir)
	if userDataDir == "" {
		userDataDir = profileId
	}
	proxyConfig := strings.TrimSpace(input.ProxyConfig)
	proxyId := strings.TrimSpace(input.ProxyId)
	selectedProxy := Proxy{}
	hasSelectedProxy := false
	if proxyId != "" {
		if proxyItem, ok := m.GetProxyByID(proxyId); ok {
			proxyConfig = strings.TrimSpace(proxyItem.ProxyConfig)
			selectedProxy = proxyItem
			hasSelectedProxy = true
		} else {
			log.Error("代理绑定失败", logger.F("profile_id", profileId), logger.F("proxy_id", proxyId))
		}
	}
	coreId := normalizeProfileCoreID(input.CoreId)
	if coreId == "" {
		if defaultCore, ok := m.GetDefaultCore(); ok {
			coreId = defaultCore.CoreId
		}
	}
	if proxyConfig == "" && m.Config.Browser.DefaultProxy != "" {
		proxyConfig = m.Config.Browser.DefaultProxy
	}
	profile := &Profile{
		ProfileId:         profileId,
		ProfileName:       input.ProfileName,
		UserDataDir:       userDataDir,
		CoreId:            coreId,
		FingerprintArgs:   input.FingerprintArgs,
		ProxyId:           proxyId,
		ProxyConfig:       proxyConfig,
		LaunchArgs:        input.LaunchArgs,
		Tags:              input.Tags,
		Keywords:          append([]string{}, input.Keywords...),
		GroupId:           strings.TrimSpace(input.GroupId),
		BehaviorProfileID: input.BehaviorProfileID,
		HumanizeSeed:      strings.TrimSpace(input.HumanizeSeed),
		Running:           false,
		DebugPort:         0,
		Pid:               0,
		LastError:         "",
		CreatedAt:         now,
		UpdatedAt:         now,
	}
	if hasSelectedProxy {
		_ = BindProfileToProxy(profile, selectedProxy, true)
	}
	if err := m.NormalizeProfileIdentityBinding(profile); err != nil {
		return nil, err
	}
	ensureProfileHumanizeSeed(profile)
	ensureProfilePersonaID(profile)
	m.Profiles[profileId] = profile
	if warnings := ValidateFingerprintArgs(profile.FingerprintArgs); len(warnings) > 0 {
		log.Warn("实例包含无效指纹标志",
			logger.F("profile_id", profileId),
			logger.F("warnings", strings.Join(warnings, "; ")),
		)
	}
	log.Info("浏览器配置创建", logger.F("profile_id", profileId), logger.F("profile_name", input.ProfileName))
	if err := m.SaveProfiles(); err != nil {
		return nil, err
	}
	if m.CodeProvider != nil {
		if code, err := m.CodeProvider.EnsureCode(profile.ProfileId); err == nil {
			profile.LaunchCode = code
		}
	}
	return profile, nil
}

// Update 更新配置
func (m *Manager) Update(profileId string, input ProfileInput) (*Profile, error) {
	log := logger.New("Browser")
	m.InitData()
	m.Mutex.Lock()
	defer m.Mutex.Unlock()
	profile, exists := m.Profiles[profileId]
	if !exists {
		log.Error("浏览器配置不存在", logger.F("profile_id", profileId))
		return nil, fmt.Errorf("profile not found")
	}
	canonicalUserDataDir, err := m.StableCanonicalUserDataDir(profile, input.UserDataDir)
	if err != nil {
		return nil, err
	}

	profile.ProfileName = input.ProfileName
	profile.UserDataDir = canonicalUserDataDir
	profile.CoreId = normalizeProfileCoreID(input.CoreId)
	profile.FingerprintArgs = input.FingerprintArgs
	if warnings := ValidateFingerprintArgs(profile.FingerprintArgs); len(warnings) > 0 {
		log.Warn("实例包含无效指纹标志",
			logger.F("profile_id", profileId),
			logger.F("warnings", strings.Join(warnings, "; ")),
		)
	}
	profile.ProxyId = strings.TrimSpace(input.ProxyId)
	if profile.ProxyId != "" {
		if proxyItem, ok := m.GetProxyByID(profile.ProxyId); ok {
			_ = BindProfileToProxy(profile, proxyItem, true)
		} else {
			log.Error("代理绑定失败", logger.F("profile_id", profileId), logger.F("proxy_id", profile.ProxyId))
		}
	} else {
		profile.ProxyConfig = input.ProxyConfig
		_ = ClearProfileProxyBinding(profile)
	}
	profile.LaunchArgs = input.LaunchArgs
	profile.Tags = input.Tags
	profile.Keywords = append([]string{}, input.Keywords...)
	profile.GroupId = strings.TrimSpace(input.GroupId)
	profile.BehaviorProfileID = input.BehaviorProfileID
	if seed := strings.TrimSpace(input.HumanizeSeed); seed != "" {
		profile.HumanizeSeed = seed
	}
	ensureProfileHumanizeSeed(profile)
	profile.UpdatedAt = time.Now().Format(time.RFC3339)
	log.Info("浏览器配置更新", logger.F("profile_id", profileId), logger.F("profile_name", input.ProfileName))
	if err := m.SaveProfiles(); err != nil {
		return nil, err
	}
	return profile, nil
}

// Delete 删除配置
func (m *Manager) Delete(profileId string) error {
	log := logger.New("Browser")
	m.InitData()
	m.Mutex.Lock()
	defer m.Mutex.Unlock()
	if _, exists := m.Profiles[profileId]; !exists {
		log.Error("浏览器配置不存在", logger.F("profile_id", profileId))
		return fmt.Errorf("profile not found")
	}
	delete(m.Profiles, profileId)
	log.Info("浏览器配置删除", logger.F("profile_id", profileId))

	// DAO 删除
	if m.ProfileDAO != nil {
		if err := m.ProfileDAO.Delete(profileId); err != nil {
			log.Error("数据库删除实例失败", logger.F("profile_id", profileId), logger.F("error", err))
			return err
		}
	} else {
		if err := m.SaveProfiles(); err != nil {
			return err
		}
	}

	if m.CodeProvider != nil {
		_ = m.CodeProvider.Remove(profileId)
	}
	return nil
}

// ApplyDefaults 应用默认配置
func (m *Manager) ApplyDefaults(profile *Profile) bool {
	log := logger.New("Browser")
	if profile.FingerprintArgs == nil || len(profile.FingerprintArgs) == 0 {
		profile.FingerprintArgs = append([]string{}, m.Config.Browser.DefaultFingerprintArgs...)
	}
	if profile.LaunchArgs == nil || len(profile.LaunchArgs) == 0 {
		profile.LaunchArgs = append([]string{}, m.Config.Browser.DefaultLaunchArgs...)
	}
	if strings.TrimSpace(profile.UserDataDir) == "" {
		profile.UserDataDir = profile.ProfileId
	}
	ensureProfileHumanizeSeed(profile)
	profile.CoreId = normalizeProfileCoreID(profile.CoreId)
	if profile.CoreId == "" {
		if defaultCore, ok := m.GetDefaultCore(); ok {
			profile.CoreId = defaultCore.CoreId
		}
	}
	proxyChanged := false
	bindChanged, boundInPool, bindMode := m.ResolveProfileProxyBinding(profile)
	if bindChanged {
		proxyChanged = true
	}
	if bindMode != "" && bindMode != "proxy_id" {
		log.Info("实例代理自动重关联",
			logger.F("profile_id", profile.ProfileId),
			logger.F("proxy_id", profile.ProxyId),
			logger.F("mode", bindMode),
		)
	}
	if profile.ProxyId != "" && !boundInPool {
		if strings.TrimSpace(profile.ProxyConfig) == "" {
			log.Error("实例代理未找到", logger.F("profile_id", profile.ProfileId), logger.F("proxy_id", profile.ProxyId))
		} else {
			log.Warn("实例代理未找到，回退使用历史代理配置", logger.F("profile_id", profile.ProfileId), logger.F("proxy_id", profile.ProxyId))
		}
	}
	if profile.ProxyConfig == "" && m.Config.Browser.DefaultProxy != "" {
		profile.ProxyConfig = m.Config.Browser.DefaultProxy
		proxyChanged = true
	}
	return proxyChanged
}

// Copy 复制实例配置（除指纹参数外全部复制，指纹使用默认值生成新种子）
func (m *Manager) Copy(profileId string, newName string) (*Profile, error) {
	log := logger.New("Browser")
	m.InitData()
	m.Mutex.Lock()
	defer m.Mutex.Unlock()
	limitEnforced := false // Local edition: profile quota is not enforced.

	// Check Profile Limit
	if limitEnforced && m.Config.App.MaxProfileLimit > 0 && len(m.Profiles) >= m.Config.App.MaxProfileLimit {
		log.Error("复制实例失败: 达到数量上限", logger.F("limit", m.Config.App.MaxProfileLimit))
		return nil, fmt.Errorf("实例数量已达上限 (%d个)，无法复制实例。请兑换额度后重试！", m.Config.App.MaxProfileLimit)
	}

	src, exists := m.Profiles[profileId]
	if !exists {
		log.Error("源实例不存在", logger.F("profile_id", profileId))
		return nil, fmt.Errorf("profile not found")
	}

	ensureProfileHumanizeSeed(src)
	now := time.Now().Format(time.RFC3339)
	newId := uuid.NewString()

	// 处理名称
	profileName := strings.TrimSpace(newName)
	if profileName == "" {
		profileName = src.ProfileName + " (副本)"
	}

	// 复制配置，指纹参数使用默认值（新种子）
	profile := &Profile{
		ProfileId:          newId,
		ProfileName:        profileName,
		UserDataDir:        newId, // 新的用户数据目录
		CoreId:             normalizeProfileCoreID(src.CoreId),
		FingerprintArgs:    append([]string{}, m.Config.Browser.DefaultFingerprintArgs...), // 使用默认指纹（新种子）
		ProxyId:            src.ProxyId,
		ProxyConfig:        src.ProxyConfig,
		ProxyBindSourceID:  src.ProxyBindSourceID,
		ProxyBindSourceURL: src.ProxyBindSourceURL,
		ProxyBindName:      src.ProxyBindName,
		ProxyBindUpdatedAt: src.ProxyBindUpdatedAt,
		LaunchArgs:         append([]string{}, src.LaunchArgs...),
		Tags:               append([]string{}, src.Tags...),
		Keywords:           append([]string{}, src.Keywords...),
		GroupId:            src.GroupId, // 复制分组
		HumanizeSeed:       src.HumanizeSeed,
		Running:            false,
		DebugPort:          0,
		Pid:                0,
		LastError:          "",
		CreatedAt:          now,
		UpdatedAt:          now,
	}

	m.Profiles[newId] = profile
	log.Info("实例复制成功", logger.F("src_id", profileId), logger.F("new_id", newId), logger.F("new_name", profileName))

	if err := m.SaveProfiles(); err != nil {
		return nil, err
	}

	if m.CodeProvider != nil {
		if code, err := m.CodeProvider.EnsureCode(profile.ProfileId); err == nil {
			profile.LaunchCode = code
		}
	}

	return profile, nil
}

// SetKeywords 设置实例关键字（独立接口，不影响其他字段）
func (m *Manager) SetKeywords(profileId string, keywords []string) (*Profile, error) {
	log := logger.New("Browser")
	m.InitData()
	m.Mutex.Lock()
	defer m.Mutex.Unlock()
	profile, exists := m.Profiles[profileId]
	if !exists {
		return nil, fmt.Errorf("profile not found")
	}
	profile.Keywords = append([]string{}, keywords...)
	profile.UpdatedAt = time.Now().Format(time.RFC3339)
	log.Info("关键字更新", logger.F("profile_id", profileId))
	if err := m.SaveProfiles(); err != nil {
		return nil, err
	}
	return profile, nil
}

// copyKeywords 深拷贝 keywords map
func copyKeywords(src map[string]string) map[string]string {
	if src == nil {
		return nil
	}
	dst := make(map[string]string, len(src))
	for k, v := range src {
		dst[k] = v
	}
	return dst
}
