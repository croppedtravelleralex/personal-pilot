package trust

import (
	"encoding/json"
	"fmt"
	"strings"
)

// CDPExec runs a CDP method and returns the raw result payload.
type CDPExec func(method string, params map[string]interface{}) ([]byte, error)

// HydrateResult summarizes TrustSurface application via CDP.
type HydrateResult struct {
	CookiesSet         int      `json:"cookiesSet"`
	LocalStorageKeys   int      `json:"localStorageKeys"`
	SessionStorageKeys int      `json:"sessionStorageKeys"`
	Errors             []string `json:"errors,omitempty"`
}

// HydrateCDP applies cookies and web storage from a TrustSurface.
// Local/session storage require the page to already be on a matching origin.
func HydrateCDP(exec CDPExec, surface TrustSurface) HydrateResult {
	var out HydrateResult
	if exec == nil {
		out.Errors = append(out.Errors, "cdp exec nil")
		return out
	}
	_, _ = exec("Network.enable", map[string]interface{}{})
	for _, c := range surface.Cookies {
		params := cookieMapToSetCookieParams(c)
		if params == nil {
			continue
		}
		if _, err := exec("Network.setCookie", params); err != nil {
			out.Errors = append(out.Errors, fmt.Sprintf("cookie %v: %v", params["name"], err))
			continue
		}
		out.CookiesSet++
	}
	if n, err := setWebStorage(exec, "localStorage", surface.LocalStorage); err != nil {
		out.Errors = append(out.Errors, err.Error())
	} else {
		out.LocalStorageKeys = n
	}
	if n, err := setWebStorage(exec, "sessionStorage", surface.SessionStorage); err != nil {
		out.Errors = append(out.Errors, err.Error())
	} else {
		out.SessionStorageKeys = n
	}
	return out
}

func cookieMapToSetCookieParams(c map[string]interface{}) map[string]interface{} {
	if c == nil {
		return nil
	}
	name, _ := c["name"].(string)
	value, _ := c["value"].(string)
	if strings.TrimSpace(name) == "" {
		return nil
	}
	params := map[string]interface{}{
		"name":  name,
		"value": value,
		"path":  "/",
	}
	if domain, ok := c["domain"].(string); ok && strings.TrimSpace(domain) != "" {
		params["domain"] = domain
	}
	if path, ok := c["path"].(string); ok && strings.TrimSpace(path) != "" {
		params["path"] = path
	}
	if secure, ok := c["secure"].(bool); ok {
		params["secure"] = secure
	}
	if httpOnly, ok := c["httpOnly"].(bool); ok {
		params["httpOnly"] = httpOnly
	}
	if httpOnly, ok := c["http_only"].(bool); ok {
		params["httpOnly"] = httpOnly
	}
	return params
}

func setWebStorage(exec CDPExec, storageName string, items map[string]string) (int, error) {
	if len(items) == 0 {
		return 0, nil
	}
	if storageName != "localStorage" && storageName != "sessionStorage" {
		return 0, fmt.Errorf("unsupported storage %q", storageName)
	}
	payload, err := json.Marshal(items)
	if err != nil {
		return 0, err
	}
	js := fmt.Sprintf(`(function(items){
		var store = window.%s;
		if (!store) return 0;
		var n = 0;
		Object.keys(items || {}).forEach(function(key) {
			store.setItem(String(key), String(items[key]));
			n++;
		});
		return n;
	})(%s)`, storageName, string(payload))
	raw, err := exec("Runtime.evaluate", map[string]interface{}{
		"expression":    js,
		"returnByValue": true,
	})
	if err != nil {
		return 0, fmt.Errorf("set %s: %w", storageName, err)
	}
	_ = raw
	return len(items), nil
}
