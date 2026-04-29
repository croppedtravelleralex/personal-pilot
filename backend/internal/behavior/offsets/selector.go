package offsets

import "sync/atomic"

// Selector manages round-robin selection of offset variants per category.
type Selector struct {
	counters map[string]*int32
}

// NewSelector creates a new offset variant selector.
func NewSelector() *Selector {
	counters := make(map[string]*int32)
	for cat := range CategoryHumanNames {
		var c int32
		counters[cat] = &c
	}
	return &Selector{counters: counters}
}

// Select returns the next offset variant for the given category using round-robin.
func (s *Selector) Select(category string) OffsetVariant {
	all := BuiltinOffsetLibrary()
	var catVariants []OffsetVariant
	for _, v := range all {
		if v.Category == category {
			catVariants = append(catVariants, v)
		}
	}
	if len(catVariants) == 0 {
		return OffsetVariant{} // Return zero value for unknown categories
	}

	counter, ok := s.counters[category]
	if !ok {
		var c int32
		counter = &c
		s.counters[category] = counter
	}

	idx := int(atomic.AddInt32(counter, 1)-1) % len(catVariants)
	return catVariants[idx]
}

// SelectAllForPlan assigns offset variants to every action in a plan, cycling
// within each category via round-robin.
func (s *Selector) SelectAllForPlan(actionCategories []string) []OffsetVariant {
	result := make([]OffsetVariant, len(actionCategories))
	for i, cat := range actionCategories {
		result[i] = s.Select(cat)
	}
	return result
}

// GetByID returns a specific offset variant by ID.
func GetByID(id string) (OffsetVariant, bool) {
	for _, v := range BuiltinOffsetLibrary() {
		if v.ID == id {
			return v, true
		}
	}
	return OffsetVariant{}, false
}

// GetByCategory returns all offset variants for a category.
func GetByCategory(category string) []OffsetVariant {
	var result []OffsetVariant
	for _, v := range BuiltinOffsetLibrary() {
		if v.Category == category {
			result = append(result, v)
		}
	}
	return result
}
