package webhook

import (
	"bytes"
	"context"
	"crypto/hmac"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"net/http"
	"sync"
	"time"
)

type WebhookEvent struct {
	Event     string                 `json:"event"`
	Timestamp string                 `json:"timestamp"`
	Data      map[string]interface{} `json:"data"`
}

var (
	client     *http.Client
	webhookURL string
	webhookKey string
	mu         sync.RWMutex
	once       sync.Once
)

func initClient() {
	once.Do(func() {
		client = &http.Client{Timeout: 10 * time.Second}
	})
}

func SetWebhook(url, secretKey string) {
	mu.Lock()
	webhookURL = url
	webhookKey = secretKey
	mu.Unlock()
}

func Send(ctx context.Context, event string, data map[string]interface{}) error {
	mu.RLock()
	url := webhookURL
	key := webhookKey
	mu.RUnlock()

	if url == "" {
		return nil
	}

	initClient()

	evt := WebhookEvent{
		Event:     event,
		Timestamp: time.Now().UTC().Format(time.RFC3339),
		Data:      data,
	}

	body, _ := json.Marshal(evt)

	req, err := http.NewRequestWithContext(ctx, "POST", url, bytes.NewReader(body))
	if err != nil {
		return fmt.Errorf("webhook request: %w", err)
	}
	req.Header.Set("Content-Type", "application/json")
	req.Header.Set("User-Agent", "Personal-Pilot-Webhook/1.0")

	if key != "" {
		mac := hmac.New(sha256.New, []byte(key))
		mac.Write(body)
		sig := hex.EncodeToString(mac.Sum(nil))
		req.Header.Set("X-Webhook-Signature", sig)
	}

	resp, err := client.Do(req)
	if err != nil {
		return fmt.Errorf("webhook delivery: %w", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode >= 300 {
		return fmt.Errorf("webhook HTTP %d", resp.StatusCode)
	}
	return nil
}
