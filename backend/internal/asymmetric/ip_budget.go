package asymmetric

import "time"

const DefaultIPDailyVisitCap = 5

// IPBudget tracks per-profile per-exit-IP daily visit counts.
type IPBudget struct {
	ProfileID string
	ExitIP    string
	DayKey    string
	Count     int
}

// DayKeyUTC returns YYYY-MM-DD for bucketing.
func DayKeyUTC(t time.Time) string {
	return t.UTC().Format("2006-01-02")
}

// HasHeadroom reports whether another visit is allowed under cap.
func HasHeadroom(count, cap int) bool {
	if cap <= 0 {
		cap = DefaultIPDailyVisitCap
	}
	return count < cap
}
