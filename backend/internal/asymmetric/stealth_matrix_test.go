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
