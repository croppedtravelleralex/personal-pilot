package email

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"regexp"
	"strings"
	"sync"
	"time"
)

const (
	mailTMDomainsURL  = "https://api.mail.tm/domains"
	mailTMAccountsURL = "https://api.mail.tm/accounts"
	mailTMTokenURL    = "https://api.mail.tm/token"
	mailTMMessagesURL = "https://api.mail.tm/messages"
	mailTMDialTimeout = 15 * time.Second
	mailTMPollInterval = 5 * time.Second
	mailTMPollMaxWait  = 3 * time.Minute
)

var mailTMCodeRe = regexp.MustCompile(`\b\d{6}\b`)

// MailTMClient provides access to mail.tm temporary email API.
type MailTMClient struct {
	hc       *http.Client
	address  string
	password string
	token    string

	cachedIDs map[string]struct{}
	cacheMu   sync.Mutex
}

// NewMailTMClient creates a new mail.tm account with an auto-generated address.
func NewMailTMClient() (*MailTMClient, error) {
	return NewMailTMClientWithAddress("")
}

// NewMailTMClientHuman creates a mail.tm account with a realistic human-like email address.
func NewMailTMClientHuman() (*MailTMClient, error) {
	hc := &http.Client{Timeout: mailTMDialTimeout}
	domain, err := getDomain(hc)
	if err != nil {
		return nil, fmt.Errorf("mail.tm get domain: %w", err)
	}

	address := GenerateHumanEmail(domain)
	password := SecureHex(16)
	return createMailTMClient(hc, address, password)
}

// NewMailTMClientWithAddress creates a mail.tm account with a specific address prefix.
// If addressPrefix is empty, a random hex-based address is generated.
func NewMailTMClientWithAddress(addressPrefix string) (*MailTMClient, error) {
	hc := &http.Client{Timeout: mailTMDialTimeout}
	domain, err := getDomain(hc)
	if err != nil {
		return nil, fmt.Errorf("mail.tm get domain: %w", err)
	}

	address := strings.TrimSpace(addressPrefix)
	if address == "" {
		address = "ds" + SecureHex(8)
	}
	// If it already contains @, use as-is; otherwise append domain
	if !strings.Contains(address, "@") {
		address = address + "@" + domain
	}

	password := SecureHex(16)
	return createMailTMClient(hc, address, password)
}

func createMailTMClient(hc *http.Client, address, password string) (*MailTMClient, error) {
	if err := createAccount(hc, address, password); err != nil {
		return nil, fmt.Errorf("mail.tm create account: %w", err)
	}

	token, err := getToken(hc, address, password)
	if err != nil {
		return nil, fmt.Errorf("mail.tm get token: %w", err)
	}

	return &MailTMClient{
		hc:        hc,
		address:   address,
		password:  password,
		token:     token,
		cachedIDs: make(map[string]struct{}),
	}, nil
}

// Address returns the email address.
func (c *MailTMClient) Address() string { return c.address }

// WaitForCode polls the inbox until a verification code is found.
func (c *MailTMClient) WaitForCode() (string, error) {
	deadline := time.Now().Add(mailTMPollMaxWait)

	for {
		if time.Now().After(deadline) {
			return "", fmt.Errorf("mail.tm poll timeout for %s", c.address)
		}

		msgs, err := c.listMessages()
		if err != nil {
			time.Sleep(mailTMPollInterval)
			continue
		}

		for _, m := range msgs {
			c.cacheMu.Lock()
			_, seen := c.cachedIDs[m.ID]
			if !seen {
				c.cachedIDs[m.ID] = struct{}{}
			}
			c.cacheMu.Unlock()
			if seen {
				continue
			}

			text, html := c.fetchDetail(m.ID)
			if code := smartExtractCode(m.Subject); code != "" {
				return code, nil
			}
			if code := smartExtractCode(text); code != "" {
				return code, nil
			}
			if code := smartExtractCode(stripHTML(html)); code != "" {
				return code, nil
			}
		}

		time.Sleep(mailTMPollInterval)
	}
}

func (c *MailTMClient) WaitForMail(ctx context.Context, timeout time.Duration, filter MailFilter) (*MailMessage, error) {
	ctx, cancel := context.WithTimeout(ctx, timeout)
	defer cancel()

	for {
		select {
		case <-ctx.Done():
			return nil, fmt.Errorf("matching mail not received within %v", timeout)
		default:
		}

		msgs, err := c.listMessages()
		if err != nil {
			time.Sleep(mailTMPollInterval)
			continue
		}

		for _, m := range msgs {
			c.cacheMu.Lock()
			_, seen := c.cachedIDs[m.ID]
			if !seen {
				c.cachedIDs[m.ID] = struct{}{}
			}
			c.cacheMu.Unlock()
			if seen {
				continue
			}

			text, html := c.fetchDetail(m.ID)
			mail := &MailMessage{
				ID:      m.ID,
				Subject: m.Subject,
				Text:    text,
				HTML:    html,
				From:    "",
			}
			if mailMatchesFilter(mail, filter) {
				return mail, nil
			}
		}

		time.Sleep(mailTMPollInterval)
	}
}

