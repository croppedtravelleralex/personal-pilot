package sms

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"time"
)

const (
	smspoolBaseURL = "https://api.smspool.net"
	smspoolMaxBody = 1 << 20
)

type SMSPoolProvider struct {
	baseURL string
	apiKey  string
	client  *http.Client
}

func NewSMSPoolProvider(apiKey string) *SMSPoolProvider {
	return &SMSPoolProvider{
		baseURL: smspoolBaseURL,
		apiKey:  apiKey,
		client:  &http.Client{Timeout: 30 * time.Second},
	}
}

func (p *SMSPoolProvider) Name() string { return "smspool" }

func (p *SMSPoolProvider) BuyNumber(ctx context.Context, req *BuyRequest) (*Number, error) {
	service := req.Service
	country := req.Country
	if country == "" {
		country = "us"
	}
	path := fmt.Sprintf("%s/sms/order/%s/%s", p.baseURL, service, country)

	httpReq, err := http.NewRequestWithContext(ctx, http.MethodGet, path, nil)
	if err != nil {
		return nil, fmt.Errorf("smspool buy request: %w", err)
	}
	httpReq.Header.Set("Authorization", "Bearer "+p.apiKey)

	resp, err := p.client.Do(httpReq)
	if err != nil {
		return nil, fmt.Errorf("smspool buy: %w", err)
	}
	defer resp.Body.Close()

	limited := io.LimitReader(resp.Body, smspoolMaxBody)
	raw, err := io.ReadAll(limited)
	if err != nil {
		return nil, fmt.Errorf("smspool buy read: %w", err)
	}

	if resp.StatusCode != http.StatusOK {
		return nil, fmt.Errorf("smspool buy HTTP %d: %s", resp.StatusCode, truncateSMS(raw))
	}

	var result struct {
		OrderID   int     `json:"order_id"`
		Number    string  `json:"number"`
		Cost      float64 `json:"cost"`
		Service   string  `json:"service"`
		Country   string  `json:"country"`
		Status    string  `json:"status"`
		ExpiresAt string  `json:"expires_at"`
		Success   bool    `json:"success"`
		Message   string  `json:"message"`
	}
	if err := json.Unmarshal(raw, &result); err != nil {
		return nil, fmt.Errorf("smspool buy decode: %w", err)
	}

	if !result.Success && result.OrderID == 0 {
		return nil, fmt.Errorf("smspool buy failed: %s", result.Message)
	}

	n := &Number{
		ID:        fmt.Sprintf("%d", result.OrderID),
		Phone:     result.Number,
		Country:   result.Country,
		Service:   result.Service,
		Price:     result.Cost,
		Status:    parseSMSPoolStatus(result.Status),
		CreatedAt: time.Now(),
	}

	if result.ExpiresAt != "" {
		if t, err := time.Parse(time.RFC3339, result.ExpiresAt); err == nil {
			n.ExpiresAt = t
		}
	}

	phone := result.Number
	if len(phone) > 0 && phone[0] == '+' {
		phone = phone[1:]
	}
	if len(phone) > 1 {
		n.PhoneCC = phone[:1]
		n.PhoneNum = phone[1:]
	}

	return n, nil
}

