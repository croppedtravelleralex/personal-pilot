package asymmetric

import "testing"

func TestStealthMatrix99PlusWithTrustAndBonuses(t *testing.T) {
	matrix := EvaluateStealthMatrix(StealthMatrixInput{
		WebRTCClean: true, DNSConsistent: true, ResidentialProxy: true,
		VerifyV2Passed: true, IPBudgetHeadroom: true,
		Runtime80of80: true, GeoLocaleMatch: true, WebdriverHidden: true,
		CreepJSTrust: 88, DetectionScore: 90,
		EntropyHumanLike: true, CadenceScore: 70, BioNoiseActive: true,
		TrustBundleValid: true, GraphTokenFresh: true, CookiesInjected: true, APIFirstReady: true,
		InHumanWindow: true, AccountSuccess: 90, ChallengeRatePct: 2,
	})
	if matrix.TotalScore < 99 {
		t.Fatalf("expected >=99 got %.1f gaps=%v", matrix.TotalScore, matrix.Gaps)
	}
	if matrix.DisplayGrade != "S+" {
		t.Fatalf("expected S+ got %s", matrix.DisplayGrade)
	}
	if matrix.BonusPoints < 4 {
		t.Fatalf("expected bonus stack got %.1f", matrix.BonusPoints)
	}
}

func TestStealthMatrixCapWithoutTrust(t *testing.T) {
	matrix := EvaluateStealthMatrix(StealthMatrixInput{
		WebRTCClean: true, DNSConsistent: true, ResidentialProxy: true,
		VerifyV2Passed: true, IPBudgetHeadroom: true,
		Runtime80of80: true, GeoLocaleMatch: true, WebdriverHidden: true,
		CreepJSTrust: 90, DetectionScore: 95,
		EntropyHumanLike: true, CadenceScore: 80, BioNoiseActive: true,
		InHumanWindow: true, AccountSuccess: 95, ChallengeRatePct: 0,
	})
	if matrix.TotalScore >= 99 {
		t.Fatalf("without trust bundle should stay below 99, got %.1f", matrix.TotalScore)
	}
	for _, g := range matrix.Gaps {
		if g == "" {
			t.Fatal("empty gap string")
		}
	}
}

func TestEvaluateCostAsymmetryLegacy(t *testing.T) {
	matrix := EvaluateStealthMatrix(StealthMatrixInput{
		WebRTCClean: true, DNSConsistent: true, ResidentialProxy: true,
		VerifyV2Passed: true, IPBudgetHeadroom: true,
		Runtime80of80: true, GeoLocaleMatch: true, WebdriverHidden: true,
		CreepJSTrust: 88, DetectionScore: 90,
		EntropyHumanLike: true, CadenceScore: 70, BioNoiseActive: true,
		TrustBundleValid: true, GraphTokenFresh: true, CookiesInjected: true, APIFirstReady: true,
		InHumanWindow: true, AccountSuccess: 90, ChallengeRatePct: 2,
	})
	legacy := EvaluateCostAsymmetryLegacy(matrix)
	if legacy.Score < 90 {
		t.Fatalf("legacy score=%d", legacy.Score)
	}
	if legacy.Level != "asymmetric_advantage" {
		t.Fatalf("level=%s score=%d grade=%s", legacy.Level, legacy.Score, matrix.DisplayGrade)
	}
}

func TestIPBudgetHeadroom(t *testing.T) {
	if !HasHeadroom(4, DefaultIPDailyVisitCap) {
		t.Fatal("4 visits should have headroom")
	}
	if HasHeadroom(5, DefaultIPDailyVisitCap) {
		t.Fatal("5 visits should exhaust cap")
	}
}

func TestStealthMatrix99PlusWithoutResidentialWhenTrustFull(t *testing.T) {
	// Local self-use can still reach S+ without residential if DNS policy is consistent
	// and trust/session inheritance is complete.
	matrix := EvaluateStealthMatrix(StealthMatrixInput{
		WebRTCClean: true, DNSConsistent: true, DNSObserved: true,
		ResidentialProxy: false, VerifyV2Passed: true, IPBudgetHeadroom: true,
		Runtime80of80: true, GeoLocaleMatch: true, WebdriverHidden: true,
		CreepJSTrust: 100, DetectionScore: 90,
		EntropyHumanLike: true, CadenceScore: 90, BioNoiseActive: true,
		TrustBundleValid: true, GraphTokenFresh: true, CookiesInjected: true, APIFirstReady: true,
		InHumanWindow: true, AccountSuccess: 90, ChallengeRatePct: 0,
	})
	if matrix.TotalScore < 99 {
		t.Fatalf("expected >=99 without residential when trust full, got %.1f dims=%+v gaps=%v", matrix.TotalScore, matrix.Dimensions, matrix.Gaps)
	}
	if matrix.DisplayGrade != "S+" {
		t.Fatalf("expected S+ got %s score=%.1f", matrix.DisplayGrade, matrix.TotalScore)
	}
}

func TestNetworkDNSInconclusiveIsNeutral(t *testing.T) {
	withClean := EvaluateStealthMatrix(StealthMatrixInput{
		WebRTCClean: true, DNSConsistent: false, DNSObserved: false, DNSLeakSuspect: false,
		ResidentialProxy: false, VerifyV2Passed: true, IPBudgetHeadroom: true,
	})
	withSuspect := EvaluateStealthMatrix(StealthMatrixInput{
		WebRTCClean: true, DNSConsistent: false, DNSObserved: true, DNSLeakSuspect: true,
		ResidentialProxy: false, VerifyV2Passed: true, IPBudgetHeadroom: true,
	})
	// Inconclusive DNS should not be worse than a clean-but-unknown baseline by DNS alone.
	// Suspect DNS should not score higher than inconclusive.
	if withSuspect.TotalScore > withClean.TotalScore {
		t.Fatalf("suspect DNS scored higher (%.1f) than inconclusive (%.1f)", withSuspect.TotalScore, withClean.TotalScore)
	}
	// Explicit consistent observation should raise score.
	withConsistent := EvaluateStealthMatrix(StealthMatrixInput{
		WebRTCClean: true, DNSConsistent: true, DNSObserved: true, DNSLeakSuspect: false,
		ResidentialProxy: false, VerifyV2Passed: true, IPBudgetHeadroom: true,
	})
	if withConsistent.TotalScore <= withClean.TotalScore {
		t.Fatalf("consistent DNS should raise score: clean=%.1f consistent=%.1f", withClean.TotalScore, withConsistent.TotalScore)
	}
}
