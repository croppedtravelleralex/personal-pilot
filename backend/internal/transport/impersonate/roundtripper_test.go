package impersonate

import (
	"testing"

	utls "github.com/metacubex/utls"
)

func TestResolveHelloID(t *testing.T) {
	if got := ResolveHelloID("Chrome_133"); got != utls.HelloChrome_133 {
		t.Fatalf("133: %+v", got)
	}
	if got := ResolveHelloID(""); got != utls.HelloChrome_Auto {
		t.Fatalf("auto: %+v", got)
	}
}
