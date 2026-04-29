package browser

import (
	"fmt"
	"os"
	"path/filepath"
	goruntime "runtime"
	"strings"
	"sync"
)

type profileIdentityBinding struct {
	profileId            string
	profile              *Profile
	canonicalUserDataDir string
}

var profileStartLocks = struct {
	sync.Mutex
	active map[*Manager]map[string]struct{}
}{
	active: make(map[*Manager]map[string]struct{}),
}

func (m *Manager) AcquireProfileStartLock(profileId string) (func(), error) {
	if m == nil {
		return nil, fmt.Errorf("identity safety gate: browser manager is required")
	}
	profileId = strings.TrimSpace(profileId)
	if profileId == "" {
		return nil, fmt.Errorf("identity safety gate: profileId is required")
	}

	profileStartLocks.Lock()
	defer profileStartLocks.Unlock()

	activeByProfile := profileStartLocks.active[m]
	if activeByProfile == nil {
		activeByProfile = make(map[string]struct{})
		profileStartLocks.active[m] = activeByProfile
	}
	if _, exists := activeByProfile[profileId]; exists {
		return nil, fmt.Errorf("identity safety gate: profile %s start is already in progress", profileId)
	}
	activeByProfile[profileId] = struct{}{}

	return func() {
		profileStartLocks.Lock()
		if activeByProfile := profileStartLocks.active[m]; activeByProfile != nil {
			delete(activeByProfile, profileId)
			if len(activeByProfile) == 0 {
				delete(profileStartLocks.active, m)
			}
		}
		profileStartLocks.Unlock()
	}, nil
}

func (m *Manager) ResolveCanonicalUserDataDir(profile *Profile) (string, error) {
	if profile == nil {
		return "", fmt.Errorf("identity safety gate: profile is required")
	}

	profileId := strings.TrimSpace(profile.ProfileId)
	if profileId == "" {
		return "", fmt.Errorf("identity safety gate: profileId is required")
	}

	raw := strings.TrimSpace(profile.UserDataDir)
	if raw == "" {
		raw = profileId
	}
	if hasParentPathTraversal(raw) {
		return "", fmt.Errorf("identity safety gate: user-data-dir path traversal is forbidden for profile %s", profileId)
	}
	if hasAmbiguousRootedRelativePath(raw) {
		return "", fmt.Errorf("identity safety gate: ambiguous user-data-dir path is forbidden for profile %s", profileId)
	}

	root, err := m.canonicalUserDataRoot()
	if err != nil {
		return "", err
	}

	candidate := raw
	if !filepath.IsAbs(candidate) {
		candidate = filepath.Join(root, candidate)
	}
	candidateAbs, err := filepath.Abs(candidate)
	if err != nil {
		return "", fmt.Errorf("identity safety gate: resolve user-data-dir for profile %s: %w", profileId, err)
	}
	candidateAbs = filepath.Clean(candidateAbs)

	if !pathInsideRoot(root, candidateAbs) {
		return "", fmt.Errorf("identity safety gate: canonical user-data-dir escapes allowed root for profile %s", profileId)
	}
	if sameCanonicalPath(root, candidateAbs) {
		return "", fmt.Errorf("identity safety gate: profile %s cannot use the user-data root as its profile directory", profileId)
	}
	if err := rejectSymlinkAmbiguityBetween(root, candidateAbs); err != nil {
		return "", fmt.Errorf("identity safety gate: profile %s has ambiguous user-data-dir ownership: %w", profileId, err)
	}

	return canonicalPathForStorage(candidateAbs), nil
}

func (m *Manager) NormalizeProfileIdentityBinding(profile *Profile) error {
	canonical, err := m.ResolveCanonicalUserDataDir(profile)
	if err != nil {
		return err
	}
	if err := m.ensureUniqueCanonicalUserDataDir(profile.ProfileId, canonical); err != nil {
		return err
	}
	profile.UserDataDir = canonical
	return nil
}

func (m *Manager) NormalizeProfileIdentityBindings() error {
	if len(m.Profiles) == 0 {
		return nil
	}

	seen := make(map[string]profileIdentityBinding, len(m.Profiles))
	pending := make([]profileIdentityBinding, 0, len(m.Profiles))
	for profileId, profile := range m.Profiles {
		if profile == nil {
			return fmt.Errorf("identity safety gate: profile %s is nil", profileId)
		}
		canonical, err := m.ResolveCanonicalUserDataDir(profile)
		if err != nil {
			return err
		}
		key := canonicalPathKey(canonical)
		if existing, ok := seen[key]; ok && existing.profileId != profile.ProfileId {
			return duplicateUserDataDirError(existing.profileId, profile.ProfileId, canonical)
		}
		binding := profileIdentityBinding{
			profileId:            profile.ProfileId,
			profile:              profile,
			canonicalUserDataDir: canonical,
		}
		seen[key] = binding
		pending = append(pending, binding)
	}

	for _, item := range pending {
		item.profile.UserDataDir = item.canonicalUserDataDir
	}
	return nil
}

