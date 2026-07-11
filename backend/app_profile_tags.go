package backend

import "strings"

func profileHasTag(profile *BrowserProfile, tag string) bool {
	if profile == nil || tag == "" {
		return false
	}
	target := strings.ToLower(strings.TrimSpace(tag))
	for _, item := range profile.Tags {
		if strings.ToLower(strings.TrimSpace(item)) == target {
			return true
		}
	}
	return false
}
