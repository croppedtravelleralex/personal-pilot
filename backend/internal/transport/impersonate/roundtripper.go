package impersonate

import (
	"bufio"
	"context"
	"fmt"
	"net"
	"net/http"
	"strings"
	"time"

	utls "github.com/metacubex/utls"
	"golang.org/x/net/http2"
	xproxy "golang.org/x/net/proxy"

	"personal-pilot/backend/internal/transport"
)

// ResolveHelloID maps an egress ClientHello name to a utls preset.
func ResolveHelloID(name string) utls.ClientHelloID {
	switch strings.TrimSpace(name) {
	case "Chrome_133":
		return utls.HelloChrome_133
	case "Chrome_131":
		return utls.HelloChrome_131
	case "Chrome_120":
		return utls.HelloChrome_120
	case "Chrome_120_PQ":
		return utls.HelloChrome_120_PQ
	default:
		return utls.HelloChrome_Auto
	}
}

// RoundTripper dials via Dialer and completes TLS with a Chrome-like ClientHello.
type RoundTripper struct {
	Dialer  xproxy.ContextDialer
	HelloID utls.ClientHelloID
}

func (rt *RoundTripper) dialer() xproxy.ContextDialer {
	if rt != nil && rt.Dialer != nil {
		return rt.Dialer
	}
	return &net.Dialer{Timeout: 30 * time.Second}
}

func (rt *RoundTripper) hello() utls.ClientHelloID {
	if rt != nil && rt.HelloID.IsSet() {
		return rt.HelloID
	}
	return utls.HelloChrome_Auto
}

func (rt *RoundTripper) RoundTrip(req *http.Request) (*http.Response, error) {
	if req == nil || req.URL == nil {
		return nil, fmt.Errorf("impersonate: nil request")
	}
	if req.URL.Scheme != "https" {
		// Non-TLS: fall back to default transport semantics via plain dial.
		return rt.roundTripPlain(req)
	}
	host := req.URL.Hostname()
	port := req.URL.Port()
	if port == "" {
		port = "443"
	}
	addr := net.JoinHostPort(host, port)
	ctx := req.Context()
	if ctx == nil {
		ctx = context.Background()
	}
	raw, err := rt.dialer().DialContext(ctx, "tcp", addr)
	if err != nil {
		return nil, err
	}
	uconn := utls.UClient(raw, &utls.Config{ServerName: host}, rt.hello())
	if err := uconn.HandshakeContext(ctx); err != nil {
		_ = raw.Close()
		return nil, err
	}
	if uconn.ConnectionState().NegotiatedProtocol == "h2" {
		t2 := &http2.Transport{}
		cc, err := t2.NewClientConn(uconn)
		if err != nil {
			_ = uconn.Close()
			return nil, err
		}
		return cc.RoundTrip(req)
	}
	if err := req.Write(uconn); err != nil {
		_ = uconn.Close()
		return nil, err
	}
	resp, err := http.ReadResponse(bufio.NewReader(uconn), req)
	if err != nil {
		_ = uconn.Close()
		return nil, err
	}
	return resp, nil
}

func (rt *RoundTripper) roundTripPlain(req *http.Request) (*http.Response, error) {
	host := req.URL.Hostname()
	port := req.URL.Port()
	if port == "" {
		port = "80"
	}
	ctx := req.Context()
	if ctx == nil {
		ctx = context.Background()
	}
	conn, err := rt.dialer().DialContext(ctx, "tcp", net.JoinHostPort(host, port))
	if err != nil {
		return nil, err
	}
	if err := req.Write(conn); err != nil {
		_ = conn.Close()
		return nil, err
	}
	resp, err := http.ReadResponse(bufio.NewReader(conn), req)
	if err != nil {
		_ = conn.Close()
		return nil, err
	}
	return resp, nil
}

// NewClient builds an http.Client that impersonates Chrome TLS on every request.
func NewClient(dialer xproxy.ContextDialer, identity transport.EgressIdentity, timeout time.Duration) *http.Client {
	if timeout <= 0 {
		timeout = 30 * time.Second
	}
	return &http.Client{
		Timeout: timeout,
		Transport: &RoundTripper{
			Dialer:  dialer,
			HelloID: ResolveHelloID(identity.ClientHello),
		},
	}
}

// DirectDialer is a ContextDialer backed by net.Dialer.
func DirectDialer(timeout time.Duration) xproxy.ContextDialer {
	if timeout <= 0 {
		timeout = 30 * time.Second
	}
	return &net.Dialer{Timeout: timeout}
}