type mailTMDomain struct {
	Domain string `json:"domain"`
}

type mailTMAccount struct {
	ID      string `json:"id"`
	Address string `json:"address"`
}

type mailTMToken struct {
	Token string `json:"token"`
}

type mailTMMessage struct {
	ID        string `json:"id"`
	Subject   string `json:"subject"`
	CreatedAt string `json:"createdAt"`
}

type mailTMMessageDetail struct {
	TextBody string `json:"text"`
	HTMLBody string `json:"html"`
}

func (c *MailTMClient) listMessages() ([]mailTMMessage, error) {
	req, _ := http.NewRequest(http.MethodGet, mailTMMessagesURL, nil)
	req.Header.Set("Authorization", "Bearer "+c.token)

	resp, err := c.hc.Do(req)
	if err != nil {
		return nil, err
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		return nil, fmt.Errorf("list messages: %d", resp.StatusCode)
	}

	var result struct {
		Member []mailTMMessage `json:"hydra:member"`
	}
	if err := json.NewDecoder(resp.Body).Decode(&result); err != nil {
		return nil, err
	}
	return result.Member, nil
}

func (c *MailTMClient) fetchDetail(id string) (text, html string) {
	url := fmt.Sprintf("%s/%s", mailTMMessagesURL, id)
	req, _ := http.NewRequest(http.MethodGet, url, nil)
	req.Header.Set("Authorization", "Bearer "+c.token)

	resp, err := c.hc.Do(req)
	if err != nil {
		return "", ""
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		return "", ""
	}

	var detail mailTMMessageDetail
	if err := json.NewDecoder(resp.Body).Decode(&detail); err != nil {
		return "", ""
	}

	return detail.TextBody, detail.HTMLBody
}

func (c *MailTMClient) getMessageBody(id string) (string, error) {
	text, html := c.fetchDetail(id)
	if text != "" {
		return text, nil
	}
	return html, nil
}

// ─── internal helpers ──────────────────────────────────────────────────────────

func getDomain(hc *http.Client) (string, error) {
	resp, err := hc.Get(mailTMDomainsURL)
	if err != nil {
		return "", err
	}
	defer resp.Body.Close()

	var result struct {
		Member []mailTMDomain `json:"hydra:member"`
	}
	if err := json.NewDecoder(resp.Body).Decode(&result); err != nil {
		return "", err
	}
	if len(result.Member) == 0 || result.Member[0].Domain == "" {
		return "", fmt.Errorf("no domains available")
	}
	return result.Member[0].Domain, nil
}

func createAccount(hc *http.Client, address, password string) error {
	body, _ := json.Marshal(map[string]string{
		"address":  address,
		"password": password,
	})
	resp, err := hc.Post(mailTMAccountsURL, "application/json", bytes.NewReader(body))
	if err != nil {
		return err
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusCreated && resp.StatusCode != http.StatusOK {
		b, _ := io.ReadAll(io.LimitReader(resp.Body, 1024))
		return fmt.Errorf("create account: %d %s", resp.StatusCode, string(b))
	}
	return nil
}

func getToken(hc *http.Client, address, password string) (string, error) {
	body, _ := json.Marshal(map[string]string{
		"address":  address,
		"password": password,
	})
	resp, err := hc.Post(mailTMTokenURL, "application/json", bytes.NewReader(body))
	if err != nil {
		return "", err
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		b, _ := io.ReadAll(io.LimitReader(resp.Body, 1024))
		return "", fmt.Errorf("get token: %d %s", resp.StatusCode, string(b))
	}

	var result mailTMToken
	if err := json.NewDecoder(resp.Body).Decode(&result); err != nil {
		return "", err
	}
	return result.Token, nil
}

// CodeRegex matches 4-8 digit verification codes in text.
var CodeRegex = regexp.MustCompile(`\b\d{4,8}\b`)

// ExtractOTP pulls the first verification code from a text body.
func ExtractOTP(body string) string {
	if code := mailTMCodeRe.FindString(body); code != "" {
		return code
	}
	return CodeRegex.FindString(strings.TrimSpace(body))
}
