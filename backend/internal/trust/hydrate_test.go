package trust

import "testing"

func TestCookieMapToSetCookieParams(t *testing.T) {
	params := cookieMapToSetCookieParams(map[string]interface{}{
		"name": "a", "value": "b", "domain": ".example.com", "httpOnly": true,
	})
	if params["name"] != "a" || params["domain"] != ".example.com" || params["httpOnly"] != true {
		t.Fatalf("%v", params)
	}
	if cookieMapToSetCookieParams(nil) != nil {
		t.Fatal("nil cookie should return nil")
	}
}

func TestHydrateCDPCounts(t *testing.T) {
	calls := 0
	exec := func(method string, params map[string]interface{}) ([]byte, error) {
		calls++
		return []byte(`{"result":{"value":1}}`), nil
	}
	res := HydrateCDP(exec, TrustSurface{
		Cookies:      []map[string]interface{}{{"name": "c", "value": "1", "domain": ".x.com"}},
		LocalStorage: map[string]string{"k": "v"},
	})
	if res.CookiesSet != 1 || res.LocalStorageKeys != 1 {
		t.Fatalf("%+v calls=%d", res, calls)
	}
}
