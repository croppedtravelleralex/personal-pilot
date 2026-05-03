package proxy

import (
	"fmt"
	"net/url"
	"strconv"
	"strings"

	"gopkg.in/yaml.v3"
)

// IsSingBoxProtocol reports whether a node should be bridged by sing-box.
func IsSingBoxProtocol(proxyConfig string) bool {
	l := strings.ToLower(strings.TrimSpace(proxyConfig))
	if strings.HasPrefix(l, "hysteria2://") || strings.HasPrefix(l, "hysteria://") || strings.HasPrefix(l, "anytls://") || strings.HasPrefix(l, "tuic://") {
		return true
	}
	// Clash YAML 鏍煎紡
	if strings.Contains(l, "type: hysteria2") || strings.Contains(l, "type:hysteria2") ||
		strings.Contains(l, "type: hysteria") || strings.Contains(l, "type:hysteria") ||
		strings.Contains(l, "type: tuic") || strings.Contains(l, "type:tuic") ||
		strings.Contains(l, "type: anytls") || strings.Contains(l, "type:anytls") {
		return true
	}
	return false
}

// BuildSingBoxOutbound 瑙ｆ瀽鑺傜偣閰嶇疆锛岃繑鍥?sing-box outbound map
func BuildSingBoxOutbound(node string) (map[string]interface{}, error) {
	src := strings.TrimSpace(node)
	l := strings.ToLower(src)

	if strings.HasPrefix(l, "hysteria2://") || strings.HasPrefix(l, "hysteria://") {
		return parseHysteria2URI(src)
	}

	if strings.HasPrefix(l, "anytls://") {
		return parseAnytlsURI(src)
	}

	if strings.HasPrefix(l, "tuic://") {
		return parseTUICURI(src)
	}

	// Clash YAML 鏍煎紡
	if strings.Contains(l, "type:") || strings.Contains(l, "proxies:") {
		return parseClashSingBoxNode(src)
	}

	return nil, fmt.Errorf("涓嶆敮鎸佺殑 sing-box 鑺傜偣鏍煎紡")
}

// parseTUICURI parses tuic://uuid:password@host:port?sni=xxx&insecure=1.
func parseTUICURI(node string) (map[string]interface{}, error) {
	u, err := url.Parse(node)
	if err != nil {
		return nil, fmt.Errorf("tuic URI 鐟欙絾鐎芥径杈Е: %v", err)
	}

	host := u.Hostname()
	port, _ := strconv.Atoi(u.Port())
	uuid := u.User.Username()
	password, _ := u.User.Password()
	q := u.Query()
	sni := firstNonEmpty(q.Get("sni"), q.Get("peer"))
	insecure := q.Get("insecure") == "1" || strings.EqualFold(q.Get("insecure"), "true")
	congestionControl := firstNonEmpty(q.Get("congestion_control"), q.Get("congestion-control"), "bbr")

	if host == "" || port == 0 || uuid == "" {
		return nil, fmt.Errorf("tuic 閼哄倻鍋ｆ穱鈩冧紖娑撳秴鐣弫? host=%s port=%d", host, port)
	}

	tls := map[string]interface{}{
		"enabled":  true,
		"insecure": insecure,
	}
	if sni != "" {
		tls["server_name"] = sni
	}
	if alpn := q.Get("alpn"); alpn != "" {
		tls["alpn"] = splitCSV(alpn)
	}

	return map[string]interface{}{
		"type":               "tuic",
		"tag":                "proxy-out",
		"server":             host,
		"server_port":        port,
		"uuid":               uuid,
		"password":           password,
		"congestion_control": congestionControl,
		"tls":                tls,
	}, nil
}

