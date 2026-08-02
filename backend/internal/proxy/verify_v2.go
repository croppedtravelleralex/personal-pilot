package proxy

import "time"

// VerifyV2Sample is one stability measurement.
type VerifyV2Sample struct {
	OK        bool
	LatencyMs int64
	ExitIP    string
	At        time.Time
}

// VerifyV2Streak evaluates repeated verify samples for stability streak.
type VerifyV2Streak struct {
	RequiredPasses int
	Samples        []VerifyV2Sample
}

// Evaluate returns pass when enough consecutive OK samples share the same exit IP.
func (s VerifyV2Streak) Evaluate() (passed bool, streak int, stableIP string) {
	if s.RequiredPasses <= 0 {
		s.RequiredPasses = 3
	}
	streak = 0
	for i := len(s.Samples) - 1; i >= 0; i-- {
		sample := s.Samples[i]
		if !sample.OK {
			break
		}
		if stableIP == "" {
			stableIP = sample.ExitIP
		}
		if sample.ExitIP != "" && stableIP != "" && sample.ExitIP != stableIP {
			break
		}
		streak++
		if streak >= s.RequiredPasses {
			return true, streak, stableIP
		}
	}
	return streak >= s.RequiredPasses, streak, stableIP
}

// Record appends a sample keeping last 10 entries.
func (s *VerifyV2Streak) Record(sample VerifyV2Sample) {
	if s == nil {
		return
	}
	if sample.At.IsZero() {
		sample.At = time.Now()
	}
	s.Samples = append(s.Samples, sample)
	if len(s.Samples) > 10 {
		s.Samples = s.Samples[len(s.Samples)-10:]
	}
}
