package pool

import (
	"testing"
	"time"
)

func TestPoolAcquireRelease(t *testing.T) {
	engine := Engine{Policy: Policy{MaxSlots: 1, DefaultLease: time.Minute}}
	slot, ok := engine.Acquire("p1", time.Now())
	if !ok || slot.State != SlotLeased {
		t.Fatalf("slot = %+v ok=%v", slot, ok)
	}
	if _, ok := engine.Acquire("p2", time.Now()); ok {
		t.Fatal("pool should be full")
	}
	if !engine.Release(slot.ID) {
		t.Fatal("release failed")
	}
	if _, ok := engine.Acquire("p2", time.Now()); !ok {
		t.Fatal("reacquire failed")
	}
}

func TestPoolPrewarmPlan(t *testing.T) {
	engine := Engine{Policy: Policy{MaxSlots: 1, DefaultLease: time.Minute}}
	plan, ok := engine.Prewarm(PrewarmRequest{ProfileID: "p1", RouteTag: "route-a", Inject: true, Now: time.Now()})
	if !ok || plan.Slot.State != SlotWarming || len(plan.Steps) != 4 {
		t.Fatalf("plan = %+v ok=%v", plan, ok)
	}
}