func (p *SMSPoolProvider) CheckSMS(ctx context.Context, orderID string) (*SMSResult, error) {
	path := fmt.Sprintf("%s/sms/check/%s", p.baseURL, orderID)
	httpReq, err := http.NewRequestWithContext(ctx, http.MethodGet, path, nil)
	if err != nil {
		return nil, fmt.Errorf("smspool check request: %w", err)
	}
	httpReq.Header.Set("Authorization", "Bearer "+p.apiKey)

	resp, err := p.client.Do(httpReq)
	if err != nil {
		return nil, fmt.Errorf("smspool check: %w", err)
	}
	defer resp.Body.Close()

	limited := io.LimitReader(resp.Body, smspoolMaxBody)
	raw, err := io.ReadAll(limited)
	if err != nil {
		return nil, fmt.Errorf("smspool check read: %w", err)
	}

	if resp.StatusCode != http.StatusOK {
		return nil, fmt.Errorf("smspool check HTTP %d: %s", resp.StatusCode, truncateSMS(raw))
	}

	var result struct {
		Success    bool   `json:"success"`
		Status     int    `json:"status"`
		SMS        string `json:"sms"`
		Sender     string `json:"sender"`
		Message    string `json:"message"`
		InsertDate string `json:"insert_date"`
	}
	if err := json.Unmarshal(raw, &result); err != nil {
		return nil, fmt.Errorf("smspool check decode: %w", err)
	}

	smsResult := &SMSResult{
		Status: parseSMSPoolCheckStatus(result.Status),
	}

	if result.SMS != "" {
		data := &SMSData{
			Text:   result.SMS,
			Sender: result.Sender,
		}
		if result.InsertDate != "" {
			if t, err := time.Parse("2006-01-02 15:04:05", result.InsertDate); err == nil {
				data.ReceivedAt = t
			}
		}
		smsResult.SMS = data

		if smsResult.Status == StatusPending && data.Text != "" {
			smsResult.Status = StatusReceived
		}
	}

	return smsResult, nil
}

func (p *SMSPoolProvider) Cancel(ctx context.Context, orderID string) error {
	path := fmt.Sprintf("%s/sms/cancel/%s", p.baseURL, orderID)
	httpReq, err := http.NewRequestWithContext(ctx, http.MethodPost, path, bytes.NewReader([]byte("{}")))
	if err != nil {
		return fmt.Errorf("smspool cancel request: %w", err)
	}
	httpReq.Header.Set("Authorization", "Bearer "+p.apiKey)
	httpReq.Header.Set("Content-Type", "application/json")

	resp, err := p.client.Do(httpReq)
	if err != nil {
		return fmt.Errorf("smspool cancel: %w", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		body, _ := io.ReadAll(io.LimitReader(resp.Body, smspoolMaxBody))
		return fmt.Errorf("smspool cancel HTTP %d: %s", resp.StatusCode, truncateSMS(body))
	}

	return nil
}

func (p *SMSPoolProvider) Finish(ctx context.Context, orderID string) error {
	return nil
}

func (p *SMSPoolProvider) GetBalance(ctx context.Context) (float64, error) {
	path := fmt.Sprintf("%s/balance", p.baseURL)
	httpReq, err := http.NewRequestWithContext(ctx, http.MethodGet, path, nil)
	if err != nil {
		return 0, fmt.Errorf("smspool balance request: %w", err)
	}
	httpReq.Header.Set("Authorization", "Bearer "+p.apiKey)

	resp, err := p.client.Do(httpReq)
	if err != nil {
		return 0, fmt.Errorf("smspool balance: %w", err)
	}
	defer resp.Body.Close()

	limited := io.LimitReader(resp.Body, smspoolMaxBody)
	raw, err := io.ReadAll(limited)
	if err != nil {
		return 0, fmt.Errorf("smspool balance read: %w", err)
	}

	if resp.StatusCode != http.StatusOK {
		return 0, fmt.Errorf("smspool balance HTTP %d: %s", resp.StatusCode, truncateSMS(raw))
	}

	var result struct {
		Balance float64 `json:"balance"`
	}
	if err := json.Unmarshal(raw, &result); err != nil {
		return 0, fmt.Errorf("smspool balance decode: %w", err)
	}

	return result.Balance, nil
}

func (p *SMSPoolProvider) GetPrices(ctx context.Context, country, service string) (float64, error) {
	return 0, fmt.Errorf("smspool: GetPrices not implemented via API")
}

func parseSMSPoolStatus(s string) NumberStatus {
	switch s {
	case "pending", "active":
		return StatusPending
	case "completed", "received":
		return StatusReceived
	case "cancelled", "canceled", "canceling":
		return StatusCanceled
	case "expired", "timeout":
		return StatusTimeout
	default:
		return StatusPending
	}
}

func parseSMSPoolCheckStatus(code int) NumberStatus {
	switch code {
	case 1:
		return StatusPending
	case 2, 3:
		return StatusReceived
	case 4:
		return StatusCanceled
	case 5:
		return StatusTimeout
	default:
		return StatusPending
	}
}
