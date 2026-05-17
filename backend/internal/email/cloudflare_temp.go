package email

import (
	"bytes"
	"context"
	"crypto/rand"
	"encoding/json"
	"fmt"
	"io"
	"math"
	"math/big"
	"net/http"
	"regexp"
	"strings"
	"sync"
	"time"
)

const (
	DefaultTempEmailAPIBase = "https://temp-email-api.chihuolingrang.de5.net"
	mailtmAPIBase           = "https://api.mail.tm"
	maxRetries              = 3
	maxResponseBytes        = 1 << 20 // 1MB limit for response bodies
)

var (
	TempEmailPollInterval = 3 * time.Second
	TempEmailPollMaxWait  = 5 * time.Minute

	// TempEmailAPIURLs is a list of worker API URLs for failover.
	// The first URL is the primary; subsequent URLs are fallbacks.
	TempEmailAPIURLs = []string{
		DefaultTempEmailAPIBase,
	}
)

// AddWorkerURL appends a backup worker API URL for failover.
func AddWorkerURL(url string) {
	TempEmailAPIURLs = append(TempEmailAPIURLs, url)
}

// TempEmailClient is the interface for temporary email providers.
type TempEmailClient interface {
	Address() string
	WaitForCode(ctx context.Context, timeout time.Duration) (string, error)
	FetchRawMails() ([]MailMessage, error)
	WaitForMail(ctx context.Context, timeout time.Duration, filter MailFilter) (*MailMessage, error)
}

// MailFilter defines criteria for matching incoming emails.
type MailFilter struct {
	FromSuffix      string
	SubjectContains string
	CodePattern     *regexp.Regexp
}

// MailMessage represents a parsed email message.
type MailMessage struct {
	ID      string `json:"id"`
	Subject string `json:"subject"`
	Text    string `json:"text"`
	HTML    string `json:"html"`
	From    string `json:"from"`
	To      string `json:"to"`
}

// CloudflareTempClient accesses the cloudflare_temp_email Worker API.
type CloudflareTempClient struct {
	hc      *http.Client
	apiBase string
	address string
	jwt     string

	cachedIDs    map[string]struct{}
	cacheMu      sync.Mutex
	pollInterval time.Duration
	pollMaxWait  time.Duration
}

// NewCloudflareTempClient creates a new temp email address via the Worker API.
// Tries each URL in TempEmailAPIURLs sequentially with exponential backoff.
// Falls back to mail.tm if all worker URLs fail.
func NewCloudflareTempClient(apiBase string) (*CloudflareTempClient, error) {
	urls := TempEmailAPIURLs
	if apiBase != "" {
		urls = []string{apiBase}
	}

	hc := &http.Client{Timeout: 15 * time.Second}

	var lastErr error
	for _, baseURL := range urls {
		for attempt := 0; attempt < maxRetries; attempt++ {
			client, err := tryCreateAddress(hc, baseURL)
			if err == nil {
				client.cachedIDs = make(map[string]struct{})
				client.pollInterval = TempEmailPollInterval
				client.pollMaxWait = TempEmailPollMaxWait
				return client, nil
			}
			lastErr = err
			if attempt < maxRetries-1 {
				backoff := time.Duration(math.Pow(2, float64(attempt))) * time.Second
				time.Sleep(backoff)
			}
		}
	}

	client, err := tryMailTmFallback(hc)
	if err != nil {
		return nil, fmt.Errorf("all workers failed (%w); mail.tm fallback also failed (%v)", lastErr, err)
	}
	client.cachedIDs = make(map[string]struct{})
	client.pollInterval = TempEmailPollInterval
	client.pollMaxWait = TempEmailPollMaxWait
	return client, nil
}

