package backend

import (
	"fmt"
	"sort"
	"strings"
	"time"
)

// ChallengeAttributionBucket is one dimension slice for AH3 correlation.
type ChallengeAttributionBucket struct {
	Dimension     string  `json:"dimension"` // site | hour | challengeType | proxyId
	Key           string  `json:"key"`
	Challenges    int     `json:"challenges"`
	Successes     int     `json:"successes"`
	Failures      int     `json:"failures"`
	ChallengeRate float64 `json:"challengeRate"`
	SampleN       int     `json:"sampleN"`
	Status        string  `json:"status"` // ok | insufficient_data
}

// ChallengeAttributionReport correlates challenges with outcomes (docs/55 AH3).
type ChallengeAttributionReport struct {
	ProfileID   string                       `json:"profileId"`
	WindowHours int                          `json:"windowHours"`
	BySite      []ChallengeAttributionBucket `json:"bySite"`
	ByHour      []ChallengeAttributionBucket `json:"byHour"`
	ByType      []ChallengeAttributionBucket `json:"byType"`
	ByProxy     []ChallengeAttributionBucket `json:"byProxy"`
	TopRisks    []ChallengeAttributionBucket `json:"topRisks"`
	Status      string                       `json:"status"`
}

type attributionAgg struct {
	challenges, successes, failures int
}

// ChallengeAttributionReport builds per-dimension challenge↔outcome attribution.
func (a *App) ChallengeAttributionReport(profileID string, windowHours int) (*ChallengeAttributionReport, error) {
	profileID = strings.TrimSpace(profileID)
	if profileID == "" {
		return nil, fmt.Errorf("profileId is required")
	}
	if windowHours <= 0 {
		windowHours = 48
	}
	if windowHours > 24*30 {
		windowHours = 24 * 30
	}
	if a == nil || a.db == nil {
		return nil, fmt.Errorf("database not ready")
	}
	since := time.Now().UTC().Add(-time.Duration(windowHours) * time.Hour)
	sinceStr := since.Format(time.RFC3339)

	bySite := map[string]*attributionAgg{}
	byHour := map[string]*attributionAgg{}
	byType := map[string]*attributionAgg{}
	byProxy := map[string]*attributionAgg{}

	bump := func(m map[string]*attributionAgg, key string, challenges, successes, failures int) {
		key = strings.TrimSpace(key)
		if key == "" {
			key = "unknown"
		}
		bucket := m[key]
		if bucket == nil {
			bucket = &attributionAgg{}
			m[key] = bucket
		}
		bucket.challenges += challenges
		bucket.successes += successes
		bucket.failures += failures
	}

	if rows, err := a.db.GetConn().Query(`
		SELECT site, challenge_type, payload, created_at
		FROM asymmetric_challenges
		WHERE profile_id = ? AND created_at >= ?`, profileID, sinceStr); err == nil {
		defer rows.Close()
		for rows.Next() {
			var site, ctype, payload, created string
			if rows.Scan(&site, &ctype, &payload, &created) != nil {
				continue
			}
			bump(bySite, site, 1, 0, 0)
			bump(byType, ctype, 1, 0, 0)
			hour := "??"
			if t, err := time.Parse(time.RFC3339, created); err == nil {
				hour = fmt.Sprintf("%02d", t.UTC().Hour())
			}
			bump(byHour, hour, 1, 0, 0)
			bump(byProxy, extractProxyIDFromChallengePayload(payload), 1, 0, 0)
		}
	}

	if rows, err := a.db.GetConn().Query(`
		SELECT payload, created_at, score
		FROM workbench_detection_results
		WHERE profile_id = ? AND kind = ? AND created_at >= ?`,
		profileID, workbenchAccountOutcomeKind, sinceStr); err == nil {
		defer rows.Close()
		for rows.Next() {
			var payload, created string
			var score int
			if rows.Scan(&payload, &created, &score) != nil {
				continue
			}
			site := extractJSONStringField(payload, "site")
			okN, failN := 0, 0
			if score >= 50 {
				okN = 1
			} else {
				failN = 1
			}
			bump(bySite, site, 0, okN, failN)
			hour := "??"
			if t, err := time.Parse(time.RFC3339, created); err == nil {
				hour = fmt.Sprintf("%02d", t.UTC().Hour())
			}
			bump(byHour, hour, 0, okN, failN)
			if proxy := extractJSONStringField(payload, "proxyId"); proxy != "" {
				bump(byProxy, proxy, 0, okN, failN)
			}
		}
	}

	report := &ChallengeAttributionReport{
		ProfileID:   profileID,
		WindowHours: windowHours,
		BySite:      attributionBuckets("site", bySite),
		ByHour:      attributionBuckets("hour", byHour),
		ByType:      attributionBuckets("challengeType", byType),
		ByProxy:     attributionBuckets("proxyId", byProxy),
		Status:      "ok",
	}
	combined := append([]ChallengeAttributionBucket{}, report.BySite...)
	combined = append(combined, report.ByHour...)
	combined = append(combined, report.ByProxy...)
	report.TopRisks = topRiskBuckets(combined, 5)
	if len(report.BySite)+len(report.ByHour)+len(report.ByType) == 0 {
		report.Status = "insufficient_data"
	}
	return report, nil
}