// parseHysteria2URI 瑙ｆ瀽 hysteria2:// URI
// 鏍煎紡: hysteria2://password@host:port?sni=xxx&insecure=1
func parseHysteria2URI(node string) (map[string]interface{}, error) {
	// 缁熶竴涓?hysteria2://
	if strings.HasPrefix(strings.ToLower(node), "hysteria://") {
		node = "hysteria2://" + node[len("hysteria://"):]
	}

	u, err := url.Parse(node)
	if err != nil {
		return nil, fmt.Errorf("hysteria2 URI 瑙ｆ瀽澶辫触: %v", err)
	}

	host := u.Hostname()
	portStr := u.Port()
	port, _ := strconv.Atoi(portStr)
	password := u.User.Username()
	if password == "" {
		// 鏈変簺鏍煎紡鎶婂瘑鐮佹斁鍦?userinfo 閲屼笉甯?@
		password = strings.TrimPrefix(u.Host, "@")
	}

	q := u.Query()
	sni := q.Get("sni")
	if sni == "" {
		sni = q.Get("peer")
	}
	insecure := q.Get("insecure") == "1" || strings.ToLower(q.Get("insecure")) == "true"
	obfsPassword := q.Get("obfs-password")

	if host == "" || port == 0 {
		return nil, fmt.Errorf("hysteria2 鑺傜偣淇℃伅涓嶅畬鏁? host=%s port=%d", host, port)
	}

	out := map[string]interface{}{
		"type":        "hysteria2",
		"tag":         "proxy-out",
		"server":      host,
		"server_port": port,
		"password":    password,
		"tls": map[string]interface{}{
			"enabled":  true,
			"insecure": insecure,
		},
	}

	if sni != "" {
		out["tls"].(map[string]interface{})["server_name"] = sni
	}

	if obfsPassword != "" {
		out["obfs"] = map[string]interface{}{
			"type":     "salamander",
			"password": obfsPassword,
		}
	}

	return out, nil
}

// parseAnytlsURI 瑙ｆ瀽 anytls:// URI
// 鏍煎紡: anytls://password@host:port?sni=xxx&insecure=1
func parseAnytlsURI(node string) (map[string]interface{}, error) {
	u, err := url.Parse(node)
	if err != nil {
		return nil, fmt.Errorf("anytls URI 瑙ｆ瀽澶辫触: %v", err)
	}

	host := u.Hostname()
	portStr := u.Port()
	port, _ := strconv.Atoi(portStr)
	password := u.User.Username()

	q := u.Query()
	sni := q.Get("sni")
	if sni == "" {
		sni = q.Get("peer")
	}
	insecure := q.Get("insecure") == "1" || strings.ToLower(q.Get("insecure")) == "true"

	if host == "" || port == 0 {
		return nil, fmt.Errorf("anytls 鑺傜偣淇℃伅涓嶅畬鏁? host=%s port=%d", host, port)
	}

	out := map[string]interface{}{
		"type":        "anytls",
		"tag":         "proxy-out",
		"server":      host,
		"server_port": port,
		"password":    password,
		"tls": map[string]interface{}{
			"enabled":  true,
			"insecure": insecure,
		},
	}

	if sni != "" {
		out["tls"].(map[string]interface{})["server_name"] = sni
	}

	return out, nil
}

// parseClashSingBoxNode 瑙ｆ瀽 Clash YAML 鏍煎紡鐨?sing-box 鑺傜偣
func parseClashSingBoxNode(src string) (map[string]interface{}, error) {
	// 澶嶇敤宸叉湁鐨?YAML 瑙ｆ瀽鍩虹璁炬柦
	var payload interface{}
	if err := yaml.Unmarshal([]byte(src), &payload); err != nil {
		return nil, fmt.Errorf("YAML 瑙ｆ瀽澶辫触: %v", err)
	}

	nodeMap := pickClashNode(payload)
	if nodeMap == nil {
		return nil, fmt.Errorf("鑺傜偣瑙ｆ瀽澶辫触")
	}

	nodeType := strings.ToLower(getMapString(nodeMap, "type"))
	switch nodeType {
	case "hysteria2", "hysteria":
		return buildSingBoxHysteria2FromClash(nodeMap)
	case "tuic":
		return buildSingBoxTUICFromClash(nodeMap)
	case "anytls":
		return buildSingBoxAnytlsFromClash(nodeMap)
	default:
		return nil, fmt.Errorf("涓嶆敮鎸佺殑 sing-box 鑺傜偣绫诲瀷: %s", nodeType)
	}
}

