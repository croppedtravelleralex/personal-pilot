package behavior

import (
	"encoding/json"
	"fmt"
	"hash/fnv"
	"sort"
	"strings"
)

// EnvironmentInjectionProfile is the serializable subset of browser environment
// controls that can be injected before page scripts run.
type EnvironmentInjectionProfile struct {
	Seed                string            `json:"seed"`
	Languages           []string          `json:"languages"`
	Platform            string            `json:"platform"`
	Vendor              string            `json:"vendor"`
	UserAgent           string            `json:"userAgent"`
	HardwareConcurrency int               `json:"hardwareConcurrency"`
	DeviceMemory        int               `json:"deviceMemory"`
	Timezone            string            `json:"timezone"`
	TimezoneOffset      *int              `json:"timezoneOffset"`
	AcceptLanguage      string            `json:"acceptLanguage"`
	WebGLVendor         string            `json:"webglVendor"`
	WebGLRenderer       string            `json:"webglRenderer"`
	Plugins             []PluginInjection `json:"plugins"`
	MediaDevices        []MediaDeviceSpec `json:"mediaDevices"`
	FontAllowlist       []string          `json:"fontAllowlist"`
	Headers             map[string]string `json:"headers"`
}

type PluginInjection struct {
	Name        string `json:"name"`
	Filename    string `json:"filename"`
	Description string `json:"description"`
	MimeType    string `json:"mimeType"`
}

type MediaDeviceSpec struct {
	Kind    string `json:"kind"`
	Label   string `json:"label"`
	GroupID string `json:"groupId"`
	ID      string `json:"id"`
}

type EnvironmentInjectionPlan struct {
	Script          string            `json:"script"`
	HeaderOverrides map[string]string `json:"headerOverrides"`
	AppliedFamilies []string          `json:"appliedFamilies"`
	Warnings        []string          `json:"warnings"`
}

// CompileEnvironmentInjectionScript compiles profile controls into one script so
// CDP can inject it with a single Page.addScriptToEvaluateOnNewDocument call.
func CompileEnvironmentInjectionScript(profile EnvironmentInjectionProfile) (EnvironmentInjectionPlan, error) {
	normalized := normalizeEnvironmentProfile(profile)
	encoded, err := json.Marshal(normalized)
	if err != nil {
		return EnvironmentInjectionPlan{}, fmt.Errorf("marshal environment profile: %w", err)
	}

	warnings := environmentInjectionWarnings(normalized)
	script := strings.TrimSpace(fmt.Sprintf(`(() => {
  const profile = %s;
  const defineGetter = (target, key, value) => {
    try { Object.defineProperty(target, key, { get: () => value, configurable: true }); } catch (_) {}
  };
  const stableNoise = (salt) => {
    let h = 2166136261;
    const text = String(profile.seed || "personal-pilot") + ":" + salt;
    for (let i = 0; i < text.length; i++) { h ^= text.charCodeAt(i); h = Math.imul(h, 16777619); }
    return ((h >>> 0) %% 997) / 997000;
  };

  defineGetter(Navigator.prototype, 'webdriver', undefined);
  if (profile.languages && profile.languages.length) defineGetter(Navigator.prototype, 'languages', Object.freeze(profile.languages.slice()));
  if (profile.platform) defineGetter(Navigator.prototype, 'platform', profile.platform);
  if (profile.vendor) defineGetter(Navigator.prototype, 'vendor', profile.vendor);
  if (profile.userAgent) defineGetter(Navigator.prototype, 'userAgent', profile.userAgent);
  if (profile.hardwareConcurrency > 0) defineGetter(Navigator.prototype, 'hardwareConcurrency', profile.hardwareConcurrency);
  if (profile.deviceMemory > 0) defineGetter(Navigator.prototype, 'deviceMemory', profile.deviceMemory);

  const plugins = (profile.plugins || []).map((p) => ({ name: p.name, filename: p.filename, description: p.description }));
  defineGetter(Navigator.prototype, 'plugins', Object.freeze(plugins));
  defineGetter(Navigator.prototype, 'mimeTypes', Object.freeze((profile.plugins || []).filter((p) => p.mimeType).map((p) => ({ type: p.mimeType, enabledPlugin: p.name }))));

  if (profile.timezone && Intl && Intl.DateTimeFormat) {
    const originalResolvedOptions = Intl.DateTimeFormat.prototype.resolvedOptions;
    Intl.DateTimeFormat.prototype.resolvedOptions = function() {
      const value = originalResolvedOptions.call(this);
      return Object.assign({}, value, { timeZone: profile.timezone, locale: (profile.languages && profile.languages[0]) || value.locale });
    };
  }
  if (Number.isFinite(profile.timezoneOffset)) {
    Date.prototype.getTimezoneOffset = function() { return profile.timezoneOffset; };
  }

  const canvasNoise = (data, salt) => {
    const shift = stableNoise(salt);
    for (let i = 0; i < data.length; i += 4) data[i] = Math.max(0, Math.min(255, data[i] + shift));
  };
  if (CanvasRenderingContext2D && CanvasRenderingContext2D.prototype) {
    const originalGetImageData = CanvasRenderingContext2D.prototype.getImageData;
    CanvasRenderingContext2D.prototype.getImageData = function(...args) {
      const image = originalGetImageData.apply(this, args);
      canvasNoise(image.data, 'getImageData');
      return image;
    };
    const originalFillText = CanvasRenderingContext2D.prototype.fillText;
    CanvasRenderingContext2D.prototype.fillText = function(text, x, y, ...rest) {
      return originalFillText.call(this, text, x + stableNoise('fillText'), y, ...rest);
    };
  }
  if (HTMLCanvasElement && HTMLCanvasElement.prototype) {
    const originalToDataURL = HTMLCanvasElement.prototype.toDataURL;
    HTMLCanvasElement.prototype.toDataURL = function(...args) {
      try {
        const ctx = this.getContext('2d');
        if (ctx) {
          const image = ctx.getImageData(0, 0, Math.min(this.width, 64), Math.min(this.height, 64));
          canvasNoise(image.data, 'toDataURL');
          ctx.putImageData(image, 0, 0);
        }
      } catch (_) {}
      return originalToDataURL.apply(this, args);
    };
  }

  const webglPatch = (proto) => {
    if (!proto) return;
    const originalGetParameter = proto.getParameter;
    proto.getParameter = function(parameter) {
      if (parameter === 37445 && profile.webglVendor) return profile.webglVendor;
      if (parameter === 37446 && profile.webglRenderer) return profile.webglRenderer;
      return originalGetParameter.call(this, parameter);
    };
  };
  webglPatch(window.WebGLRenderingContext && WebGLRenderingContext.prototype);
  webglPatch(window.WebGL2RenderingContext && WebGL2RenderingContext.prototype);

  if (navigator.mediaDevices && navigator.mediaDevices.enumerateDevices) {
    navigator.mediaDevices.enumerateDevices = async () => (profile.mediaDevices || []).map((d) => ({
      kind: d.kind, label: d.label, groupId: d.groupId, deviceId: d.id
    }));
  }
})();`, encoded))

	return EnvironmentInjectionPlan{
		Script:          script,
		HeaderOverrides: normalized.Headers,
		AppliedFamilies: []string{"browser_api_surface", "canvas_rendering", "timezone_locale", "webgl_gpu", "fonts_text_metrics", "media_devices"},
		Warnings:        warnings,
	}, nil
}

