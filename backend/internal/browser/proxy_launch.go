package browser

import "strings"

// NormalizeProxyServerForBrowser converts bridge URLs into Chromium --proxy-server values.
// Chromium accepts socks5/http/https only; socks5h is a curl convention and causes
// ERR_NO_SUPPORTED_PROXIES. Remote DNS for local bridges is handled by sing-box plus
// --host-resolver-rules instead.
func NormalizeProxyServerForBrowser(proxyServer string) string {
	proxyServer = strings.TrimSpace(proxyServer)
	if proxyServer == "" {
		return proxyServer
	}
	if strings.HasPrefix(proxyServer, "socks5h://") {
		proxyServer = "socks5://" + strings.TrimPrefix(proxyServer, "socks5h://")
	}
	if strings.HasPrefix(proxyServer, "socks5://localhost:") {
		proxyServer = strings.Replace(proxyServer, "socks5://localhost:", "socks5://127.0.0.1:", 1)
	}
	return proxyServer
}

// AppendProxyHardeningArgs adds Chrome flags that reduce DNS and WebRTC leaks when
// traffic is routed through a proxy. Call after fingerprint/launch args are merged.
func AppendProxyHardeningArgs(args []string, proxyServer string) []string {
	proxyServer = strings.TrimSpace(proxyServer)
	if proxyServer == "" || proxyServer == "direct://" {
		return args
	}
	if !hasLaunchArgPrefix(args, "--host-resolver-rules") {
		args = append(args, `--host-resolver-rules=MAP * ~NOTFOUND , EXCLUDE 127.0.0.1`)
	}
	if !hasLaunchArgPrefix(args, "--webrtc-ip-handling-policy") {
		args = append(args, "--webrtc-ip-handling-policy=disable_non_proxied_udp")
	}
	return args
}

func hasLaunchArgPrefix(args []string, prefix string) bool {
	for _, arg := range args {
		if strings.HasPrefix(strings.TrimSpace(arg), prefix) {
			return true
		}
	}
	return false
}
