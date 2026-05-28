package lifecycle

import (
	"database/sql"
	"testing"
	"time"

	_ "modernc.org/sqlite"
)

func TestLifecycleStorePersistsState(t *testing.T) {
	db, err := sql.Open("sqlite", ":memory:")
	if err != nil {
		t.Fatal(err)
	}
	defer db.Close()
	store, err := NewStore(db)
	if err != nil {
		t.Fatal(err)
	}
	now := time.Date(2026, 5, 28, 10, 0, 0, 0, time.UTC)
	state := State{ProfileID: "p1", CreatedAt: now, LastActiveAt: now, InterestVector: map[string]float64{"news": 0.8}, SocialStage: "like", ExitRegion: "US", PreferredLocale: "en-US"}
	if err := store.Save(state); err != nil {
		t.Fatal(err)
	}
	if err := store.IncrementSession("p1", now.Add(time.Hour)); err != nil {
		t.Fatal(err)
	}
	loaded, err := store.Load("p1")
	if err != nil {
		t.Fatal(err)
	}
	if loaded.SessionCount != 1 || loaded.InterestVector["news"] == 0 || loaded.SocialStage != "like" {
		t.Fatalf("loaded = %+v", loaded)
	}
	if risk := AssessRisk(loaded, 0.5); risk <= 0 {
		t.Fatalf("risk = %f", risk)
	}
}