func (e *CDPExecutor) ApplyEnvironmentInjection(profile EnvironmentInjectionProfile) (EnvironmentInjectionPlan, error) {
	plan, err := CompileEnvironmentInjectionScript(profile)
	if err != nil {
		return plan, err
	}
	if len(plan.HeaderOverrides) > 0 {
		if _, err := e.sendCommand("Network.setExtraHTTPHeaders", map[string]interface{}{"headers": plan.HeaderOverrides}); err != nil {
			return plan, fmt.Errorf("set environment headers: %w", err)
		}
	}
	if _, err := e.sendCommand("Page.addScriptToEvaluateOnNewDocument", map[string]interface{}{"source": plan.Script}); err != nil {
		return plan, fmt.Errorf("inject environment script: %w", err)
	}
	return plan, nil
}

func normalizeEnvironmentProfile(profile EnvironmentInjectionProfile) EnvironmentInjectionProfile {
	if strings.TrimSpace(profile.Seed) == "" {
		profile.Seed = "personal-pilot"
	}
	if len(profile.Languages) == 0 && strings.TrimSpace(profile.AcceptLanguage) != "" {
		profile.Languages = []string{strings.TrimSpace(strings.Split(profile.AcceptLanguage, ",")[0])}
	}
	if len(profile.Languages) == 0 {
		profile.Languages = []string{"en-US", "en"}
	}
	if profile.AcceptLanguage == "" {
		profile.AcceptLanguage = profile.Languages[0]
		if len(profile.Languages) > 1 {
			profile.AcceptLanguage += "," + profile.Languages[1] + ";q=0.9"
		}
	}
	if profile.Headers == nil {
		profile.Headers = map[string]string{}
	}
	profile.Headers["Accept-Language"] = profile.AcceptLanguage
	if profile.HardwareConcurrency < 0 {
		profile.HardwareConcurrency = 0
	}
	if profile.DeviceMemory < 0 {
		profile.DeviceMemory = 0
	}
	sort.Strings(profile.FontAllowlist)
	return profile
}

func environmentInjectionWarnings(profile EnvironmentInjectionProfile) []string {
	var warnings []string
	if profile.Timezone == "" {
		warnings = append(warnings, "timezone_not_configured")
	}
	if profile.Platform == "" || profile.UserAgent == "" || profile.Vendor == "" {
		warnings = append(warnings, "user_agent_platform_vendor_chain_incomplete")
	}
	if profile.WebGLVendor == "" || profile.WebGLRenderer == "" {
		warnings = append(warnings, "webgl_identity_incomplete")
	}
	return warnings
}

func stableProfileSeed(text string) uint64 {
	h := fnv.New64a()
	_, _ = h.Write([]byte(text))
	return h.Sum64()
}
