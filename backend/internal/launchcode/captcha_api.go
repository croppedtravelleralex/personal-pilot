package launchcode

import (
	"encoding/base64"
	"net/http"
	"time"

	"personal-pilot/backend/internal/captcha"
	"personal-pilot/backend/internal/logger"
)

type captchaSolveRequest struct {
	Type      string         `json:"type"`
	ImageData string         `json:"imageData"`
	ImageURL  string         `json:"imageUrl"`
	SiteKey   string         `json:"siteKey"`
	PageURL   string         `json:"pageUrl"`
	Proxy     string         `json:"proxy"`
	UserAgent string         `json:"userAgent"`
	Options   map[string]any `json:"options"`
}

type captchaSolveTokenRequest struct {
	Type      string `json:"type"`
	SiteKey   string `json:"siteKey"`
	PageURL   string `json:"pageUrl"`
	Proxy     string `json:"proxy"`
	UserAgent string `json:"userAgent"`
}

func (s *LaunchServer) SetCaptchaManager(m *captcha.Manager) {
	s.captchaManager = m
}

func (s *LaunchServer) HandleCaptchaSolve(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		writeJSON(w, http.StatusMethodNotAllowed, map[string]interface{}{"ok": false, "error": "method not allowed"})
		return
	}
	if s.captchaManager == nil {
		writeJSON(w, http.StatusServiceUnavailable, map[string]interface{}{"ok": false, "error": "captcha service not available"})
		return
	}

	var req captchaSolveRequest
	if status, errMsg := decodeLimitedJSONBody(r, &req); errMsg != "" {
		writeJSON(w, status, map[string]interface{}{"ok": false, "error": errMsg})
		return
	}

	ct := captcha.CaptchaType(req.Type)
	if ct == "" {
		ct = captcha.CaptchaImage
	}

	var imageData []byte
	if req.ImageData != "" {
		var err error
		imageData, err = base64.StdEncoding.DecodeString(req.ImageData)
		if err != nil {
			writeJSON(w, http.StatusBadRequest, map[string]interface{}{"ok": false, "error": "invalid base64 imageData"})
			return
		}
	}

	solveReq := &captcha.SolveRequest{
		Type:      ct,
		ImageData: imageData,
		ImageURL:  req.ImageURL,
		SiteKey:   req.SiteKey,
		PageURL:   req.PageURL,
		Proxy:     req.Proxy,
		UserAgent: req.UserAgent,
		Options:   req.Options,
	}

	start := time.Now()
	result, err := s.captchaManager.Solve(r.Context(), solveReq)
	elapsed := time.Since(start).Milliseconds()

	if err != nil {
		logger.New("CaptchaAPI").Error("solve failed", logger.F("error", err.Error()))
		writeJSON(w, http.StatusInternalServerError, map[string]interface{}{
			"ok":        false,
			"error":     "captcha solve failed",
			"elapsedMs": elapsed,
		})
		return
	}
	writeJSON(w, http.StatusOK, map[string]interface{}{
		"ok": true,
		"data": map[string]interface{}{
			"text":      result.Text,
			"token":     result.Token,
			"cost":      result.Cost,
			"provider":  result.Provider,
			"elapsedMs": elapsed,
		},
	})
}

func (s *LaunchServer) HandleCaptchaSolveToken(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		writeJSON(w, http.StatusMethodNotAllowed, map[string]interface{}{"ok": false, "error": "method not allowed"})
		return
	}
	if s.captchaManager == nil {
		writeJSON(w, http.StatusServiceUnavailable, map[string]interface{}{"ok": false, "error": "captcha service not available"})
		return
	}

	var req captchaSolveTokenRequest
	if status, errMsg := decodeLimitedJSONBody(r, &req); errMsg != "" {
		writeJSON(w, status, map[string]interface{}{"ok": false, "error": errMsg})
		return
	}

	ct := captcha.CaptchaType(req.Type)
	if ct == "" {
		writeJSON(w, http.StatusBadRequest, map[string]interface{}{"ok": false, "error": "type is required (recaptcha, turnstile, hcaptcha)"})
		return
	}
	if req.SiteKey == "" {
		writeJSON(w, http.StatusBadRequest, map[string]interface{}{"ok": false, "error": "siteKey is required"})
		return
	}
	if req.PageURL == "" {
		writeJSON(w, http.StatusBadRequest, map[string]interface{}{"ok": false, "error": "pageUrl is required"})
		return
	}

	solveReq := &captcha.SolveRequest{
		Type:      ct,
		SiteKey:   req.SiteKey,
		PageURL:   req.PageURL,
		Proxy:     req.Proxy,
		UserAgent: req.UserAgent,
	}

	start := time.Now()
	result, err := s.captchaManager.Solve(r.Context(), solveReq)
	elapsed := time.Since(start).Milliseconds()

	if err != nil {
		logger.New("CaptchaAPI").Error("solve token failed", logger.F("error", err.Error()))
		writeJSON(w, http.StatusInternalServerError, map[string]interface{}{
			"ok":        false,
			"error":     "captcha solve failed",
			"elapsedMs": elapsed,
		})
		return
	}
	writeJSON(w, http.StatusOK, map[string]interface{}{
		"ok": true,
		"data": map[string]interface{}{
			"token":     result.Token,
			"cost":      result.Cost,
			"provider":  result.Provider,
			"elapsedMs": elapsed,
		},
	})
}

func (s *LaunchServer) HandleCaptchaConfig(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodGet {
		writeJSON(w, http.StatusMethodNotAllowed, map[string]interface{}{"ok": false, "error": "method not allowed"})
		return
	}
	if s.captchaManager == nil {
		writeJSON(w, http.StatusServiceUnavailable, map[string]interface{}{"ok": false, "error": "captcha service not available"})
		return
	}

	providerNames := make([]string, 0)
	for _, solver := range s.captchaManager.Solvers() {
		providerNames = append(providerNames, solver.Name())
	}

	writeJSON(w, http.StatusOK, map[string]interface{}{
		"ok": true,
		"data": map[string]interface{}{
			"providers": providerNames,
			"config":    s.captchaManager.Config(),
		},
	})
}

func (s *LaunchServer) HandleCaptchaBalance(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodGet {
		writeJSON(w, http.StatusMethodNotAllowed, map[string]interface{}{"ok": false, "error": "method not allowed"})
		return
	}
	if s.captchaManager == nil {
		writeJSON(w, http.StatusServiceUnavailable, map[string]interface{}{"ok": false, "error": "captcha service not available"})
		return
	}

	balances := make(map[string]float64)
	for _, solver := range s.captchaManager.Solvers() {
		bal, err := solver.GetBalance(r.Context())
		if err == nil {
			balances[solver.Name()] = bal
		}
	}

	total := 0.0
	for _, b := range balances {
		total += b
	}

	writeJSON(w, http.StatusOK, map[string]interface{}{
		"ok": true,
		"data": map[string]interface{}{
			"balances": balances,
			"total":    total,
		},
	})
}
