package browser

import "testing"

func TestLookupGeoCoordinate(t *testing.T) {
	c, ok := LookupGeoCoordinate("CN", "Shanghai")
	if !ok || c.Lat < 30 || c.Lat > 32 {
		t.Fatalf("%v ok=%v", c, ok)
	}
	c2, ok := LookupGeoCoordinate("US", "")
	if !ok {
		t.Fatal("US default")
	}
	_ = c2
	if _, ok := LookupGeoCoordinate("", ""); ok {
		t.Fatal("empty should fail")
	}
}
