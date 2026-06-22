package pool

import (
	"fmt"
	"time"
)

type SlotState string

const (
	SlotIdle    SlotState = "idle"
	SlotWarming SlotState = "warming"
	SlotLeased  SlotState = "leased"
	SlotFailed  SlotState = "failed"
)

type Slot struct {
	ID                  string
	ProfileID           string
	RouteTag            string
	State               SlotState
	Process             *ProcessAttachment
	ProxyID             string
	SessionBindingKey   string
	CreatedAt           time.Time
	UpdatedAt           time.Time
	LastLeasedAt        time.Time
	LastReleasedAt      time.Time
	LastCleanedAt       time.Time
	LeaseUntil          time.Time
	LeaseCount          uint32
	CleanupCount        uint32
	LastCleanupReason   string
	LastFailureReason   string
	LastResourceSummary string
}

type Policy struct {
	MaxSlots       uint32
	DefaultLease   time.Duration
	IdleTTL        time.Duration
	WarmupTimeout  time.Duration
	PrewarmEnabled bool
}

type Engine struct {
	Policy Policy
	Slots  []Slot
}

type ResourceUsage struct {
	Total         uint32
	Idle          uint32
	Warming       uint32
	Leased        uint32
	Failed        uint32
	ExpiredLeases uint32
	Capacity      uint32
}

type BudgetStatus struct {
	Status string
	Usage  ResourceUsage
	Reason string
}

type CleanupProof struct {
	SlotID        string
	FromState     SlotState
	ToState       SlotState
	Reason        string
	Cleaned       bool
	Removed       bool
	Process       *ProcessCleanupProof
	CleanedAt     time.Time
	LeaseCount    uint32
	CleanupCount  uint32
	ResourceUsage ResourceUsage
}

type CleanupReport struct {
	GeneratedAt    time.Time
	Proofs         []CleanupProof
	RemovedSlotIDs []string
	Usage          ResourceUsage
	Budget         BudgetStatus
}

type ProcessAttachment struct {
	RootPID           int
	ChildPIDs         []int
	CDPPort           int
	WorkingSetMB      float64
	AttachedAt        time.Time
	TempProfileDir    string
	ProxyID           string
	SessionBindingKey string
}

type ProcessCleanupProof struct {
	RootPID               int
	ObservedPIDs          []int
	RemainingPIDs         []int
	CDPReady              bool
	ProcessCount          int
	WorkingSetMB          float64
	CleanupComplete       bool
	TempProfileRemoved    bool
	ProxyBindingCleaned   bool
	SessionBindingCleaned bool
}

func (e *Engine) effectivePolicy() Policy {
	policy := e.Policy
	if policy.MaxSlots == 0 {
		policy.MaxSlots = 1
	}
	if policy.DefaultLease <= 0 {
		policy.DefaultLease = time.Minute
	}
	return policy
}

func (e *Engine) slotID(profileID string) string {
	return fmt.Sprintf("%s-slot-%d", profileID, len(e.Slots)+1)
}

func (e *Engine) Usage(now time.Time) ResourceUsage {
	policy := e.effectivePolicy()
	usage := ResourceUsage{Total: uint32(len(e.Slots)), Capacity: policy.MaxSlots}
	for _, slot := range e.Slots {
		switch slot.State {
		case SlotIdle:
			usage.Idle++
		case SlotWarming:
			usage.Warming++
		case SlotLeased:
			usage.Leased++
			if !slot.LeaseUntil.IsZero() && !now.Before(slot.LeaseUntil) {
				usage.ExpiredLeases++
			}
		case SlotFailed:
			usage.Failed++
		}
	}
	return usage
}

func (e *Engine) Budget(now time.Time) BudgetStatus {
	usage := e.Usage(now)
	switch {
	case usage.Total > usage.Capacity:
		return BudgetStatus{Status: "over_capacity", Usage: usage, Reason: "slot count exceeds MaxSlots"}
	case usage.ExpiredLeases > 0:
		return BudgetStatus{Status: "cleanup_required", Usage: usage, Reason: "expired leases require cleanup"}
	case usage.Failed > 0:
		return BudgetStatus{Status: "cleanup_required", Usage: usage, Reason: "failed slots require cleanup"}
	case usage.Total == usage.Capacity && usage.Idle == 0:
		return BudgetStatus{Status: "at_capacity", Usage: usage, Reason: "all slots are allocated"}
	default:
		return BudgetStatus{Status: "within_budget", Usage: usage, Reason: "slot usage is within policy"}
	}
}

