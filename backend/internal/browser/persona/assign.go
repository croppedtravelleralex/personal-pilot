package persona

import (
	"hash/fnv"
	"strings"
)

// AssignPersonaID deterministically picks a persona from the library using humanizeSeed.
func AssignPersonaID(humanizeSeed string) string {
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
	idx := int(h.Sum32() % uint32(len(lib)))
	return lib[idx].ID
}

// Resolve returns a persona by ID, or nil.
func Resolve(personaID string) *DevicePersona {
	want := strings.TrimSpace(personaID)
	if want == "" {
		return nil
	}
	for i := range Library() {
		p := Library()[i]
		if p.ID == want {
			cp := p
			return &cp
		}
	}
	return nil
}

// ResolveOrAssign returns an existing persona or assigns from seed.
func ResolveOrAssign(personaID, humanizeSeed string) *DevicePersona {
	if p := Resolve(personaID); p != nil {
		return p
	}
	return Resolve(AssignPersonaID(humanizeSeed))
}