func tryCreateAddress(hc *http.Client, apiBase string) (*CloudflareTempClient, error) {
	resp, err := hc.Post(apiBase+"/api/new_address", "application/json", bytes.NewReader([]byte("{}")))
	if err != nil {
		return nil, fmt.Errorf("create temp address: %w", err)
	}
	defer resp.Body.Close()

	limited := io.LimitReader(resp.Body, maxResponseBytes)
	raw, err := io.ReadAll(limited)
	if err != nil {
		return nil, fmt.Errorf("read new_address response: %w", err)
	}

	if resp.StatusCode != 200 {
		return nil, fmt.Errorf("create temp address HTTP %d: %s", resp.StatusCode, truncateForError(raw))
	}

	var result struct {
		JWT     string `json:"jwt"`
		Address string `json:"address"`
	}
	if err := json.Unmarshal(raw, &result); err != nil {
		return nil, fmt.Errorf("decode new_address response: %w", err)
	}

	return &CloudflareTempClient{
		hc:      hc,
		apiBase: apiBase,
		address: result.Address,
		jwt:     result.JWT,
	}, nil
}

func tryMailTmFallback(hc *http.Client) (*CloudflareTempClient, error) {
	domainsResp, err := hc.Get(mailtmAPIBase + "/domains")
	if err != nil {
		return nil, fmt.Errorf("mail.tm domains: %w", err)
	}
	defer domainsResp.Body.Close()

	if domainsResp.StatusCode != 200 {
		return nil, fmt.Errorf("mail.tm domains HTTP %d", domainsResp.StatusCode)
	}

	var domains struct {
		HydraMember []struct {
			Domain string `json:"domain"`
		} `json:"hydra:member"`
	}
	if err := json.NewDecoder(io.LimitReader(domainsResp.Body, maxResponseBytes)).Decode(&domains); err != nil {
		return nil, fmt.Errorf("mail.tm decode domains: %w", err)
	}
	if len(domains.HydraMember) == 0 {
		return nil, fmt.Errorf("mail.tm: no domains available")
	}
	domain := domains.HydraMember[0].Domain

	n, _ := rand.Int(rand.Reader, big.NewInt(100000000))
	addr := fmt.Sprintf("dsreg%d", n.Int64())
	password := GenerateHumanPassword()
	payload := map[string]string{
		"address":  addr + "@" + domain,
		"password": password,
	}
	body, _ := json.Marshal(payload)
	acctResp, err := hc.Post(mailtmAPIBase+"/accounts", "application/json", bytes.NewReader(body))
	if err != nil {
		return nil, fmt.Errorf("mail.tm create account: %w", err)
	}
	defer acctResp.Body.Close()

	var account struct {
		Address string `json:"address"`
		ID      string `json:"id"`
	}
	if err := json.NewDecoder(io.LimitReader(acctResp.Body, maxResponseBytes)).Decode(&account); err != nil {
		return nil, fmt.Errorf("mail.tm decode account: %w", err)
	}

	tokenPayload := map[string]string{
		"address":  account.Address,
		"password": password,
	}
	tokenBody, _ := json.Marshal(tokenPayload)
	tokenResp, err := hc.Post(mailtmAPIBase+"/token", "application/json", bytes.NewReader(tokenBody))
	if err != nil {
		return nil, fmt.Errorf("mail.tm get token: %w", err)
	}
	defer tokenResp.Body.Close()

	var token struct {
		Token string `json:"token"`
	}
	if err := json.NewDecoder(io.LimitReader(tokenResp.Body, maxResponseBytes)).Decode(&token); err != nil {
		return nil, fmt.Errorf("mail.tm decode token: %w", err)
	}

	return &CloudflareTempClient{
		hc:      hc,
		apiBase: mailtmAPIBase,
		address: account.Address,
		jwt:     token.Token,
	}, nil
}

func (c *CloudflareTempClient) Address() string { return c.address }