func extractProxyIDFromChallengePayload(payload string) string {
	id := extractJSONStringField(payload, "proxyId")
	if id == "" {
		return "unknown"
	}
	return id
}

func extractJSONStringField(payload, field string) string {
	marker := `"` + field + `"`
	i := strings.Index(payload, marker)
	if i < 0 {
		return ""
	}
	rest := payload[i+len(marker):]
	j := strings.Index(rest, `"`)
	if j < 0 {
		return ""
	}
	rest = rest[j+1:]
	k := strings.Index(rest, `"`)
	if k < 0 {
		return ""
	}
	return strings.TrimSpace(rest[:k])
}

func attributionBuckets(dimension string, m map[string]*attributionAgg) []ChallengeAttributionBucket {
	out := make([]ChallengeAttributionBucket, 0, len(m))
	for key, a := range m {
		out = append(out, buildAttributionBucket(dimension, key, a.challenges, a.successes, a.failures))
	}
	sort.Slice(out, func(i, j int) bool {
		if out[i].ChallengeRate == out[j].ChallengeRate {
			return out[i].Challenges > out[j].Challenges
		}
		return out[i].ChallengeRate > out[j].ChallengeRate
	})
	return out
}

func buildAttributionBucket(dimension, key string, challenges, successes, failures int) ChallengeAttributionBucket {
	b := ChallengeAttributionBucket{
		Dimension:  dimension,
		Key:        key,
		Challenges: challenges,
		Successes:  successes,
		Failures:   failures,
		Status:     "ok",
	}
	b.SampleN = challenges + successes + failures
	if b.SampleN < 3 {
		b.Status = "insufficient_data"
	}
	denom := challenges + successes
	if denom > 0 {
		b.ChallengeRate = float64(challenges) / float64(denom)
	}
	return b
}

func topRiskBuckets(in []ChallengeAttributionBucket, n int) []ChallengeAttributionBucket {
	filtered := make([]ChallengeAttributionBucket, 0, len(in))
	for _, b := range in {
		if b.Challenges == 0 {
			continue
		}
		filtered = append(filtered, b)
	}
	sort.Slice(filtered, func(i, j int) bool {
		if filtered[i].ChallengeRate == filtered[j].ChallengeRate {
			return filtered[i].Challenges > filtered[j].Challenges
		}
		return filtered[i].ChallengeRate > filtered[j].ChallengeRate
	})
	if n > 0 && len(filtered) > n {
		filtered = filtered[:n]
	}
	return filtered
}
