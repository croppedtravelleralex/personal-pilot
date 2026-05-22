package email

import (
	"context"
	"fmt"
	"regexp"
	"time"
)

type Address struct {
	Email     string
	Password  string
	Provider  string
	Token     string
	ExpiresAt time.Time
}

type Provider interface {
	Name() string
	CreateAddress(ctx context.Context) (*Address, error)
	Address() *Address
	WaitForCode(ctx context.Context, filter *MailFilter, timeout time.Duration) (string, error)
	WaitForMail(ctx context.Context, filter *MailFilter, timeout time.Duration) (*MailMessage, error)
}

type CloudflareProvider struct {
	inner *CloudflareTempClient
}

func NewCloudflareProvider(client *CloudflareTempClient) *CloudflareProvider {
	return &CloudflareProvider{inner: client}
}

func CreateCloudflareProvider(ctx context.Context, apiBase string) (*CloudflareProvider, error) {
	client, err := NewCloudflareTempClient(apiBase)
	if err != nil {
		return nil, err
	}
	return NewCloudflareProvider(client), nil
}

func (p *CloudflareProvider) Name() string { return "cloudflare_temp" }

func (p *CloudflareProvider) CreateAddress(ctx context.Context) (*Address, error) {
	provider, err := CreateCloudflareProvider(ctx, "")
	if err != nil {
		return nil, err
	}
	p.inner = provider.inner
	return p.Address(), nil
}

func (p *CloudflareProvider) Address() *Address {
	if p.inner == nil {
		return nil
	}
	return &Address{
		Email:     p.inner.Address(),
		Provider:  p.Name(),
		ExpiresAt: time.Now().Add(TempEmailPollMaxWait),
	}
}

func (p *CloudflareProvider) WaitForCode(ctx context.Context, filter *MailFilter, timeout time.Duration) (string, error) {
	if p.inner == nil {
		return "", fmt.Errorf("cloudflare temp email provider is not initialized")
	}
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

		mails, err := p.inner.FetchRawMails()
		if err != nil {
			time.Sleep(TempEmailPollInterval)
			continue
		}

		for _, mail := range mails {
			if filter != nil && !mailMatchesFilter(&mail, *filter) {
				continue
			}
			if code := ExtractCodeFromMail(&mail, codePattern(filter)); code != "" {
				return code, nil
			}
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

func (p *CloudflareProvider) WaitForMail(ctx context.Context, filter *MailFilter, timeout time.Duration) (*MailMessage, error) {
	if p.inner == nil {
		return nil, fmt.Errorf("cloudflare temp email provider is not initialized")
	}
	f := MailFilter{}
	if filter != nil {
		f = *filter
	}
	return p.inner.WaitForMail(ctx, timeout, f)
}

type MailTMProvider struct {
	inner *MailTMClient
}

func NewMailTMProvider(client *MailTMClient) *MailTMProvider {
	return &MailTMProvider{inner: client}
}

func CreateMailTMProvider(ctx context.Context) (*MailTMProvider, error) {
	client, err := NewMailTMClientHuman()
	if err != nil {
		return nil, err
	}
	return NewMailTMProvider(client), nil
}

func (p *MailTMProvider) Name() string { return "mailtm" }

func (p *MailTMProvider) CreateAddress(ctx context.Context) (*Address, error) {
	provider, err := CreateMailTMProvider(ctx)
	if err != nil {
		return nil, err
	}
	p.inner = provider.inner
	return p.Address(), nil
}

func (p *MailTMProvider) Address() *Address {
	if p.inner == nil {
		return nil
	}
	return &Address{
		Email:     p.inner.Address(),
		Provider:  p.Name(),
		ExpiresAt: time.Now().Add(mailTMPollMaxWait),
	}
}

func (p *MailTMProvider) WaitForCode(ctx context.Context, filter *MailFilter, timeout time.Duration) (string, error) {
	if p.inner == nil {
		return "", fmt.Errorf("mail.tm provider is not initialized")
	}
	ctx, cancel := context.WithTimeout(ctx, timeout)
	defer cancel()

	for {
		select {
		case <-ctx.Done():
			return "", fmt.Errorf("verification code not received within %v", timeout)
		default:
		}

		msgs, err := p.inner.listMessages()
		if err != nil {
			time.Sleep(mailTMPollInterval)
			continue
		}

		for _, m := range msgs {
			text, html := p.inner.fetchDetail(m.ID)
			mail := &MailMessage{
				ID:      m.ID,
				Subject: m.Subject,
				Text:    text,
				HTML:    html,
			}

			if filter != nil && !mailMatchesFilter(mail, *filter) {
				continue
			}
			if code := ExtractCodeFromMail(mail, codePattern(filter)); code != "" {
				return code, nil
			}
		}

		time.Sleep(mailTMPollInterval)
	}
}

func (p *MailTMProvider) WaitForMail(ctx context.Context, filter *MailFilter, timeout time.Duration) (*MailMessage, error) {
	if p.inner == nil {
		return nil, fmt.Errorf("mail.tm provider is not initialized")
	}
	f := MailFilter{}
	if filter != nil {
		f = *filter
	}
	return p.inner.WaitForMail(ctx, timeout, f)
}

func codePattern(filter *MailFilter) *regexp.Regexp {
	if filter == nil {
		return nil
	}
	return filter.CodePattern
}
