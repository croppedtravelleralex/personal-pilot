package browser

// FingerprintFieldPriorityLayer classifies fingerprint fields by importance.
type FingerprintFieldPriorityLayer int

const (
	FPLayerL1 FingerprintFieldPriorityLayer = iota // must be consumed first
	FPLayerL2                                       // high-value next layer
	FPLayerL3                                       // advanced fingerprint layer
)

// FingerprintPerfBudgetTag maps layers to performance budgets.
type FingerprintPerfBudgetTag int

const (
	FPBudgetLight  FingerprintPerfBudgetTag = 0
	FPBudgetMedium FingerprintPerfBudgetTag = 1
	FPBudgetHeavy  FingerprintPerfBudgetTag = 2
)

// FingerprintFieldPriorityRule maps a field to its priority layer with a reason.
type FingerprintFieldPriorityRule struct {
	Field  string
	Layer  FingerprintFieldPriorityLayer
	Reason string
}

// FingerprintFieldPriorityRules returns the canonical priority rules for all 15 fingerprint fields.
func FingerprintFieldPriorityRules() []FingerprintFieldPriorityRule {
	return []FingerprintFieldPriorityRule{
		{Field: "user_agent", Layer: FPLayerL1, Reason: "must be truly consumed first"},
		{Field: "accept_language", Layer: FPLayerL1, Reason: "must be truly consumed first"},
		{Field: "locale", Layer: FPLayerL1, Reason: "must be truly consumed first"},
		{Field: "timezone", Layer: FPLayerL1, Reason: "must be truly consumed first"},
		{Field: "viewport", Layer: FPLayerL1, Reason: "must be truly consumed first"},
		{Field: "platform", Layer: FPLayerL1, Reason: "must be truly consumed first"},
		{Field: "client_hints", Layer: FPLayerL2, Reason: "high-value next layer"},
		{Field: "hardware_concurrency", Layer: FPLayerL2, Reason: "high-value next layer"},
		{Field: "device_memory", Layer: FPLayerL2, Reason: "high-value next layer"},
		{Field: "color_scheme", Layer: FPLayerL2, Reason: "high-value next layer"},
		{Field: "canvas", Layer: FPLayerL3, Reason: "advanced fingerprint layer"},
		{Field: "webgl", Layer: FPLayerL3, Reason: "advanced fingerprint layer"},
		{Field: "audio", Layer: FPLayerL3, Reason: "advanced fingerprint layer"},
		{Field: "fonts", Layer: FPLayerL3, Reason: "advanced fingerprint layer"},
		{Field: "anti_detection_flags", Layer: FPLayerL3, Reason: "advanced fingerprint layer"},
	}
}

// ClassifyFingerprintField returns the priority layer for a fingerprint field, or nil if unknown.
func ClassifyFingerprintField(field string) *FingerprintFieldPriorityLayer {
	for _, rule := range FingerprintFieldPriorityRules() {
		if rule.Field == field {
			layer := rule.Layer
			return &layer
		}
	}
	return nil
}

// DefaultPerfBudgetForLayer maps a priority layer to its default performance budget tag.
func DefaultPerfBudgetForLayer(layer FingerprintFieldPriorityLayer) FingerprintPerfBudgetTag {
	switch layer {
	case FPLayerL1:
		return FPBudgetLight
	case FPLayerL2:
		return FPBudgetMedium
	case FPLayerL3:
		return FPBudgetHeavy
	default:
		return FPBudgetLight
	}
}

// LayerForPerfBudget returns the recommended field layer that fits within the given budget.
// Light budget → L1 only, Medium → L1+L2, Heavy → all fields.
func LayerForPerfBudget(budget FingerprintPerfBudgetTag) FingerprintFieldPriorityLayer {
	switch budget {
	case FPBudgetLight:
		return FPLayerL1
	case FPBudgetMedium:
		return FPLayerL2
	default:
		return FPLayerL3
	}
}