func (e *Engine) Acquire(profileID string, now time.Time) (Slot, bool) {
	policy := e.effectivePolicy()
	if now.IsZero() {
		now = time.Now()
	}
	e.Cleanup(now)
	for i := range e.Slots {
		if e.Slots[i].State == SlotIdle || (e.Slots[i].State == SlotLeased && now.After(e.Slots[i].LeaseUntil)) {
			e.Slots[i].ProfileID = profileID
			e.Slots[i].State = SlotLeased
			e.Slots[i].UpdatedAt = now
			e.Slots[i].LastLeasedAt = now
			e.Slots[i].LeaseUntil = now.Add(policy.DefaultLease)
			e.Slots[i].LeaseCount++
			e.Slots[i].LastResourceSummary = e.Budget(now).Status
			return e.Slots[i], true
		}
	}
	if uint32(len(e.Slots)) < policy.MaxSlots {
		slot := Slot{
			ID:                  e.slotID(profileID),
			ProfileID:           profileID,
			State:               SlotLeased,
			CreatedAt:           now,
			UpdatedAt:           now,
			LastLeasedAt:        now,
			LeaseUntil:          now.Add(policy.DefaultLease),
			LeaseCount:          1,
			LastResourceSummary: e.Budget(now).Status,
		}
		e.Slots = append(e.Slots, slot)
		return slot, true
	}
	return Slot{}, false
}

func (e *Engine) Release(slotID string) bool {
	_, ok := e.ReleaseWithCleanup(slotID, time.Now(), "release")
	return ok
}

func (e *Engine) ReleaseWithCleanup(slotID string, now time.Time, reason string) (CleanupProof, bool) {
	if now.IsZero() {
		now = time.Now()
	}
	if reason == "" {
		reason = "release"
	}
	for i := range e.Slots {
		if e.Slots[i].ID == slotID {
			from := e.Slots[i].State
			process := e.Slots[i].Process
			e.Slots[i].State = SlotIdle
			e.Slots[i].ProfileID = ""
			e.Slots[i].Process = nil
			e.Slots[i].ProxyID = ""
			e.Slots[i].SessionBindingKey = ""
			e.Slots[i].UpdatedAt = now
			e.Slots[i].LastReleasedAt = now
			e.Slots[i].LastCleanedAt = now
			e.Slots[i].CleanupCount++
			e.Slots[i].LastCleanupReason = reason
			e.Slots[i].LeaseUntil = time.Time{}
			e.Slots[i].LastResourceSummary = e.Budget(now).Status
			proof := CleanupProof{
				SlotID:        slotID,
				FromState:     from,
				ToState:       SlotIdle,
				Reason:        reason,
				Cleaned:       true,
				CleanedAt:     now,
				LeaseCount:    e.Slots[i].LeaseCount,
				CleanupCount:  e.Slots[i].CleanupCount,
				ResourceUsage: e.Usage(now),
			}
			if process != nil {
				proof.Process = &ProcessCleanupProof{
					RootPID:               process.RootPID,
					ObservedPIDs:          append([]int{process.RootPID}, process.ChildPIDs...),
					ProcessCount:          1 + len(process.ChildPIDs),
					WorkingSetMB:          process.WorkingSetMB,
					CleanupComplete:       false,
					ProxyBindingCleaned:   process.ProxyID != "",
					SessionBindingCleaned: process.SessionBindingKey != "",
				}
			}
			return proof, true
		}
	}
	return CleanupProof{}, false
}

func (e *Engine) AttachProcess(slotID string, attachment ProcessAttachment, now time.Time) bool {
	if now.IsZero() {
		now = time.Now()
	}
	for i := range e.Slots {
		if e.Slots[i].ID == slotID {
			if attachment.AttachedAt.IsZero() {
				attachment.AttachedAt = now
			}
			e.Slots[i].Process = &attachment
			e.Slots[i].ProxyID = attachment.ProxyID
			e.Slots[i].SessionBindingKey = attachment.SessionBindingKey
			e.Slots[i].UpdatedAt = now
			e.Slots[i].LastResourceSummary = e.Budget(now).Status
			return true
		}
	}
	return false
}

