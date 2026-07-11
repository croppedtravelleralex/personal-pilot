package proxy

import "testing"

func TestAnalyzeICECandidatesPrivateLeak(t *testing.T) {
	probe := AnalyzeICECandidates("203.0.113.10", []string{
		"candidate:1 1 UDP 2130706431 192.168.1.5 54321 typ host",
	})
	if !probe.PrivateFound {
		t.Fatal("expected privateFound")
	}
	if !probe.LeakSuspect {
		t.Fatal("expected leakSuspect for private candidate")
	}
}

func TestAnalyzeICECandidatesClean(t *testing.T) {
	probe := AnalyzeICECandidates("203.0.113.10", []string{
		"candidate:1 1 UDP 2130706431 203.0.113.10 54321 typ srflx",
	})
	if probe.LeakSuspect {
		t.Fatalf("unexpected leakSuspect: %+v", probe)
	}
}

func TestClassifySystemDNSObservationDoesNotCompareCDNAddressToExitIP(t *testing.T) {
	result := classifySystemDNSObservation(LeakProbeResult{
		ExitIP:         "203.0.113.10",
		DNSResolvedIPs: []string{"198.51.100.20"},
	})
	if result.DNSStatus != DNSProbeStatusInconclusive {
		t.Fatalf("status=%q, want %q", result.DNSStatus, DNSProbeStatusInconclusive)
	}
	if result.DNSLeakSuspect {
		t.Fatalf("system resolver observation must not claim a proxy DNS leak: %+v", result)
	}
	if result.DNSConsistent {
		t.Fatalf("system resolver observation must not claim proxy DNS consistency: %+v", result)
	}
}
