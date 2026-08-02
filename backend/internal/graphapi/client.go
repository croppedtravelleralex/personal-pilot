package graphapi

import (
	"context"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"net/url"
	"strings"
	"time"

	"personal-pilot/backend/internal/transport"
	"personal-pilot/backend/internal/trust"
)

const (
	tokenEndpoint = "https://login.microsoftonline.com/common/oauth2/v2.0/token"
	graphBase     = "https://graph.microsoft.com/v1.0"
)

// Client performs Microsoft Graph calls using trust-inherited tokens.
type Client struct {
	HTTP         *http.Client
	ClientID     string
	ClientSecret string
	UserAgent    string
}

func (c *Client) userAgent() string {
	return transport.BrowserLikeUserAgent(c.UserAgent)
}

func (c *Client) setUserAgent(req *http.Request) {
	req.Header.Set("User-Agent", c.userAgent())
}

// RefreshAccessToken exchanges refresh_token for a new access token.
func (c *Client) RefreshAccessToken(ctx context.Context, refreshToken string) (trust.Bundle, error) {
	if strings.TrimSpace(refreshToken) == "" {
		return trust.Bundle{}, fmt.Errorf("refresh_token required")
	}
	form := url.Values{}
	form.Set("client_id", c.ClientID)
	form.Set("grant_type", "refresh_token")
	form.Set("refresh_token", refreshToken)
	if c.ClientSecret != "" {
		form.Set("client_secret", c.ClientSecret)
	}
	req, err := http.NewRequestWithContext(ctx, http.MethodPost, tokenEndpoint, strings.NewReader(form.Encode()))
	if err != nil {
		return trust.Bundle{}, err
	}
	req.Header.Set("Content-Type", "application/x-www-form-urlencoded")
	c.setUserAgent(req)
	httpClient := c.HTTP
	if httpClient == nil {
		httpClient = &http.Client{Timeout: 30 * time.Second}
	}
	resp, err := httpClient.Do(req)
	if err != nil {
		return trust.Bundle{}, err
	}
	defer resp.Body.Close()
	body, _ := io.ReadAll(resp.Body)
	if resp.StatusCode >= 400 {
		return trust.Bundle{}, fmt.Errorf("token refresh failed: %s", string(body))
	}
	var payload struct {
		AccessToken  string `json:"access_token"`
		RefreshToken string `json:"refresh_token"`
		ExpiresIn    int64  `json:"expires_in"`
	}
	if err := json.Unmarshal(body, &payload); err != nil {
		return trust.Bundle{}, err
	}
	b := trust.Bundle{
		Provider:    trust.ProviderMicrosoft,
		AccessToken: payload.AccessToken,
		UpdatedAt:   time.Now().UTC(),
	}
	if payload.RefreshToken != "" {
		b.RefreshToken = payload.RefreshToken
	} else {
		b.RefreshToken = refreshToken
	}
	if payload.ExpiresIn > 0 {
		b.ExpiresAt = time.Now().UTC().Add(time.Duration(payload.ExpiresIn) * time.Second)
	}
	return b, nil
}

// ListMessages returns recent inbox messages via Graph API (browser bypass path).
func (c *Client) ListMessages(ctx context.Context, accessToken string, top int) (map[string]interface{}, error) {
	if top <= 0 {
		top = 10
	}
	endpoint := fmt.Sprintf("%s/me/messages?$top=%d&$select=subject,receivedDateTime,from", graphBase, top)
	req, err := http.NewRequestWithContext(ctx, http.MethodGet, endpoint, nil)
	if err != nil {
		return nil, err
	}
	req.Header.Set("Authorization", "Bearer "+accessToken)
	req.Header.Set("Accept", "application/json")
	c.setUserAgent(req)
	httpClient := c.HTTP
	if httpClient == nil {
		httpClient = &http.Client{Timeout: 30 * time.Second}
	}
	resp, err := httpClient.Do(req)
	if err != nil {
		return nil, err
	}
	defer resp.Body.Close()
	body, _ := io.ReadAll(resp.Body)
	if resp.StatusCode >= 400 {
		return nil, fmt.Errorf("graph api %d: %s", resp.StatusCode, string(body))
	}
	var out map[string]interface{}
	if err := json.Unmarshal(body, &out); err != nil {
		return nil, err
	}
	return out, nil
}