func (e *Engine) ReleaseProcessWithCleanup(slotID string, now time.Time, reason string, processProof ProcessCleanupProof) (CleanupProof, bool) {
	proof, ok := e.ReleaseWithCleanup(slotID, now, reason)
	if !ok {
		return CleanupProof{}, false
	}
	proof.Process = &processProof
	return proof, true
}

func (e *Engine) MarkReady(slotID string, now time.Time) bool {
	if now.IsZero() {
		now = time.Now()
	}
	for i := range e.Slots {
		if e.Slots[i].ID == slotID {
			e.Slots[i].State = SlotIdle
			e.Slots[i].UpdatedAt = now
			e.Slots[i].LastReleasedAt = now
			e.Slots[i].LeaseUntil = time.Time{}
			e.Slots[i].LastFailureReason = ""
			e.Slots[i].LastResourceSummary = e.Budget(now).Status
			return true
		}
	}
	return false
}

func (e *Engine) MarkFailed(slotID string, now time.Time, reason string) bool {
	if now.IsZero() {
		now = time.Now()
	}
	if reason == "" {
		reason = "unknown"
	}
	for i := range e.Slots {
		if e.Slots[i].ID == slotID {
			e.Slots[i].State = SlotFailed
			e.Slots[i].UpdatedAt = now
			e.Slots[i].LastFailureReason = reason
			e.Slots[i].LastResourceSummary = e.Budget(now).Status
			return true
		}
	}
	return false
}

func (e *Engine) Cleanup(now time.Time) CleanupReport {
	if now.IsZero() {
		now = time.Now()
	}
	policy := e.effectivePolicy()
	report := CleanupReport{GeneratedAt: now}
	kept := e.Slots[:0]
	for i := range e.Slots {
		slot := e.Slots[i]
		proof := CleanupProof{}
		remove := false
		switch slot.State {
		case SlotLeased:
			if !slot.LeaseUntil.IsZero() && !now.Before(slot.LeaseUntil) {
				proof = cleanupProof(slot, SlotIdle, "lease_expired", now, e.Usage(now), false)
				slot.State = SlotIdle
				slot.ProfileID = ""
				slot.LeaseUntil = time.Time{}
				slot.LastReleasedAt = now
			}
		case SlotWarming:
			if policy.WarmupTimeout > 0 && !slot.UpdatedAt.IsZero() && now.Sub(slot.UpdatedAt) >= policy.WarmupTimeout {
				proof = cleanupProof(slot, SlotFailed, "warmup_timeout", now, e.Usage(now), false)
				slot.State = SlotFailed
				slot.LastFailureReason = "warmup_timeout"
			}
		case SlotIdle:
			if policy.IdleTTL > 0 && !slot.LastReleasedAt.IsZero() && now.Sub(slot.LastReleasedAt) >= policy.IdleTTL {
				proof = cleanupProof(slot, SlotIdle, "idle_ttl_expired", now, e.Usage(now), true)
				remove = true
			}
		case SlotFailed:
			proof = cleanupProof(slot, SlotFailed, "failed_slot_reclaimed", now, e.Usage(now), true)
			remove = true
		}
		if proof.Cleaned {
			slot.UpdatedAt = now
			slot.LastCleanedAt = now
			slot.CleanupCount++
			slot.LastCleanupReason = proof.Reason
			proof.CleanupCount = slot.CleanupCount
			report.Proofs = append(report.Proofs, proof)
			if proof.Removed {
				report.RemovedSlotIDs = append(report.RemovedSlotIDs, slot.ID)
			}
		}
		if !remove {
			kept = append(kept, slot)
		}
	}
	e.Slots = kept
	report.Usage = e.Usage(now)
	report.Budget = e.Budget(now)
	return report
}

func cleanupProof(slot Slot, to SlotState, reason string, now time.Time, usage ResourceUsage, removed bool) CleanupProof {
	return CleanupProof{
		SlotID:        slot.ID,
		FromState:     slot.State,
		ToState:       to,
		Reason:        reason,
		Cleaned:       true,
		Removed:       removed,
		CleanedAt:     now,
		LeaseCount:    slot.LeaseCount,
		CleanupCount:  slot.CleanupCount + 1,
		ResourceUsage: usage,
	}
}
