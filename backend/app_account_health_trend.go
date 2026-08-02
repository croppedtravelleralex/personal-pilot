package backend

import (
	"fmt"
	"strings"
	"time"
)

// AccountHealthDailyRow is one day bucket for longitudinal metrics (docs/55 AH1).
type AccountHealthDailyRow struct {
	Day              string  `json:"day"`
	Site             string  `json:"site,omitempty"`
	ChallengeRate    float64 `json:"challengeRate"`
	SuccessRate      float64 `json:"successRate"`
	DetectorPassRate float64 `json:"detectorPassRate"`
	SampleN          int     `json:"sampleN"`
	Status           string  `json:"status"` // ok | insufficient_data
}

// AccountHealthTrend returns per-day rates for a profile over windowDays.
func (a *App) AccountHealthTrend(profileID, metric string, windowDays int) ([]AccountHealthDailyRow, error) {
	profileID = strings.TrimSpace(profileID)
	if profileID == "" {
		return nil, fmt.Errorf("profileId is required")
	}
	if windowDays <= 0 {
		windowDays = 7
	}
	if windowDays > 90 {
		windowDays = 90
	}
	if a == nil || a.db == nil {
		return nil, fmt.Errorf("database not ready")
	}
	_ = metric // reserved for future metric filter; currently returns composite rows
	since := time.Now().UTC().AddDate(0, 0, -windowDays).Format("2006-01-02")
	rows, err := a.db.GetConn().Query(`
		SELECT day_key, site, challenges, successes, failures, detector_ok, detector_n
		FROM account_health_daily
		WHERE profile_id = ? AND day_key >= ?
		ORDER BY day_key ASC`, profileID, since)
	if err != nil {
		// Table may not exist yet on older DBs before migration 17.
		return a.accountHealthTrendFromRaw(profileID, windowDays)
	}
	defer rows.Close()
	out := make([]AccountHealthDailyRow, 0)
	for rows.Next() {
		var day, site string
		var challenges, successes, failures, detectorOK, detectorN int
		if err := rows.Scan(&day, &site, &challenges, &successes, &failures, &detectorOK, &detectorN); err != nil {
			continue
		}
		out = append(out, buildHealthDailyRow(day, site, challenges, successes, failures, detectorOK, detectorN))
	}
	if len(out) == 0 {
		return a.accountHealthTrendFromRaw(profileID, windowDays)
	}
	return out, nil
}

func buildHealthDailyRow(day, site string, challenges, successes, failures, detectorOK, detectorN int) AccountHealthDailyRow {
	row := AccountHealthDailyRow{Day: day, Site: site, Status: "ok"}
	totalOutcomes := successes + failures
	row.SampleN = challenges + totalOutcomes + detectorN
	if row.SampleN < 3 {
		row.Status = "insufficient_data"
	}
	if challenges+successes > 0 {
		row.ChallengeRate = float64(challenges) / float64(challenges+successes)
	}
	if totalOutcomes > 0 {
		row.SuccessRate = float64(successes) / float64(totalOutcomes)
	}
	if detectorN > 0 {
		row.DetectorPassRate = float64(detectorOK) / float64(detectorN)
	}
	return row
}

func (a *App) accountHealthTrendFromRaw(profileID string, windowDays int) ([]AccountHealthDailyRow, error) {
	since := time.Now().UTC().AddDate(0, 0, -windowDays)
	type agg struct {
		challenges, successes, failures, detectorOK, detectorN int
	}
	byDay := map[string]*agg{}

	if rows, err := a.db.GetConn().Query(`
		SELECT substr(created_at, 1, 10), COUNT(*)
		FROM asymmetric_challenges
		WHERE profile_id = ? AND created_at >= ?
		GROUP BY substr(created_at, 1, 10)`, profileID, since.Format(time.RFC3339)); err == nil {
		defer rows.Close()
		for rows.Next() {
			var day string
			var n int
			if rows.Scan(&day, &n) == nil {
				bucket := byDay[day]
				if bucket == nil {
					bucket = &agg{}
					byDay[day] = bucket
				}
				bucket.challenges += n
			}
		}
	}

	if rows, err := a.db.GetConn().Query(`
		SELECT substr(created_at, 1, 10),
		       SUM(CASE WHEN score >= 50 THEN 1 ELSE 0 END),
		       SUM(CASE WHEN score < 50 THEN 1 ELSE 0 END)
		FROM workbench_detection_results
		WHERE profile_id = ? AND kind = ? AND created_at >= ?
		GROUP BY substr(created_at, 1, 10)`, profileID, workbenchAccountOutcomeKind, since.Format(time.RFC3339)); err == nil {
		defer rows.Close()
		for rows.Next() {
			var day string
			var okN, failN int
			if rows.Scan(&day, &okN, &failN) == nil {
				bucket := byDay[day]
				if bucket == nil {
					bucket = &agg{}
					byDay[day] = bucket
				}
				bucket.successes += okN
				bucket.failures += failN
			}
		}
	}

	out := make([]AccountHealthDailyRow, 0, len(byDay))
	for day, a := range byDay {
		out = append(out, buildHealthDailyRow(day, "", a.challenges, a.successes, a.failures, a.detectorOK, a.detectorN))
	}
	if len(out) == 0 {
		return []AccountHealthDailyRow{{
			Day:    time.Now().UTC().Format("2006-01-02"),
			Status: "insufficient_data",
		}}, nil
	}
	return out, nil
}

// NoteAccountHealthObservation upserts a daily rollup cell (best-effort).
func (a *App) NoteAccountHealthObservation(profileID, site string, challengeDelta, successDelta, failureDelta, detectorOKDelta, detectorNDelta int) {
	if a == nil || a.db == nil || strings.TrimSpace(profileID) == "" {
		return
	}
	day := time.Now().UTC().Format("2006-01-02")
	site = strings.TrimSpace(site)
	now := time.Now().UTC().Format(time.RFC3339)
	_, _ = a.db.GetConn().Exec(`
		INSERT INTO account_health_daily
		  (profile_id, day_key, site, challenges, successes, failures, detector_ok, detector_n, updated_at)
		VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
		ON CONFLICT(profile_id, day_key, site) DO UPDATE SET
		  challenges = challenges + excluded.challenges,
		  successes = successes + excluded.successes,
		  failures = failures + excluded.failures,
		  detector_ok = detector_ok + excluded.detector_ok,
		  detector_n = detector_n + excluded.detector_n,
		  updated_at = excluded.updated_at`,
		profileID, day, site, challengeDelta, successDelta, failureDelta, detectorOKDelta, detectorNDelta, now,
	)
}
