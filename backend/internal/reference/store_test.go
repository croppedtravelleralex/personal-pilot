package reference

import "testing"

func TestReferenceStore(t *testing.T) {
	store := NewStore()
	store.Put(Sample{ID: "chrome-win", Signals: map[string]float64{"webgl": 1, "audio": 0.5}})
	_, score, ok := store.Recognize(Sample{Signals: map[string]float64{"webgl": 1, "audio": 0.5}})
	if !ok || score < 0.99 {
		t.Fatalf("score = %f ok=%v", score, ok)
	}
	if len(CollectorProbeManifest()) < 5 {
		t.Fatal("probe manifest too small")
	}
}