// WaitForCode polls inbox until a verification code is found. Uses exponential backoff.
func (c *CloudflareTempClient) WaitForCode(ctx context.Context, timeout time.Duration) (string, error) {
	ctx, cancel := context.WithTimeout(ctx, timeout)
	defer cancel()

	baseInterval := 1 * time.Second
	maxInterval := 10 * time.Second
	attempt := 0

	for {
		select {
		case <-ctx.Done():
			return "", fmt.Errorf("verification code not received within %v", timeout)
		default:
		}

		code, err := c.fetchCode(ctx)
		if err == nil && code != "" {
			return code, nil
		}

		backoff := baseInterval * time.Duration(1<<min(attempt, 5))
		if backoff > maxInterval {
			backoff = maxInterval
		}
		attempt++

		select {
		case <-time.After(backoff):
		case <-ctx.Done():
			return "", fmt.Errorf("verification code not received within %v", timeout)
		}
	}
}

// WaitForMail polls inbox until a message matching the filter arrives.
// Returns the full mail message for further processing.
func (c *CloudflareTempClient) WaitForMail(ctx context.Context, timeout time.Duration, filter MailFilter) (*MailMessage, error) {
	ctx, cancel := context.WithTimeout(ctx, timeout)
	defer cancel()

	for {
		select {
		case <-ctx.Done():
			return nil, fmt.Errorf("matching mail not received within %v", timeout)
		default:
		}

		mails, err := c.FetchRawMails()
		if err != nil {
			time.Sleep(TempEmailPollInterval)
			continue
		}

		for i := len(mails) - 1; i >= 0; i-- {
			mail := mails[i]
			c.cacheMu.Lock()
			_, seen := c.cachedIDs[mail.ID]
			if !seen {
				c.cachedIDs[mail.ID] = struct{}{}
			}
			c.cacheMu.Unlock()
			if seen {
				continue
			}
			if mailMatchesFilter(&mail, filter) {
				return &mail, nil
			}
		}

		time.Sleep(TempEmailPollInterval)
	}
}

func mailMatchesFilter(mail *MailMessage, filter MailFilter) bool {
	if filter.FromSuffix != "" && !strings.HasSuffix(strings.ToLower(mail.From), strings.ToLower(filter.FromSuffix)) {
		return false
	}
	if filter.SubjectContains != "" && !strings.Contains(strings.ToLower(mail.Subject), strings.ToLower(filter.SubjectContains)) {
		return false
	}
	return true
}

// FetchRawMails returns all parsed mail messages from inbox.
func (c *CloudflareTempClient) FetchRawMails() ([]MailMessage, error) {
	url := c.apiBase + "/api/mails?limit=20&offset=0"
	if c.apiBase == mailtmAPIBase {
		url = c.apiBase + "/messages?page=1"
	}

	req, _ := http.NewRequest("GET", url, nil)
	req.Header.Set("Authorization", "Bearer "+c.jwt)

	resp, err := c.hc.Do(req)
	if err != nil {
		return nil, err
	}
	defer resp.Body.Close()

	if resp.StatusCode != 200 {
		return nil, fmt.Errorf("fetch mails HTTP %d", resp.StatusCode)
	}

	if c.apiBase == mailtmAPIBase {
		return c.parseMailTmMessages(resp)
	}
	return parsePrimaryMessages(resp)
}

func (c *CloudflareTempClient) fetchCode(ctx context.Context) (string, error) {
	mails, err := c.FetchRawMails()
	if err != nil {
		return "", err
	}

	c.cacheMu.Lock()
	for _, mail := range mails {
		if _, seen := c.cachedIDs[mail.ID]; seen {
			continue
		}
		c.cachedIDs[mail.ID] = struct{}{}
		c.cacheMu.Unlock()

		if code := smartExtractCode(mail.Subject); code != "" {
			return code, nil
		}
		if code := smartExtractCode(mail.Text); code != "" {
			return code, nil
		}
		if code := smartExtractCode(stripHTML(mail.HTML)); code != "" {
			return code, nil
		}

		c.cacheMu.Lock()
	}
	c.cacheMu.Unlock()
	return "", nil
}

