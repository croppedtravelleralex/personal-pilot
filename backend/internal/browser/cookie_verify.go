package browser

import (
	"errors"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"strings"
)

const (
	CookieIssueSeverityWarn  = "warn"
	CookieIssueSeverityBlock = "block"
)

type CookieVerificationIssue struct {
	Severity string `json:"severity"`
	Code     string `json:"code"`
	Message  string `json:"message"`
	Path     string `json:"path,omitempty"`
}

type CookieVerificationResult struct {
	ProfileId   string                    `json:"profileId"`
	UserDataDir string                    `json:"userDataDir"`
	CookiePaths []string                  `json:"cookiePaths"`
	Warnings    []CookieVerificationIssue `json:"warnings"`
	Blockers    []CookieVerificationIssue `json:"blockers"`
}

func VerifyCookieAssetReadOnly(profileId string, userDataDir string) CookieVerificationResult {
	result := CookieVerificationResult{
		ProfileId:   strings.TrimSpace(profileId),
		UserDataDir: strings.TrimSpace(userDataDir),
	}
	if result.UserDataDir == "" {
		result.Blockers = append(result.Blockers, cookieIssue(CookieIssueSeverityBlock, "cookie_path_empty", "cookie verification target user-data-dir is empty", ""))
		return result
	}

	root, err := filepath.Abs(result.UserDataDir)
	if err != nil {
		result.Blockers = append(result.Blockers, cookieIssue(CookieIssueSeverityBlock, "cookie_path_invalid", fmt.Sprintf("cookie verification target path is invalid: %v", err), result.UserDataDir))
		return result
	}
	result.UserDataDir = filepath.Clean(root)

	candidates := []string{
		filepath.Join(result.UserDataDir, "Default", "Network", "Cookies"),
		filepath.Join(result.UserDataDir, "Default", "Cookies"),
	}
	found := false
	for _, path := range candidates {
		cleanPath := filepath.Clean(path)
		if !pathInside(cleanPath, result.UserDataDir) {
			result.Blockers = append(result.Blockers, cookieIssue(CookieIssueSeverityBlock, "cookie_path_mismatch", "cookie DB path escapes profile user-data-dir", cleanPath))
			continue
		}

		info, statErr := os.Lstat(cleanPath)
		if statErr != nil {
			if errors.Is(statErr, os.ErrNotExist) {
				continue
			}
			result.Warnings = append(result.Warnings, cookieIssue(CookieIssueSeverityWarn, "cookie_stat_failed", statErr.Error(), cleanPath))
			continue
		}
		if info.Mode()&os.ModeSymlink != 0 {
			result.Blockers = append(result.Blockers, cookieIssue(CookieIssueSeverityBlock, "cookie_path_symlink", "cookie DB path is a symlink or reparse-style indirection", cleanPath))
			continue
		}
		if info.IsDir() {
			result.Warnings = append(result.Warnings, cookieIssue(CookieIssueSeverityWarn, "cookie_db_is_dir", "cookie DB path is a directory", cleanPath))
			continue
		}

		found = true
		result.CookiePaths = append(result.CookiePaths, cleanPath)
		if err := readCookieFileProbe(cleanPath); err != nil {
			result.Warnings = append(result.Warnings, cookieIssue(CookieIssueSeverityWarn, "cookie_read_failed", err.Error(), cleanPath))
		}
	}
	if !found && len(result.Blockers) == 0 {
		result.Warnings = append(result.Warnings, cookieIssue(CookieIssueSeverityWarn, "cookie_db_missing", "cookie DB is missing; no automatic repair will be attempted", result.UserDataDir))
	}
	return result
}

func readCookieFileProbe(path string) error {
	f, err := os.Open(path)
	if err != nil {
		return err
	}
	defer f.Close()

	buf := make([]byte, 1)
	_, err = f.Read(buf)
	if err != nil && !errors.Is(err, io.EOF) {
		return err
	}
	return nil
}

func cookieIssue(severity, code, message, path string) CookieVerificationIssue {
	return CookieVerificationIssue{
		Severity: strings.TrimSpace(severity),
		Code:     strings.TrimSpace(code),
		Message:  strings.TrimSpace(message),
		Path:     strings.TrimSpace(path),
	}
}

func pathInside(path, root string) bool {
	rel, err := filepath.Rel(root, path)
	if err != nil {
		return false
	}
	return rel == "." || (!strings.HasPrefix(rel, ".."+string(filepath.Separator)) && rel != "..")
}
