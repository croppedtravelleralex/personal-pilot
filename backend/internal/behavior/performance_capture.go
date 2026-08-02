package behavior

// PerformanceCaptureMetrics holds lightweight performance timeline samples.
type PerformanceCaptureMetrics struct {
	NavigationStartMs float64 `json:"navigationStartMs"`
	DOMContentLoadedMs float64 `json:"domContentLoadedMs"`
	LoadEventEndMs    float64 `json:"loadEventEndMs"`
	FirstPaintMs      float64 `json:"firstPaintMs"`
	ResourceCount     int     `json:"resourceCount"`
}

const performanceCaptureRetrieveJS = `(function(){
  const nav = performance.getEntriesByType('navigation')[0];
  const paints = performance.getEntriesByType('paint');
  const fp = paints.find((p) => p.name === 'first-paint');
  return JSON.stringify({
    navigationStartMs: nav ? nav.startTime : 0,
    domContentLoadedMs: nav ? nav.domContentLoadedEventEnd : 0,
    loadEventEndMs: nav ? nav.loadEventEnd : 0,
    firstPaintMs: fp ? fp.startTime : 0,
    resourceCount: performance.getEntriesByType('resource').length
  });
})();`
