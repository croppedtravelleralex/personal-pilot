package persona

import (
	"hash/fnv"
	"strings"
)

// FeatureKey returns a coarse persona fingerprint for diversity checks.
func FeatureKey(p DevicePersona) string {
	return strings.Join([]string{
		strings.ToLower(p.Platform),
		p.GPUVendor,
		p.GPURenderer,
		itoa(p.ScreenWidth) + "x" + itoa(p.ScreenHeight),
		itoa(p.HardwareConcurrency),
	}, "|")
}

func itoa(n int) string {
	if n == 0 {
		return "0"
	}
	neg := n < 0
	if neg {
		n = -n
	}
	var b [20]byte
	i := len(b)
	for n > 0 {
		i--
		b[i] = byte('0' + n%10)
		n /= 10
	}
	if neg {
		i--
		b[i] = '-'
	}
	return string(b[i:])
}

// AssignPersonaIDDiverse picks a persona avoiding overused feature keys (docs/52 DP5).
// maxPerFeature caps how many profiles may share the same coarse feature key.
func AssignPersonaIDDiverse(humanizeSeed string, usedFeatureCounts map[string]int, maxPerFeature int) string {
	if maxPerFeature <= 0 {
		maxPerFeature = 3
	}
	lib := Library()
	if len(lib) == 0 {
		return ""
	}
	seed := strings.TrimSpace(humanizeSeed)
	if seed == "" {
		seed = "default"
	}
	h := fnv.New32a()
	_, _ = h.Write([]byte(seed))
	start := int(h.Sum32() % uint32(len(lib)))
	for offset := 0; offset < len(lib); offset++ {
		p := lib[(start+offset)%len(lib)]
		key := FeatureKey(p)
		if usedFeatureCounts == nil || usedFeatureCounts[key] < maxPerFeature {
			return p.ID
		}
	}
	// Pool saturated: fall back to deterministic assign.
	return AssignPersonaID(seed)
}
