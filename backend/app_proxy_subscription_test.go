package backend

import "testing"

func TestConfiguredSubscriptionURLRequiresEnvironmentConfiguration(t *testing.T) {
	t.Setenv(proxySubscriptionURLEnv, "")
	if _, err := configuredSubscriptionURL(); err == nil {
		t.Fatal("expected missing subscription URL error")
	}
}

func TestConfiguredSubscriptionURLRejectsUnsafeScheme(t *testing.T) {
	t.Setenv(proxySubscriptionURLEnv, "file:///tmp/subscription.txt")
	if _, err := configuredSubscriptionURL(); err == nil {
		t.Fatal("expected unsafe scheme error")
	}
}

func TestConfiguredSubscriptionURLAcceptsHTTPS(t *testing.T) {
	t.Setenv(proxySubscriptionURLEnv, "https://example.com/subscription")
	got, err := configuredSubscriptionURL()
	if err != nil {
		t.Fatalf("configuredSubscriptionURL: %v", err)
	}
	if got != "https://example.com/subscription" {
		t.Fatalf("url=%q", got)
	}
}