// ExtractCodeFromMail attempts to extract a verification code from a specific mail
// message, optionally using a custom code pattern.
func ExtractCodeFromMail(mail *MailMessage, customPattern *regexp.Regexp) string {
	sources := []string{mail.Subject, mail.Text, stripHTML(mail.HTML)}
	for _, src := range sources {
		if src == "" {
			continue
		}
		if customPattern != nil {
			if matches := customPattern.FindStringSubmatch(src); len(matches) > 1 {
				return matches[1]
			}
			if code := customPattern.FindString(src); code != "" {
				return code
			}
		}
		if code := smartExtractCode(src); code != "" {
			return code
		}
	}
	return ""
}

// AllCodesFromMail extracts ALL verification codes from a mail message (not just the first).
// Returns deduplicated codes in order of appearance.
func AllCodesFromMail(mail *MailMessage) []string {
	seen := map[string]struct{}{}
	var codes []string
	sources := []string{mail.Subject, mail.Text, stripHTML(mail.HTML)}

	addCode := func(code string) {
		if code == "" {
			return
		}
		if _, dup := seen[code]; !dup {
			seen[code] = struct{}{}
			codes = append(codes, code)
		}
	}

	for _, src := range sources {
		if src == "" {
			continue
		}
		for _, kp := range keywordPatterns {
			for _, match := range kp.FindAllStringSubmatch(src, -1) {
				if len(match) > 1 {
					addCode(match[1])
				}
			}
		}
		for _, match := range re6Digit.FindAllStringSubmatch(src, -1) {
			if len(match) > 1 {
				addCode(match[1])
			}
		}
		for _, match := range re5to8Digit.FindAllStringSubmatch(src, -1) {
			if len(match) > 1 {
				addCode(match[1])
			}
		}
	}

	return codes
}

// Compile regexes once at package level to avoid recompilation on every call.
var (
	re6Digit    = regexp.MustCompile(`\b(\d{6})\b`)
	re4Digit    = regexp.MustCompile(`\b(\d{4})\b`)
	re5to8Digit = regexp.MustCompile(`\b(\d{5,8})\b`)

	// Keyword-labeled code patterns (English + Chinese).
	reCodeLabel1 = regexp.MustCompile(`(?:verification|verify|code|otp|pin|确认码|验证码)[:\s]*(\d{4,8})`)
	reCodeLabel2 = regexp.MustCompile(`(\d{4,8})(?:\s*(?:is|was|are)\s*(?:your|the)\s*(?:verification|code|otp|pin))`)
	reCodeLabel3 = regexp.MustCompile(`(?:code|otp|pin)[:\s]*(\d{4,8})`)
	reCodeLabel4 = regexp.MustCompile(`(\d{4,8})\s*(?:是|为)\s*(?:你的|您的)?(?:验证码|确认码)`)

	keywordPatterns = []*regexp.Regexp{
		reCodeLabel1,
		reCodeLabel2,
		reCodeLabel3,
		reCodeLabel4,
	}
)

// smartExtractCode extracts verification codes from text with multiple strategies.
// Strategy order: keyword-labeled codes first (most reliable), then bare digit patterns.
func smartExtractCode(text string) string {
	if text == "" {
		return ""
	}

	for _, kp := range keywordPatterns {
		if matches := kp.FindStringSubmatch(text); len(matches) > 1 {
			return matches[1]
		}
	}

	if matches := re6Digit.FindStringSubmatch(text); len(matches) > 1 {
		return matches[1]
	}

	if matches := re4Digit.FindStringSubmatch(text); len(matches) > 1 {
		return matches[1]
	}

	if matches := re5to8Digit.FindStringSubmatch(text); len(matches) > 1 {
		return matches[1]
	}

	return ""
}

var (
	reScriptTag = regexp.MustCompile(`(?si)<script[^>]*>.*?</script>`)
	reStyleTag  = regexp.MustCompile(`(?si)<style[^>]*>.*?</style>`)
	reBlockTag  = regexp.MustCompile(`(?si)</?(?:p|div|br|tr|li|h[1-6]|blockquote|section|article)[^>]*>`)
	reTag       = regexp.MustCompile(`<[^>]*>`)
	reWhitespace = regexp.MustCompile(`\s+`)
)

