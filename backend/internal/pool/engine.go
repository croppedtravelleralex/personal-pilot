package pool

import "time"

type SlotState string

const (
	SlotIdle    SlotState = "idle"
	SlotWarming SlotState = "warming"
	SlotLeased  SlotState = "leased"
	SlotFailed  SlotState = "failed"
)

type Slot struct {
	ID         string
	ProfileID  string
	State      SlotState
	LeaseUntil time.Time
}

type Policy struct {
	MaxSlots       uint32
	DefaultLease   time.Duration
	PrewarmEnabled bool
}

type Engine struct {
	Policy Policy
	Slots  []Slot
}

func (e *Engine) Acquire(profileID string, now time.Time) (Slot, bool) {
	for i := range e.Slots {
		if e.Slots[i].State == SlotIdle || (e.Slots[i].State == SlotLeased && now.After(e.Slots[i].LeaseUntil)) {
			e.Slots[i].ProfileID = profileID
			e.Slots[i].State = SlotLeased
			e.Slots[i].LeaseUntil = now.Add(e.Policy.DefaultLease)
			return e.Slots[i], true
		}
	}
	if uint32(len(e.Slots)) < e.Policy.MaxSlots {
		slot := Slot{ID: profileID + "-slot", ProfileID: profileID, State: SlotLeased, LeaseUntil: now.Add(e.Policy.DefaultLease)}
		e.Slots = append(e.Slots, slot)
		return slot, true
	}
	return Slot{}, false
}

func (e *Engine) Release(slotID string) bool {
	for i := range e.Slots {
		if e.Slots[i].ID == slotID {
			e.Slots[i].State = SlotIdle
			e.Slots[i].ProfileID = ""
			return true
		}
	}
	return false
}