func buildSingBoxHysteria2FromClash(node map[string]interface{}) (map[string]interface{}, error) {
	host := getMapString(node, "server")
	port := getMapInt(node, "port")
	password := getMapString(node, "password")
	sni := getMapString(node, "sni")
	if sni == "" {
		sni = getMapString(node, "servername")
	}
	skipVerify := getMapBool(node, "skip-cert-verify")

	if host == "" || port == 0 {
		return nil, fmt.Errorf("incomplete hysteria2 node")
	}

	tls := map[string]interface{}{
		"enabled":  true,
		"insecure": skipVerify,
	}
	if sni != "" {
		tls["server_name"] = sni
	}

	out := map[string]interface{}{
		"type":        "hysteria2",
		"tag":         "proxy-out",
		"server":      host,
		"server_port": port,
		"password":    password,
		"tls":         tls,
	}

	// 甯﹀闄愬埗锛堝彲閫夛級
	if up := getMapString(node, "up"); up != "" {
		out["up_mbps"] = parseBandwidthMbps(up)
	}
	if down := getMapString(node, "down"); down != "" {
		out["down_mbps"] = parseBandwidthMbps(down)
	}

	// obfs
	if obfsPassword := getMapString(node, "obfs-password"); obfsPassword != "" {
		out["obfs"] = map[string]interface{}{
			"type":     "salamander",
			"password": obfsPassword,
		}
	}

	return out, nil
}

func buildSingBoxAnytlsFromClash(node map[string]interface{}) (map[string]interface{}, error) {
	host := getMapString(node, "server")
	port := getMapInt(node, "port")
	password := getMapString(node, "password")
	if password == "" {
		password = getMapString(node, "uuid")
	}
	sni := getMapString(node, "sni")
	if sni == "" {
		sni = getMapString(node, "servername")
	}
	skipVerify := getMapBool(node, "skip-cert-verify")

	if host == "" || port == 0 {
		return nil, fmt.Errorf("incomplete anytls node")
	}

	tls := map[string]interface{}{
		"enabled":  true,
		"insecure": skipVerify,
	}
	if sni != "" {
		tls["server_name"] = sni
	}

	return map[string]interface{}{
		"type":        "anytls",
		"tag":         "proxy-out",
		"server":      host,
		"server_port": port,
		"password":    password,
		"tls":         tls,
	}, nil
}

func buildSingBoxTUICFromClash(node map[string]interface{}) (map[string]interface{}, error) {
	host := getMapString(node, "server")
	port := getMapInt(node, "port")
	uuid := getMapString(node, "uuid")
	password := getMapString(node, "password")
	sni := getMapString(node, "sni")
	skipVerify := getMapBool(node, "skip-cert-verify")

	if host == "" || port == 0 {
		return nil, fmt.Errorf("incomplete tuic node")
	}

	tls := map[string]interface{}{
		"enabled":  true,
		"insecure": skipVerify,
	}
	if sni != "" {
		tls["server_name"] = sni
	}

	// alpn
	if alpnRaw, ok := node["alpn"]; ok {
		if alpnList := toStringSlice(alpnRaw); len(alpnList) > 0 {
			tls["alpn"] = alpnList
		}
	}

	return map[string]interface{}{
		"type":               "tuic",
		"tag":                "proxy-out",
		"server":             host,
		"server_port":        port,
		"uuid":               uuid,
		"password":           password,
		"congestion_control": "bbr",
		"tls":                tls,
	}, nil
}

// parseBandwidthMbps 瑙ｆ瀽甯﹀瀛楃涓诧紝杩斿洖 Mbps 鏁存暟
// 鏀寔: "100 Mbps", "100", "100M"
func parseBandwidthMbps(s string) int {
	s = strings.TrimSpace(s)
	s = strings.ToUpper(s)
	s = strings.ReplaceAll(s, " ", "")
	s = strings.TrimSuffix(s, "BPS")
	s = strings.TrimSuffix(s, "B")
	s = strings.TrimSuffix(s, "M")
	n, _ := strconv.Atoi(s)
	return n
}

// toStringSlice 灏?interface{} 杞负 []string
func splitCSV(raw string) []string {
	parts := strings.Split(raw, ",")
	out := make([]string, 0, len(parts))
	for _, part := range parts {
		item := strings.TrimSpace(part)
		if item != "" {
			out = append(out, item)
		}
	}
	return out
}
func toStringSlice(v interface{}) []string {
	if v == nil {
		return nil
	}
	if arr, ok := v.([]interface{}); ok {
		result := make([]string, 0, len(arr))
		for _, item := range arr {
			if s, ok := item.(string); ok {
				result = append(result, s)
			}
		}
		return result
	}
	if s, ok := v.(string); ok && s != "" {
		return []string{s}
	}
	return nil
}
