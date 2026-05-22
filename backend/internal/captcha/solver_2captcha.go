package captcha

import (
	"context"
	"encoding/base64"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"net/url"
	"strings"
	"time"

	"personal-pilot/backend/internal/logger"
)

type TwoCaptchaSolver struct {
	apiKey  string
	client  *http.Client
	baseURL string
	log     *logger.Logger
	costs   map[CaptchaType]float64
}

func NewTwoCaptchaSolver(apiKey string, timeout time.Duration) *TwoCaptchaSolver {
	if timeout <= 0 {
		timeout = 120 * time.Second
	}
	return &TwoCaptchaSolver{
		apiKey:  apiKey,
		client:  &http.Client{Timeout: timeout},
		baseURL: "https://2captcha.com",
		log:     logger.New("captcha.2captcha"),
		costs: map[CaptchaType]float64{
			CaptchaImage:     0.002,
			CaptchaReCaptcha: 0.003,
			CaptchaTurnstile: 0.003,
		},
	}
}

func (s *TwoCaptchaSolver) Name() string { return "2captcha" }

func (s *TwoCaptchaSolver) GetBalance(ctx context.Context) (float64, error) {
	u := fmt.Sprintf("%s/res.php?key=%s&action=getbalance&json=1", s.baseURL, s.apiKey)
	req, err := http.NewRequestWithContext(ctx, http.MethodGet, u, nil)
	if err != nil {
		return 0, fmt.Errorf("2captcha: create balance request: %w", err)
	}

	resp, err := s.client.Do(req)
	if err != nil {
		return 0, fmt.Errorf("2captcha: balance request failed: %w", err)
	}
	defer resp.Body.Close()

	raw, _ := io.ReadAll(resp.Body)

	var balanceResp struct {
		Status  int     `json:"status"`
		Request float64 `json:"request,string"`
	}
	if err := json.Unmarshal(raw, &balanceResp); err != nil {
		return 0, fmt.Errorf("2captcha: decode balance response: %w", err)
	}

	if balanceResp.Status != 1 {
		return 0, fmt.Errorf("2captcha: balance error (status=%d)", balanceResp.Status)
	}
	return balanceResp.Request, nil
}

func (s *TwoCaptchaSolver) Solve(ctx context.Context, req *SolveRequest) (*SolveResult, error) {
	form := url.Values{}
	form.Set("key", s.apiKey)
	form.Set("json", "1")

	switch req.Type {
	case CaptchaImage:
		b64 := ""
		if len(req.ImageData) > 0 {
			b64 = base64.StdEncoding.EncodeToString(req.ImageData)
		} else {
			return nil, fmt.Errorf("2captcha: image data required for normal captcha")
		}
		form.Set("method", "base64")
		form.Set("body", b64)

	case CaptchaReCaptcha:
		form.Set("method", "userrecaptcha")
		form.Set("googlekey", req.SiteKey)
		form.Set("pageurl", req.PageURL)
		if req.Proxy != "" {
			form.Set("proxy", req.Proxy)
		}

	case CaptchaTurnstile:
		form.Set("method", "turnstile")
		form.Set("sitekey", req.SiteKey)
		form.Set("pageurl", req.PageURL)
		if req.Proxy != "" {
			form.Set("proxy", req.Proxy)
		}

	default:
		return nil, fmt.Errorf("2captcha: unsupported captcha type: %s", req.Type)
	}

	id, err := s.submitCaptcha(ctx, form)
	if err != nil {
		return nil, err
	}

	return s.pollResult(ctx, id, req.Type)
}

func (s *TwoCaptchaSolver) submitCaptcha(ctx context.Context, form url.Values) (string, error) {
	req, err := http.NewRequestWithContext(ctx, http.MethodPost, s.baseURL+"/in.php", strings.NewReader(form.Encode()))
	if err != nil {
		return "", fmt.Errorf("2captcha: create submit request: %w", err)
	}
	req.Header.Set("Content-Type", "application/x-www-form-urlencoded")

	resp, err := s.client.Do(req)
	if err != nil {
		return "", fmt.Errorf("2captcha: submit failed: %w", err)
	}
	defer resp.Body.Close()

	raw, _ := io.ReadAll(resp.Body)

	s.log.Debug("2captcha submit response",
		logger.F("status", resp.StatusCode),
		logger.F("body", string(raw)),
	)

	var submitResp struct {
		Status  int    `json:"status"`
		Request string `json:"request"`
	}
	if err := json.Unmarshal(raw, &submitResp); err != nil {
		return "", fmt.Errorf("2captcha: decode submit response: %w", err)
	}

	if submitResp.Status != 1 {
		return "", fmt.Errorf("2captcha: submit error: %s", submitResp.Request)
	}
	if submitResp.Request == "" {
		return "", fmt.Errorf("2captcha: empty captcha ID in submit response")
	}

	return submitResp.Request, nil
}

func (s *TwoCaptchaSolver) pollResult(ctx context.Context, id string, cType CaptchaType) (*SolveResult, error) {
	pollURL := fmt.Sprintf("%s/res.php?key=%s&action=get&id=%s&json=1", s.baseURL, s.apiKey, id)

	start := time.Now()
	maxAttempts := 60

	for attempt := 0; attempt < maxAttempts; attempt++ {
		select {
		case <-ctx.Done():
			return nil, fmt.Errorf("2captcha: context cancelled while polling id %s: %w", id, ctx.Err())
		default:
		}

		req, err := http.NewRequestWithContext(ctx, http.MethodGet, pollURL, nil)
		if err != nil {
			return nil, fmt.Errorf("2captcha: create poll request: %w", err)
		}

		resp, err := s.client.Do(req)
		if err != nil {
			return nil, fmt.Errorf("2captcha: poll id %s failed: %w", id, err)
		}

		raw, _ := io.ReadAll(resp.Body)
		resp.Body.Close()

		var pollResp struct {
			Status  int    `json:"status"`
			Request string `json:"request"`
		}
		if err := json.Unmarshal(raw, &pollResp); err != nil {
			return nil, fmt.Errorf("2captcha: decode poll response: %w", err)
		}

		if pollResp.Status == 1 {
			cost := s.getCostForType(cType)
			result := &SolveResult{
				SolvedAt: time.Now(),
				Cost:     cost,
				Provider: "2captcha",
			}

			switch cType {
			case CaptchaImage:
				result.Text = pollResp.Request
			case CaptchaReCaptcha, CaptchaTurnstile:
				result.Token = pollResp.Request
			default:
				result.Token = pollResp.Request
			}

			s.log.Info("2captcha task solved",
				logger.F("id", id),
				logger.F("elapsed", time.Since(start).Seconds()),
				logger.F("cost", cost),
			)

			return result, nil
		}

		if strings.Contains(pollResp.Request, "ERROR") || strings.Contains(pollResp.Request, "error") {
			return nil, fmt.Errorf("2captcha: task %s reported error: %s", id, pollResp.Request)
		}

		select {
		case <-ctx.Done():
			return nil, fmt.Errorf("2captcha: context cancelled while waiting for id %s: %w", id, ctx.Err())
		case <-time.After(5 * time.Second):
		}
	}

	return nil, fmt.Errorf("2captcha: task %s did not complete within max polling attempts", id)
}

func (s *TwoCaptchaSolver) getCostForType(cType CaptchaType) float64 {
	if cost, ok := s.costs[cType]; ok {
		return cost
	}
	return 0.003
}

func (s *TwoCaptchaSolver) SetCost(cType CaptchaType, cost float64) {
	s.costs[cType] = cost
}
