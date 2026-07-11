package browser

import "strings"

// IsNavigationErrorPage reports URLs that indicate the browser failed to load the target.
func IsNavigationErrorPage(pageURL string) bool {
	u := strings.ToLower(strings.TrimSpace(pageURL))
	if u == "" {
		return true
	}
	if strings.HasPrefix(u, "chrome-error:") ||
		strings.HasPrefix(u, "about:neterror") {
		return true
	}
	return strings.Contains(u, "chromewebdata")
}
