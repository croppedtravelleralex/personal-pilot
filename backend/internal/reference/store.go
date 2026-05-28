package reference

import "math"

type Sample struct {
	ID      string
	Runtime string
	Signals map[string]float64
}

type Store struct {
	samples map[string]Sample
}

func NewStore() Store { return Store{samples: map[string]Sample{}} }

func (s *Store) Put(sample Sample) {
	if s.samples == nil {
		s.samples = map[string]Sample{}
	}
	s.samples[sample.ID] = sample
}

func (s Store) Recognize(query Sample) (Sample, float64, bool) {
	var best Sample
	bestScore := -1.0
	for _, sample := range s.samples {
		score := cosine(sample.Signals, query.Signals)
		if score > bestScore {
			bestScore = score
			best = sample
		}
	}
	return best, bestScore, bestScore >= 0
}

func cosine(a, b map[string]float64) float64 {
	var dot, ma, mb float64
	for key, av := range a {
		bv := b[key]
		dot += av * bv
		ma += av * av
	}
	for _, bv := range b {
		mb += bv * bv
	}
	if ma == 0 || mb == 0 {
		return 0
	}
	return dot / (math.Sqrt(ma) * math.Sqrt(mb))
}
