package sms

import (
	"context"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"strconv"
	"strings"
	"time"
)

const (
	fiveSimBaseURL = "https://5sim.net/v1"
	fiveSimMaxBody = 1 << 20
)

type FiveSimProvider struct {
	baseURL string
	apiKey  string
	client  *http.Client
}

func NewFiveSimProvider(apiKey string) *FiveSimProvider {
	return &FiveSimProvider{
		baseURL: fiveSimBaseURL,
		apiKey:  apiKey,
		client:  &http.Client{Timeout: 30 * time.Second},
	}
}

func (p *FiveSimProvider) Name() string { return "5sim" }

func (p *FiveSimProvider) BuyNumber(ctx context.Context, req *BuyRequest) (*Number, error) {
	operator := req.Operator
	if operator == "" {
		operator = "any"
	}
	path := fmt.Sprintf("%s/v1/user/buy/activation/%s/%s/%s", p.baseURL, req.Country, operator, req.Service)
	httpReq, err := http.NewRequestWithContext(ctx, http.MethodGet, path, nil)
	if err != nil {
		return nil, fmt.Errorf("5sim buy request: %w", err)
	}
	httpReq.Header.Set("Authorization", "Bearer "+p.apiKey)

	resp, err := p.client.Do(httpReq)
	if err != nil {
		return nil, fmt.Errorf("5sim buy: %w", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		body, _ := io.ReadAll(io.LimitReader(resp.Body, fiveSimMaxBody))
		if resp.StatusCode == http.StatusTooManyRequests {
			retryAfter := resp.Header.Get("Retry-After")
			return nil, fmt.Errorf("5sim rate limited (retry-after: %s): %s", retryAfter, truncateSMS(body))
		}
		return nil, fmt.Errorf("5sim buy HTTP %d: %s", resp.StatusCode, truncateSMS(body))
	}

	limited := io.LimitReader(resp.Body, fiveSimMaxBody)
	raw, err := io.ReadAll(limited)
	if err != nil {
		return nil, fmt.Errorf("5sim buy read: %w", err)
	}

	var result struct {
		ID      int    `json:"id"`
		Phone   int64  `json:"phone"`
		Price   int    `json:"price"`
		Status  string `json:"status"`
		Expires string `json:"expires"`
		Country string `json:"country"`
		Product string `json:"product"`
	}
	if err := json.Unmarshal(raw, &result); err != nil {
		return nil, fmt.Errorf("5sim buy decode: %w", err)
	}

	phone := strconv.FormatInt(result.Phone, 10)
	country := result.Country
	if country == "" {
		country = req.Country
	}
	service := result.Product
	if service == "" {
		service = req.Service
	}

	n := &Number{
		ID:      strconv.Itoa(result.ID),
		Phone:   "+" + phone,
		Country: country,
		Service: service,
		Price:   float64(result.Price) / 100,
		Status:  parseFiveSimStatus(result.Status),
	}

	if len(phone) > 0 {
		n.PhoneCC = phone[:1]
	}
	if len(phone) > 1 {
		n.PhoneNum = phone[1:]
	}

	if result.Expires != "" {
		if t, err := parseFiveSimTime(result.Expires); err == nil {
			n.ExpiresAt = t
		}
	}
	n.CreatedAt = time.Now()

	return n, nil
}

func (p *FiveSimProvider) CheckSMS(ctx context.Context, orderID string) (*SMSResult, error) {
	path := fmt.Sprintf("%s/v1/user/check/%s", p.baseURL, orderID)
	httpReq, err := http.NewRequestWithContext(ctx, http.MethodGet, path, nil)
	if err != nil {
		return nil, fmt.Errorf("5sim check request: %w", err)
	}
	httpReq.Header.Set("Authorization", "Bearer "+p.apiKey)

	resp, err := p.client.Do(httpReq)
	if err != nil {
		return nil, fmt.Errorf("5sim check: %w", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		body, _ := io.ReadAll(io.LimitReader(resp.Body, fiveSimMaxBody))
		return nil, fmt.Errorf("5sim check HTTP %d: %s", resp.StatusCode, truncateSMS(body))
	}

	limited := io.LimitReader(resp.Body, fiveSimMaxBody)
	raw, err := io.ReadAll(limited)
	if err != nil {
		return nil, fmt.Errorf("5sim check read: %w", err)
	}

	var result struct {
		Status string `json:"status"`
		SMS    []struct {
			Code   string `json:"code"`
			Text   string `json:"text"`
			Sender string `json:"sender"`
			Date   string `json:"date"`
		} `json:"sms"`
	}
	if err := json.Unmarshal(raw, &result); err != nil {
		return nil, fmt.Errorf("5sim check decode: %w", err)
	}

	smsResult := &SMSResult{
		Status: parseFiveSimStatus(result.Status),
	}

	if len(result.SMS) > 0 {
		sms := result.SMS[0]
		data := &SMSData{
			Code:   sms.Code,
			Text:   sms.Text,
			Sender: sms.Sender,
		}
		if sms.Date != "" {
			if t, err := parseFiveSimTime(sms.Date); err == nil {
				data.ReceivedAt = t
			}
		}
		smsResult.SMS = data

		if smsResult.Status == StatusPending && data.Code != "" {
			smsResult.Status = StatusReceived
		}
	}

	return smsResult, nil
}

func (p *FiveSimProvider) Cancel(ctx context.Context, orderID string) error {
	path := fmt.Sprintf("%s/v1/user/cancel/%s", p.baseURL, orderID)
	return p.fiveSimAction(ctx, path, "cancel")
}

func (p *FiveSimProvider) Finish(ctx context.Context, orderID string) error {
	path := fmt.Sprintf("%s/v1/user/finish/%s", p.baseURL, orderID)
	return p.fiveSimAction(ctx, path, "finish")
}

func (p *FiveSimProvider) GetBalance(ctx context.Context) (float64, error) {
	path := fmt.Sprintf("%s/v1/user/profile", p.baseURL)
	httpReq, err := http.NewRequestWithContext(ctx, http.MethodGet, path, nil)
	if err != nil {
		return 0, fmt.Errorf("5sim balance request: %w", err)
	}
	httpReq.Header.Set("Authorization", "Bearer "+p.apiKey)

	resp, err := p.client.Do(httpReq)
	if err != nil {
		return 0, fmt.Errorf("5sim balance: %w", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		body, _ := io.ReadAll(io.LimitReader(resp.Body, fiveSimMaxBody))
		return 0, fmt.Errorf("5sim balance HTTP %d: %s", resp.StatusCode, truncateSMS(body))
	}

	limited := io.LimitReader(resp.Body, fiveSimMaxBody)
	raw, err := io.ReadAll(limited)
	if err != nil {
		return 0, fmt.Errorf("5sim balance read: %w", err)
	}

	var profile struct {
		Balance float64 `json:"balance"`
	}
	if err := json.Unmarshal(raw, &profile); err != nil {
		return 0, fmt.Errorf("5sim balance decode: %w", err)
	}

	return profile.Balance, nil
}

func (p *FiveSimProvider) GetPrices(ctx context.Context, country, service string) (float64, error) {
	path := fmt.Sprintf("%s/v1/user/guest/prices/%s/%s", p.baseURL, country, service)
	httpReq, err := http.NewRequestWithContext(ctx, http.MethodGet, path, nil)
	if err != nil {
		return 0, fmt.Errorf("5sim prices request: %w", err)
	}

	resp, err := p.client.Do(httpReq)
	if err != nil {
		return 0, fmt.Errorf("5sim prices: %w", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		body, _ := io.ReadAll(io.LimitReader(resp.Body, fiveSimMaxBody))
		return 0, fmt.Errorf("5sim prices HTTP %d: %s", resp.StatusCode, truncateSMS(body))
	}

	limited := io.LimitReader(resp.Body, fiveSimMaxBody)
	raw, err := io.ReadAll(limited)
	if err != nil {
		return 0, fmt.Errorf("5sim prices read: %w", err)
	}

	var prices map[string]map[string]struct {
		Cost float64 `json:"cost"`
	}
	if err := json.Unmarshal(raw, &prices); err != nil {
		return 0, fmt.Errorf("5sim prices decode: %w", err)
	}

	for _, opers := range prices {
		for _, op := range opers {
			return op.Cost, nil
		}
	}

	return 0, fmt.Errorf("5sim: no price found for %s/%s", country, service)
}

func (p *FiveSimProvider) fiveSimAction(ctx context.Context, url, action string) error {
	httpReq, err := http.NewRequestWithContext(ctx, http.MethodGet, url, nil)
	if err != nil {
		return fmt.Errorf("5sim %s request: %w", action, err)
	}
	httpReq.Header.Set("Authorization", "Bearer "+p.apiKey)

	resp, err := p.client.Do(httpReq)
	if err != nil {
		return fmt.Errorf("5sim %s: %w", action, err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		body, _ := io.ReadAll(io.LimitReader(resp.Body, fiveSimMaxBody))
		return fmt.Errorf("5sim %s HTTP %d: %s", action, resp.StatusCode, truncateSMS(body))
	}

	return nil
}

func parseFiveSimStatus(s string) NumberStatus {
	switch strings.ToLower(s) {
	case "received", "sms_received", "finished":
		return StatusReceived
	case "cancel", "canceled":
		return StatusCanceled
	case "timeout":
		return StatusTimeout
	default:
		return StatusPending
	}
}

func parseFiveSimTime(s string) (time.Time, error) {
	return time.Parse(time.RFC3339, s)
}

func truncateSMS(b []byte) string {
	const maxLen = 256
	if len(b) > maxLen {
		return strings.TrimSpace(string(b[:maxLen])) + "..."
	}
	return strings.TrimSpace(string(b))
}
