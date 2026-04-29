package llm

import (
	"bytes"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"os"

	"ant-chrome/backend/internal/config"
)

// Client 执行 LLM API 调用
type Client struct {
	cfg  config.LLMConfig
	http *http.Client
}

// ChatMessage represents a single message in a chat conversation.
type ChatMessage struct {
	Role    string `json:"role"`
	Content string `json:"content"`
}

// chatRequest is the OpenAI-compatible request body.
type chatRequest struct {
	Model       string        `json:"model"`
	Messages    []ChatMessage `json:"messages"`
	Temperature float64       `json:"temperature"`
	MaxTokens   int           `json:"max_tokens"`
}

// chatResponse is the OpenAI-compatible response body.
type chatResponse struct {
	Choices []struct {
		Message ChatMessage `json:"message"`
	} `json:"choices"`
	Error *struct {
		Message string `json:"message"`
	} `json:"error,omitempty"`
}

// NewClient 创建 LLM 客户端
func NewClient(cfg config.LLMConfig) *Client {
	apiKey := cfg.APIKey
	if apiKey == "" {
		apiKey = os.Getenv("LLM_API_KEY")
	}
	if apiKey == "" {
		apiKey = os.Getenv("OPENAI_API_KEY")
	}
	cfg.APIKey = apiKey

	return &Client{
		cfg: cfg,
		http: &http.Client{
			Timeout: cfg.Timeout,
		},
	}
}

// HasKey returns true if an API key is configured.
func (c *Client) HasKey() bool {
	return c.cfg.APIKey != ""
}

// Chat sends a conversation to the LLM and returns the response text.
func (c *Client) Chat(systemPrompt string, userMessage string) (string, error) {
	if !c.HasKey() {
		return "", fmt.Errorf("LLM API key not configured (set llm.api_key in config.yaml or LLM_API_KEY env var)")
	}

	messages := []ChatMessage{
		{Role: "system", Content: systemPrompt},
		{Role: "user", Content: userMessage},
	}

	reqBody := chatRequest{
		Model:       c.cfg.Model,
		Messages:    messages,
		Temperature: 0.3,
		MaxTokens:   4096,
	}

	bodyBytes, err := json.Marshal(reqBody)
	if err != nil {
		return "", fmt.Errorf("marshal request: %w", err)
	}

	url := c.cfg.Endpoint + "/chat/completions"
	req, err := http.NewRequest(http.MethodPost, url, bytes.NewReader(bodyBytes))
	if err != nil {
		return "", fmt.Errorf("create request: %w", err)
	}
	req.Header.Set("Content-Type", "application/json")
	req.Header.Set("Authorization", "Bearer "+c.cfg.APIKey)

	resp, err := c.http.Do(req)
	if err != nil {
		return "", fmt.Errorf("LLM API call: %w", err)
	}
	defer resp.Body.Close()

	respBytes, err := io.ReadAll(io.LimitReader(resp.Body, 1<<20)) // 1MB max
	if err != nil {
		return "", fmt.Errorf("read response: %w", err)
	}

	if resp.StatusCode != http.StatusOK {
		return "", fmt.Errorf("LLM API returned %d: %s", resp.StatusCode, string(respBytes))
	}

	var chatResp chatResponse
	if err := json.Unmarshal(respBytes, &chatResp); err != nil {
		return "", fmt.Errorf("parse response: %w", err)
	}

	if chatResp.Error != nil {
		return "", fmt.Errorf("LLM API error: %s", chatResp.Error.Message)
	}

	if len(chatResp.Choices) == 0 {
		return "", fmt.Errorf("LLM returned no choices")
	}

	return chatResp.Choices[0].Message.Content, nil
}
