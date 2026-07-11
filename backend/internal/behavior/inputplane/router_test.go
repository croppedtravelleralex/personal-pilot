package inputplane

import "testing"

func TestResolvePlane(t *testing.T) {
	cases := []struct {
		mode     Mode
		action   string
		frame    bool
		osAvail  bool
		want     Plane
	}{
		{ModeAuto, "click", false, true, PlaneOS},
		{ModeAuto, "type", false, true, PlaneOS},
		{ModeAuto, "scroll", false, true, PlaneCDP},
		{ModeAuto, "click", true, true, PlaneCDP},
		{ModeCDP, "click", false, true, PlaneCDP},
		{ModeOS, "click", false, true, PlaneOS},
		{ModeOS, "click", false, false, PlaneCDP},
		{ModeAuto, "navigate", false, true, PlaneCDP},
	}
	for _, tc := range cases {
		got := ResolvePlane(tc.mode, tc.action, tc.frame, tc.osAvail)
		if got != tc.want {
			t.Fatalf("ResolvePlane(%q,%q,frame=%v,os=%v)=%q want %q",
				tc.mode, tc.action, tc.frame, tc.osAvail, got, tc.want)
		}
	}
}

func TestParseMode(t *testing.T) {
	if ParseMode("") != ModeAuto {
		t.Fatal("empty → auto")
	}
	if ParseMode("OS") != ModeOS {
		t.Fatal("OS → os")
	}
	if ParseMode("cdp") != ModeCDP {
		t.Fatal("cdp")
	}
}
