package launchcode

import (
	"crypto/subtle"
	"net/http"
	"os"
	"strings"
)

const DefaultAPIKeyHeader = "X-Personal-Pilot-Api-Key"

// APIAuthConfig 定义 LaunchServer 对 /api/* 请求的可选认证配置。
type APIAuthConfig struct {
	Enabled bool
	APIKey  string
	Header  string
}

func normalizeAPIAuthConfig(cfg APIAuthConfig) APIAuthConfig {
	cfg.APIKey = resolveEnvOnlyAPIKey(cfg.APIKey)
	cfg.Header = strings.TrimSpace(cfg.Header)
	if cfg.Header == "" {
		cfg.Header = DefaultAPIKeyHeader
	}
	return cfg
}

func resolveEnvOnlyAPIKey(value string) string {
	value = strings.TrimSpace(value)
	if value == "" {
		return ""
	}

	if strings.HasPrefix(value, "${") && strings.HasSuffix(value, "}") {
		name := strings.TrimSpace(strings.TrimSuffix(strings.TrimPrefix(value, "${"), "}"))
		return strings.TrimSpace(os.Getenv(name))
	}
	if strings.HasPrefix(value, "$") && len(value) > 1 && !strings.ContainsAny(value[1:], "${}") {
		return strings.TrimSpace(os.Getenv(strings.TrimSpace(value[1:])))
	}
	if strings.HasPrefix(value, "%") && strings.HasSuffix(value, "%") && len(value) > 2 {
		name := strings.TrimSpace(strings.TrimSuffix(strings.TrimPrefix(value, "%"), "%"))
		return strings.TrimSpace(os.Getenv(name))
	}
	return value
}

func (cfg APIAuthConfig) Requested() bool {
	return cfg.Enabled
}

func (cfg APIAuthConfig) Configured() bool {
	return cfg.APIKey != ""
}

func (cfg APIAuthConfig) Active() bool {
	return cfg.Requested() && cfg.Configured()
}

func (s *LaunchServer) SetAPIAuthConfig(cfg APIAuthConfig) {
	s.authMu.Lock()
	s.apiAuth = normalizeAPIAuthConfig(cfg)
	s.authMu.Unlock()
}

func (s *LaunchServer) SetRateLimiter(rl *RateLimiter) {
	s.mu.Lock()
	s.rateLimiter = rl
	s.mu.Unlock()
}

func (s *LaunchServer) apiAuthConfig() APIAuthConfig {
	s.authMu.RLock()
	cfg := s.apiAuth
	s.authMu.RUnlock()
	return cfg
}

func (s *LaunchServer) APIAuthHeader() string {
	return s.apiAuthConfig().Header
}

func (s *LaunchServer) APIAuthRequested() bool {
	return s.apiAuthConfig().Requested()
}

func (s *LaunchServer) APIAuthConfigured() bool {
	return s.apiAuthConfig().Configured()
}

func (s *LaunchServer) APIAuthEnabled() bool {
	return s.apiAuthConfig().Active()
}

func (s *LaunchServer) apiAuthMiddleware(next http.Handler) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if !strings.HasPrefix(r.URL.Path, "/api/") {
			next.ServeHTTP(w, r)
			return
		}

		cfg := s.apiAuthConfig()
		if !cfg.Active() {
			next.ServeHTTP(w, r)
			return
		}

		providedKey := strings.TrimSpace(r.Header.Get(cfg.Header))
		if subtle.ConstantTimeCompare([]byte(providedKey), []byte(cfg.APIKey)) != 1 {
			writeJSON(w, http.StatusUnauthorized, map[string]interface{}{
				"ok":         false,
				"error":      "unauthorized: invalid api key",
				"authHeader": cfg.Header,
			})
			return
		}

		next.ServeHTTP(w, r)
	})
}
