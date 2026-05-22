package proxy

import (
	"net/url"
	"strings"
)

func RedactProxyURL(rawURL string) string {
	if rawURL == "" {
		return ""
	}
	u, err := url.Parse(rawURL)
	if err != nil {
		idx := strings.LastIndex(rawURL, "@")
		if idx > 0 {
			return "[REDACTED]" + rawURL[idx:]
		}
		return rawURL
	}
	if u.User != nil {
		u.User = url.User("****")
	}
	return u.String()
}
