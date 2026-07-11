package proxy

import (
	"fmt"
	"net"
	"net/url"
	"regexp"
	"strconv"
	"strings"
)

var sshTargetPattern = regexp.MustCompile(`^[A-Za-z0-9._@:-]+$`)

type SSHTunnelDirective struct {
	Target        string
	SSHPort       int
	RemoteHost    string
	RemotePort    int
	UpstreamHost  string
	CleanProxyURL string
}

// HasSSHTunnelDirective reports whether a standard proxy URL requests a local
// SSH forward before dialing the upstream proxy.
func HasSSHTunnelDirective(proxyConfig string) bool {
	_, ok, _ := ParseSSHTunnelDirective(proxyConfig)
	return ok
}

// ParseSSHTunnelDirective reads PersonalPilot-only URL parameters:
//
//	?pp_via_ssh=panda
//	?pp_ssh_port=22
//
// The returned CleanProxyURL has these parameters removed.
func ParseSSHTunnelDirective(proxyConfig string) (SSHTunnelDirective, bool, error) {
	src := NormalizeStandardProxyScheme(proxyConfig)
	if strings.TrimSpace(src) == "" {
		return SSHTunnelDirective{}, false, nil
	}
	u, err := url.Parse(src)
	if err != nil {
		return SSHTunnelDirective{}, false, err
	}
	scheme := strings.ToLower(u.Scheme)
	if scheme != "http" && scheme != "https" && scheme != "socks5" {
		return SSHTunnelDirective{}, false, nil
	}

	query := u.Query()
	target := strings.TrimSpace(firstNonEmpty(query.Get("pp_via_ssh"), query.Get("pp_ssh"), query.Get("via_ssh")))
	if target == "" {
		return SSHTunnelDirective{}, false, nil
	}
	if err := validateSSHTarget(target); err != nil {
		return SSHTunnelDirective{}, true, err
	}

	remoteHost := strings.TrimSpace(u.Hostname())
	remotePort, err := strconv.Atoi(u.Port())
	if remoteHost == "" || err != nil || remotePort <= 0 || remotePort > 65535 {
		return SSHTunnelDirective{}, true, fmt.Errorf("SSH 隧道代理地址缺少有效 host/port")
	}

	sshPort := 0
	if rawPort := strings.TrimSpace(query.Get("pp_ssh_port")); rawPort != "" {
		parsedPort, parseErr := strconv.Atoi(rawPort)
		if parseErr != nil || parsedPort <= 0 || parsedPort > 65535 {
			return SSHTunnelDirective{}, true, fmt.Errorf("pp_ssh_port 无效: %s", rawPort)
		}
		sshPort = parsedPort
	}

	cleanQuery := u.Query()
	for _, key := range []string{"pp_via_ssh", "pp_ssh", "via_ssh", "pp_ssh_port"} {
		cleanQuery.Del(key)
	}
	u.RawQuery = cleanQuery.Encode()
	u.Fragment = ""

	return SSHTunnelDirective{
		Target:        target,
		SSHPort:       sshPort,
		RemoteHost:    remoteHost,
		RemotePort:    remotePort,
		UpstreamHost:  remoteHost,
		CleanProxyURL: u.String(),
	}, true, nil
}

func validateSSHTarget(target string) error {
	if target == "" {
		return fmt.Errorf("SSH 目标不能为空")
	}
	if len(target) > 128 {
		return fmt.Errorf("SSH 目标过长")
	}
	if strings.HasPrefix(target, "-") {
		return fmt.Errorf("SSH 目标不能以 '-' 开头")
	}
	if strings.ContainsAny(target, " \t\r\n\"'`;$&|<>") {
		return fmt.Errorf("SSH 目标包含不允许的字符")
	}
	if !sshTargetPattern.MatchString(target) {
		return fmt.Errorf("SSH 目标格式不支持: %s", target)
	}
	return nil
}

func buildLocalForwardSpec(localPort int, remoteHost string, remotePort int) string {
	forwardHost := remoteHost
	if strings.Contains(remoteHost, ":") && !strings.HasPrefix(remoteHost, "[") {
		forwardHost = "[" + remoteHost + "]"
	}
	return fmt.Sprintf("127.0.0.1:%d:%s:%d", localPort, forwardHost, remotePort)
}

func rewriteProxyURLToLocalForward(cleanProxyURL string, localPort int) (string, error) {
	u, err := url.Parse(NormalizeStandardProxyScheme(cleanProxyURL))
	if err != nil {
		return "", err
	}
	u.Host = net.JoinHostPort("127.0.0.1", strconv.Itoa(localPort))
	u.Fragment = ""
	return u.String(), nil
}