// stripHTML removes HTML tags and decodes common entities for plain text extraction.
func stripHTML(html string) string {
	if html == "" {
		return ""
	}

	html = reScriptTag.ReplaceAllString(html, " ")
	html = reStyleTag.ReplaceAllString(html, " ")
	html = reBlockTag.ReplaceAllString(html, "\n")
	html = reTag.ReplaceAllString(html, " ")

	html = strings.NewReplacer(
		"&nbsp;", " ", "&amp;", "&", "&lt;", "<", "&gt;", ">",
		"&quot;", `"`, "&#39;", "'", "&ndash;", "-", "&mdash;", "-",
	).Replace(html)

	html = reWhitespace.ReplaceAllString(html, " ")
	return strings.TrimSpace(html)
}

func parsePrimaryMessages(resp *http.Response) ([]MailMessage, error) {
	var result struct {
		Results []struct {
			ID      int    `json:"id"`
			Subject string `json:"subject"`
			Text    string `json:"text"`
			HTML    string `json:"html"`
			From    string `json:"from"`
		} `json:"results"`
	}
	if err := json.NewDecoder(io.LimitReader(resp.Body, maxResponseBytes)).Decode(&result); err != nil {
		return nil, err
	}

	mails := make([]MailMessage, 0, len(result.Results))
	for _, m := range result.Results {
		mails = append(mails, MailMessage{
			ID:      fmt.Sprintf("%d", m.ID),
			Subject: m.Subject,
			Text:    m.Text,
			HTML:    m.HTML,
			From:    m.From,
		})
	}
	return mails, nil
}

func (c *CloudflareTempClient) parseMailTmMessages(resp *http.Response) ([]MailMessage, error) {
	var result struct {
		HydraMember []struct {
			ID      string `json:"id"`
			Subject string `json:"subject"`
			From    struct {
				Address string `json:"address"`
				Name    string `json:"name"`
			} `json:"from"`
			Intro string `json:"intro"`
		} `json:"hydra:member"`
	}
	if err := json.NewDecoder(io.LimitReader(resp.Body, maxResponseBytes)).Decode(&result); err != nil {
		return nil, err
	}

	mails := make([]MailMessage, 0, len(result.HydraMember))
	for _, m := range result.HydraMember {
		mail := MailMessage{
			ID:      m.ID,
			Subject: m.Subject,
			Text:    m.Intro,
			From:    m.From.Address,
		}
		if text, html := c.fetchMailTmBody(context.Background(), m.ID); text != "" || html != "" {
			mail.Text = text
			mail.HTML = html
		}
		mails = append(mails, mail)
	}
	return mails, nil
}

func (c *CloudflareTempClient) fetchMailTmBody(ctx context.Context, msgID string) (text, html string) {
	url := fmt.Sprintf("%s/messages/%s", mailtmAPIBase, msgID)
	req, err := http.NewRequestWithContext(ctx, "GET", url, nil)
	if err != nil {
		return "", ""
	}
	req.Header.Set("Authorization", "Bearer "+c.jwt)

	resp, err := c.hc.Do(req)
	if err != nil {
		return "", ""
	}
	defer resp.Body.Close()

	var body struct {
		Text string `json:"text"`
		HTML string `json:"html"`
	}
	if err := json.NewDecoder(io.LimitReader(resp.Body, maxResponseBytes)).Decode(&body); err != nil {
		return "", ""
	}
	return body.Text, body.HTML
}

func truncateForError(raw []byte) string {
	const maxErrLen = 256
	if len(raw) > maxErrLen {
		return string(raw[:maxErrLen]) + "..."
	}
	return string(raw)
}

func min(a, b int) int {
	if a < b {
		return a
	}
	return b
}
