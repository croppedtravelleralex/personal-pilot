package behavior

import _ "embed"

// RecordingInjectJS is the JavaScript snippet injected into a browser page.
// It records down/up atomics instead of click to avoid duplicate playback.
// It also persists state before navigation so the same CDP target can be
// re-injected after refresh/navigation and continue accumulating events.
//
//go:embed inject_script.js
var RecordingInjectJS string
