package pool

import "time"

type PrewarmRequest struct {
	ProfileID string
	RouteTag  string
	Inject    bool
	Now       time.Time
}

type PrewarmStep struct {
	Name   string
	Status string
	Detail string
}

type PrewarmPlan struct {
	Slot  Slot
	Steps []PrewarmStep
}

func (e *Engine) Prewarm(request PrewarmRequest) (PrewarmPlan, bool) {
	if request.Now.IsZero() {
		request.Now = time.Now()
	}
	slot, ok := e.Acquire(request.ProfileID, request.Now)
	if !ok {
		return PrewarmPlan{}, false
	}
	for i := range e.Slots {
		if e.Slots[i].ID == slot.ID {
			e.Slots[i].RouteTag = request.RouteTag
			e.Slots[i].State = SlotWarming
			e.Slots[i].UpdatedAt = request.Now
			e.Slots[i].LastResourceSummary = e.Budget(request.Now).Status
			slot = e.Slots[i]
		}
	}
	steps := []PrewarmStep{
		{Name: "profile", Status: "planned", Detail: request.ProfileID},
		{Name: "route", Status: "planned", Detail: request.RouteTag},
		{Name: "launch", Status: "planned", Detail: "browser_start"},
		{Name: "budget", Status: "planned", Detail: e.Budget(request.Now).Status},
	}
	if request.Inject {
		steps = append(steps, PrewarmStep{Name: "inject", Status: "planned", Detail: "environment"})
	}
	return PrewarmPlan{Slot: slot, Steps: steps}, true
}
