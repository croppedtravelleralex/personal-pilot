package captcha

import (
	"bytes"
	"context"
	"encoding/base64"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"time"

	"personal-pilot/backend/internal/logger"
)

type CapsolverSolver struct {
	apiKey  string
	client  *http.Client
	baseURL string
	log     *logger.Logger
}

func NewCapsolverSolver(apiKey string, timeout time.Duration) *CapsolverSolver {
	if timeout <= 0 {
		timeout = 120 * time.Second
	}
	return &CapsolverSolver{
		apiKey:  apiKey,
		client:  &http.Client{Timeout: timeout},
		baseURL: "https://api.capsolver.com",
		log:     logger.New("captcha.capsolver"),
	}
}

func (s *CapsolverSolver) Name() string { return "capsolver" }

func (s *CapsolverSolver) GetBalance(ctx context.Context) (float64, error) {
	payload := map[string]string{"clientKey": s.apiKey}
	body, _ := json.Marshal(payload)

	req, err := http.NewRequestWithContext(ctx, http.MethodPost, s.baseURL+"/getBalance", bytes.NewReader(body))
	if err != nil {
		return 0, fmt.Errorf("capsolver: create balance request: %w", err)
	}
	req.Header.Set("Content-Type", "application/json")

	resp, err := s.client.Do(req)
	if err != nil {
		return 0, fmt.Errorf("capsolver: balance request failed: %w", err)
	}
	defer resp.Body.Close()

	var result struct {
		ErrorId int     `json:"errorId"`
		Balance float64 `json:"balance"`
	}
	if err := json.NewDecoder(resp.Body).Decode(&result); err != nil {
		return 0, fmt.Errorf("capsolver: decode balance response: %w", err)
	}
	if result.ErrorId != 0 {
		return 0, fmt.Errorf("capsolver: balance error (errorId=%d)", result.ErrorId)
	}
	return result.Balance, nil
}

func (s *CapsolverSolver) Solve(ctx context.Context, req *SolveRequest) (*SolveResult, error) {
	task, err := s.buildTask(req)
	if err != nil {
		return nil, err
	}
	return s.solveTask(ctx, task)
}

func (s *CapsolverSolver) buildTask(req *SolveRequest) (map[string]any, error) {
	switch req.Type {
	case CaptchaImage:
		b64 := ""
		if len(req.ImageData) > 0 {
			b64 = base64.StdEncoding.EncodeToString(req.ImageData)
		} else {
			return nil, fmt.Errorf("capsolver: image data required for ImageToText")
		}
		task := map[string]any{
			"type": "ImageToTextTask",
			"body": b64,
		}
		if req.Options != nil {
			if module, ok := req.Options["module"].(string); ok {
				task["module"] = module
			}
		}
		return task, nil

	case CaptchaReCaptcha:
		task := map[string]any{
			"type":       "ReCaptchaV2Task",
			"websiteURL": req.PageURL,
			"websiteKey": req.SiteKey,
		}
		if req.Proxy != "" {
			task["proxy"] = req.Proxy
		}
		if req.UserAgent != "" {
			task["userAgent"] = req.UserAgent
		}
		return task, nil

	case CaptchaTurnstile:
		task := map[string]any{
			"type":       "AntiTurnstileTaskProxyLess",
			"websiteURL": req.PageURL,
			"websiteKey": req.SiteKey,
		}
		return task, nil

	case CaptchaHCaptcha:
		task := map[string]any{
			"type":       "HCaptchaTask",
			"websiteURL": req.PageURL,
			"websiteKey": req.SiteKey,
		}
		if req.Proxy != "" {
			task["proxy"] = req.Proxy
		}
		return task, nil

	case CaptchaGeeTest:
		gt, _ := req.Options["gt"].(string)
		challenge, _ := req.Options["challenge"].(string)
		if gt == "" || challenge == "" {
			return nil, fmt.Errorf("capsolver: gt and challenge required for GeeTest")
		}
		task := map[string]any{
			"type":      "GeeTestTask",
			"websiteURL": req.PageURL,
			"gt":        gt,
			"challenge": challenge,
		}
		return task, nil

	default:
		return nil, fmt.Errorf("capsolver: unsupported captcha type: %s", req.Type)
	}
}

func (s *CapsolverSolver) solveTask(ctx context.Context, task map[string]any) (*SolveResult, error) {
	taskId, err := s.createTask(ctx, task)
	if err != nil {
		return nil, err
	}

	return s.pollTaskResult(ctx, taskId)
}

