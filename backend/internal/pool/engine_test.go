package pool

import (
	"testing"
	"time"
)

func TestPoolAcquireRelease(t *testing.T) {
	now := time.Date(2026, 6, 2, 8, 0, 0, 0, time.UTC)
	engine := Engine{Policy: Policy{MaxSlots: 1, DefaultLease: time.Minute}}
	slot, ok := engine.Acquire("p1", now)
	if !ok || slot.State != SlotLeased {
		t.Fatalf("slot = %+v ok=%v", slot, ok)
	}
	if _, ok := engine.Acquire("p2", now); ok {
		t.Fatal("pool should be full")
	}
	proof, ok := engine.ReleaseWithCleanup(slot.ID, now.Add(10*time.Second), "task_finished")
	if !ok || !proof.Cleaned || proof.Reason != "task_finished" {
		t.Fatalf("release proof = %+v ok=%v", proof, ok)
	}
	if proof.FromState != SlotLeased || proof.ToState != SlotIdle {
		t.Fatalf("unexpected proof transition: %+v", proof)
	}
	if engine.Slots[0].ProfileID != "" || engine.Slots[0].CleanupCount != 1 {
		t.Fatalf("slot was not cleaned: %+v", engine.Slots[0])
	}
	if !engine.Release(slot.ID) {
		t.Fatal("release failed")
	}
	if _, ok := engine.Acquire("p2", now.Add(20*time.Second)); !ok {
		t.Fatal("reacquire failed")
	}
}

func TestPoolPrewarmPlan(t *testing.T) {
	engine := Engine{Policy: Policy{MaxSlots: 1, DefaultLease: time.Minute}}
	plan, ok := engine.Prewarm(PrewarmRequest{ProfileID: "p1", RouteTag: "route-a", Inject: true, Now: time.Now()})
	if !ok || plan.Slot.State != SlotWarming || len(plan.Steps) != 5 {
		t.Fatalf("plan = %+v ok=%v", plan, ok)
	}
	if plan.Slot.RouteTag != "route-a" {
		t.Fatalf("route tag was not retained: %+v", plan.Slot)
	}
	if plan.Steps[3].Name != "budget" {
		t.Fatalf("expected budget step, got %+v", plan.Steps)
	}
}

func TestPoolBudgetAndCleanupExpiredLease(t *testing.T) {
	now := time.Date(2026, 6, 2, 9, 0, 0, 0, time.UTC)
	engine := Engine{Policy: Policy{MaxSlots: 2, DefaultLease: time.Minute}}
	if _, ok := engine.Acquire("p1", now); !ok {
		t.Fatal("acquire p1")
	}
	if _, ok := engine.Acquire("p2", now); !ok {
		t.Fatal("acquire p2")
	}
	if budget := engine.Budget(now); budget.Status != "at_capacity" {
		t.Fatalf("budget = %+v", budget)
	}

	cleanup := engine.Cleanup(now.Add(2 * time.Minute))
	if len(cleanup.Proofs) != 2 {
		t.Fatalf("cleanup proofs = %+v", cleanup)
	}
	for _, proof := range cleanup.Proofs {
		if proof.Reason != "lease_expired" || proof.FromState != SlotLeased || proof.ToState != SlotIdle {
			t.Fatalf("unexpected cleanup proof: %+v", proof)
		}
	}
	if usage := engine.Usage(now.Add(2 * time.Minute)); usage.Idle != 2 || usage.Leased != 0 {
		t.Fatalf("usage after cleanup = %+v", usage)
	}
	if budget := engine.Budget(now.Add(2 * time.Minute)); budget.Status != "within_budget" {
		t.Fatalf("budget after cleanup = %+v", budget)
	}
}

func TestPoolCleanupReclaimsStaleIdleAndFailedSlots(t *testing.T) {
	now := time.Date(2026, 6, 2, 10, 0, 0, 0, time.UTC)
	engine := Engine{Policy: Policy{MaxSlots: 3, DefaultLease: time.Minute, IdleTTL: time.Minute}}
	slot, ok := engine.Acquire("p1", now)
	if !ok {
		t.Fatal("acquire p1")
	}
	failed, ok := engine.Acquire("p2", now.Add(5*time.Second))
	if !ok {
		t.Fatal("acquire p2")
	}
	if _, ok := engine.ReleaseWithCleanup(slot.ID, now.Add(10*time.Second), "task_finished"); !ok {
		t.Fatal("release p1")
	}
	if !engine.MarkFailed(failed.ID, now.Add(30*time.Second), "launch_failed") {
		t.Fatal("mark failed")
	}

	report := engine.Cleanup(now.Add(2 * time.Minute))
	if len(report.RemovedSlotIDs) != 2 {
		t.Fatalf("expected stale idle and failed slot removal, got %+v", report)
	}
	if len(engine.Slots) != 0 {
		t.Fatalf("expected empty pool after cleanup, got %+v", engine.Slots)
	}
}
