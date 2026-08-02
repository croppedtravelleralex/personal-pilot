package persona

import (
	"fmt"
	"strings"
	"time"
)

// EvolvePersona applies controlled fingerprint aging (docs/52 DP4).
// Hardware/GPU/screen stay fixed; fonts only grow; OS patch may tick slowly.
func EvolvePersona(base DevicePersona, bornAt, now time.Time) DevicePersona {
	out := base
	if bornAt.IsZero() || now.Before(bornAt) {
		return out
	}
	days := int(now.Sub(bornAt).Hours() / 24)
	if days < 90 {
		return out
	}
	extra := []string{"Segoe UI Symbol", "Segoe UI Emoji", "Microsoft YaHei UI", "PingFang SC"}
	steps := days / 90
	if steps > len(extra) {
		steps = len(extra)
	}
	have := map[string]bool{}
	for _, f := range out.FontAllowlist {
		have[f] = true
	}
	for i := 0; i < steps; i++ {
		f := extra[i]
		if !have[f] {
			out.FontAllowlist = append(out.FontAllowlist, f)
			have[f] = true
		}
	}
	if strings.EqualFold(out.Platform, "windows") && out.OSVersion != "" {
		parts := strings.Split(out.OSVersion, ".")
		if len(parts) >= 3 {
			patch := days / 180
			if patch > 9 {
				patch = 9
			}
			if patch > 0 {
				parts[2] = fmt.Sprintf("%d", patch)
				out.OSVersion = strings.Join(parts[:3], ".")
			}
		}
	}
	return out
}