func (s *CapsolverSolver) createTask(ctx context.Context, task map[string]any) (string, error) {
	payload := map[string]any{
		"clientKey": s.apiKey,
		"task":      task,
	}
	body, _ := json.Marshal(payload)

	req, err := http.NewRequestWithContext(ctx, http.MethodPost, s.baseURL+"/createTask", bytes.NewReader(body))
	if err != nil {
		return "", fmt.Errorf("capsolver: create request: %w", err)
	}
	req.Header.Set("Content-Type", "application/json")

	resp, err := s.client.Do(req)
	if err != nil {
		return "", fmt.Errorf("capsolver: create task failed: %w", err)
	}
	defer resp.Body.Close()

	raw, _ := io.ReadAll(resp.Body)
	s.log.Debug("createTask response",
		logger.F("status", resp.StatusCode),
		logger.F("body", string(raw)),
	)

	var createResp struct {
		ErrorId          int    `json:"errorId"`
		TaskId           string `json:"taskId"`
		ErrorDescription string `json:"errorDescription,omitempty"`
		ErrorCode        string `json:"errorCode,omitempty"`
	}

	if err := json.Unmarshal(raw, &createResp); err != nil {
		return "", fmt.Errorf("capsolver: decode createTask response: %w", err)
	}

	if createResp.ErrorId != 0 {
		return "", fmt.Errorf("capsolver: createTask error (id=%d): %s [%s]",
			createResp.ErrorId, createResp.ErrorDescription, createResp.ErrorCode)
	}
	if createResp.TaskId == "" {
		return "", fmt.Errorf("capsolver: empty taskId in createTask response")
	}

	return createResp.TaskId, nil
}

func (s *CapsolverSolver) pollTaskResult(ctx context.Context, taskId string) (*SolveResult, error) {
	baseDelay := 1 * time.Second
	maxDelay := 10 * time.Second
	maxAttempts := 30

	for attempt := 0; attempt < maxAttempts; attempt++ {
		select {
		case <-ctx.Done():
			return nil, fmt.Errorf("capsolver: context cancelled while polling task %s: %w", taskId, ctx.Err())
		default:
		}

		payload := map[string]string{
			"clientKey": s.apiKey,
			"taskId":    taskId,
		}
		body, _ := json.Marshal(payload)

		req, err := http.NewRequestWithContext(ctx, http.MethodPost, s.baseURL+"/getTaskResult", bytes.NewReader(body))
		if err != nil {
			return nil, fmt.Errorf("capsolver: create poll request: %w", err)
		}
		req.Header.Set("Content-Type", "application/json")

		resp, err := s.client.Do(req)
		if err != nil {
			return nil, fmt.Errorf("capsolver: poll task %s failed: %w", taskId, err)
		}

		raw, _ := io.ReadAll(resp.Body)
		resp.Body.Close()

		var pollResp struct {
			ErrorId          int            `json:"errorId"`
			ErrorCode        string         `json:"errorCode,omitempty"`
			ErrorDescription string         `json:"errorDescription,omitempty"`
			Status           string         `json:"status"`
			Solution         map[string]any `json:"solution,omitempty"`
		}

		if err := json.Unmarshal(raw, &pollResp); err != nil {
			return nil, fmt.Errorf("capsolver: decode poll response: %w", err)
		}

		if pollResp.ErrorId != 0 {
			return nil, fmt.Errorf("capsolver: poll error (id=%d): %s [%s]",
				pollResp.ErrorId, pollResp.ErrorDescription, pollResp.ErrorCode)
		}

		if pollResp.Status == "ready" {
			return s.parseSolution(taskId, pollResp.Solution)
		}

		delay := baseDelay << uint(attempt)
		if delay > maxDelay {
			delay = maxDelay
		}

		s.log.Debug("capsolver task still processing",
			logger.F("taskId", taskId),
			logger.F("attempt", attempt+1),
			logger.F("nextPollMs", delay.Milliseconds()),
		)

		select {
		case <-ctx.Done():
			return nil, fmt.Errorf("capsolver: context cancelled while waiting for task %s: %w", taskId, ctx.Err())
		case <-time.After(delay):
		}
	}

	return nil, fmt.Errorf("capsolver: task %s did not complete within max polling attempts", taskId)
}

func (s *CapsolverSolver) parseSolution(taskId string, solution map[string]any) (*SolveResult, error) {
	result := &SolveResult{
		SolvedAt: time.Now(),
		Provider: "capsolver",
	}

	if solution == nil {
		return nil, fmt.Errorf("capsolver: empty solution for task %s", taskId)
	}

	if text, ok := solution["text"].(string); ok {
		result.Text = text
	}

	if token, ok := solution["token"].(string); ok {
		result.Token = token
	}

	if gRecaptchaResponse, ok := solution["gRecaptchaResponse"].(string); ok {
		result.Token = gRecaptchaResponse
	}

	if cost, ok := solution["cost"].(float64); ok {
		result.Cost = cost
	}

	s.log.Info("capsolver task solved",
		logger.F("taskId", taskId),
		logger.F("cost", result.Cost),
	)

	return result, nil
}
