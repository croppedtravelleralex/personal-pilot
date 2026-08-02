package browser

import (
	"personal-pilot/backend/internal/config"
	"strings"
)

func normalizeCoreKind(kind string) string {
	kind = strings.ToLower(strings.TrimSpace(kind))
	switch kind {
	case config.CoreKindLightpanda:
		return config.CoreKindLightpanda
	case config.CoreKindCamoufox:
		return config.CoreKindCamoufox
	default:
		return config.CoreKindChromium
	}
}