func (m *Manager) StableCanonicalUserDataDir(existing *Profile, nextRawUserDataDir string) (string, error) {
	if existing == nil {
		return "", fmt.Errorf("identity safety gate: existing profile is required")
	}

	currentCanonical, err := m.ResolveCanonicalUserDataDir(existing)
	if err != nil {
		return "", err
	}

	candidate := *existing
	if strings.TrimSpace(nextRawUserDataDir) != "" {
		candidate.UserDataDir = nextRawUserDataDir
	}
	nextCanonical, err := m.ResolveCanonicalUserDataDir(&candidate)
	if err != nil {
		return "", err
	}
	if !sameCanonicalPath(currentCanonical, nextCanonical) {
		return "", fmt.Errorf("identity safety gate: changing user-data-dir for profile %s requires explicit confirmation", existing.ProfileId)
	}
	if err := m.ensureUniqueCanonicalUserDataDir(existing.ProfileId, nextCanonical); err != nil {
		return "", err
	}
	return nextCanonical, nil
}

func (m *Manager) ensureUniqueCanonicalUserDataDir(profileId, canonicalUserDataDir string) error {
	key := canonicalPathKey(canonicalUserDataDir)
	for existingId, existing := range m.Profiles {
		if existing == nil || existingId == profileId || existing.ProfileId == profileId {
			continue
		}
		existingCanonical, err := m.ResolveCanonicalUserDataDir(existing)
		if err != nil {
			return err
		}
		if canonicalPathKey(existingCanonical) == key {
			return duplicateUserDataDirError(existing.ProfileId, profileId, canonicalUserDataDir)
		}
	}
	return nil
}

func (m *Manager) canonicalUserDataRoot() (string, error) {
	root := "data"
	if m != nil && m.Config != nil {
		if configured := strings.TrimSpace(m.Config.Browser.UserDataRoot); configured != "" {
			root = configured
		}
	}
	if hasParentPathTraversal(root) {
		return "", fmt.Errorf("identity safety gate: user-data root path traversal is forbidden")
	}
	if hasAmbiguousRootedRelativePath(root) {
		return "", fmt.Errorf("identity safety gate: ambiguous user-data root path is forbidden")
	}
	if m != nil {
		root = m.ResolveRelativePath(root)
	}
	rootAbs, err := filepath.Abs(root)
	if err != nil {
		return "", fmt.Errorf("identity safety gate: resolve user-data root: %w", err)
	}
	rootAbs = filepath.Clean(rootAbs)
	if err := rejectExistingPathSymlinkAmbiguity(rootAbs); err != nil {
		return "", fmt.Errorf("identity safety gate: ambiguous user-data root ownership: %w", err)
	}
	return rootAbs, nil
}

func duplicateUserDataDirError(existingProfileId, newProfileId, canonicalUserDataDir string) error {
	return fmt.Errorf("identity safety gate: duplicate canonical user-data-dir %q for profiles %s and %s", canonicalUserDataDir, existingProfileId, newProfileId)
}

func hasParentPathTraversal(path string) bool {
	for _, segment := range strings.Split(strings.ReplaceAll(path, "\\", "/"), "/") {
		if strings.TrimSpace(segment) == ".." {
			return true
		}
	}
	return false
}

func hasAmbiguousRootedRelativePath(path string) bool {
	path = strings.TrimSpace(path)
	if path == "" || filepath.IsAbs(path) {
		return false
	}
	if filepath.VolumeName(path) != "" {
		return true
	}
	return strings.HasPrefix(path, `/`) || strings.HasPrefix(path, `\`)
}

func pathInsideRoot(root, target string) bool {
	rootKey := strings.TrimRight(canonicalPathKey(root), `\/`)
	targetKey := canonicalPathKey(target)
	if targetKey == rootKey {
		return true
	}
	return strings.HasPrefix(targetKey, rootKey+string(os.PathSeparator))
}

func sameCanonicalPath(a, b string) bool {
	return canonicalPathKey(a) == canonicalPathKey(b)
}

func canonicalPathKey(path string) string {
	key := filepath.Clean(strings.TrimSpace(path))
	if goruntime.GOOS == "windows" {
		key = strings.ToLower(key)
	}
	return key
}

func canonicalPathForStorage(path string) string {
	path = filepath.Clean(strings.TrimSpace(path))
	if goruntime.GOOS == "windows" {
		path = strings.ToLower(path)
	}
	return path
}

func rejectSymlinkAmbiguityBetween(root, target string) error {
	root = filepath.Clean(root)
	target = filepath.Clean(target)
	if err := rejectExistingPathSymlinkAmbiguity(root); err != nil {
		return err
	}
	rel, err := filepath.Rel(root, target)
	if err != nil {
		return err
	}
	if rel == "." {
		return nil
	}

	current := root
	for _, part := range strings.Split(filepath.ToSlash(rel), "/") {
		part = strings.TrimSpace(part)
		if part == "" || part == "." {
			continue
		}
		current = filepath.Join(current, part)
		if err := rejectExistingPathSymlinkAmbiguity(current); err != nil {
			return err
		}
	}
	return nil
}

func rejectExistingPathSymlinkAmbiguity(path string) error {
	info, err := os.Lstat(path)
	if err != nil {
		if os.IsNotExist(err) {
			return nil
		}
		return err
	}
	if info.Mode()&os.ModeSymlink != 0 {
		return fmt.Errorf("%s is a symlink or junction", path)
	}
	resolved, err := filepath.EvalSymlinks(path)
	if err != nil {
		if os.IsNotExist(err) {
			return nil
		}
		return err
	}
	if !sameCanonicalPath(resolved, path) {
		return fmt.Errorf("%s resolves outside its literal path", path)
	}
	return nil
}
