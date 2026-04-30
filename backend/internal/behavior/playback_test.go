package behavior

import (
	"context"
	"encoding/json"
	"errors"
	"math"
	"math/rand"
	"strings"
	"testing"
	"time"
)

func TestGaussian_ZeroStddev(t *testing.T) {
	e := &PlaybackEngine{rng: rand.New(rand.NewSource(42))}
	for i := 0; i < 100; i++ {
		v := e.gaussian(5.0, 0.0)
		if v != 5.0 {
			t.Errorf("gaussian(5, 0): got %f, want 5.0", v)
		}
	}
}

func TestGaussian_Distribution(t *testing.T) {
	e := &PlaybackEngine{rng: rand.New(rand.NewSource(42))}
	const N = 10000
	mean := 100.0
	stddev := 20.0

	var sum, sumSq float64
	samples := make([]float64, N)
	for i := 0; i < N; i++ {
		samples[i] = e.gaussian(mean, stddev)
		sum += samples[i]
		sumSq += samples[i] * samples[i]
	}

	// Check empirical mean is within 3% of target
	empMean := sum / N
	if math.Abs(empMean-mean) > mean*0.03 {
		t.Errorf("empirical mean %f deviates too far from %f", empMean, mean)
	}

	// Check empirical stddev is within 10% of target
	empVar := sumSq/N - empMean*empMean
	empStd := math.Sqrt(empVar)
	if math.Abs(empStd-stddev) > stddev*0.10 {
		t.Errorf("empirical stddev %f deviates too far from %f", empStd, stddev)
	}
}

func TestGaussian_AllPositive(t *testing.T) {
	// Verify that Box-Muller with a positive seed produces varied outputs
	e := &PlaybackEngine{rng: rand.New(rand.NewSource(12345))}
	seen := make(map[float64]bool)
	for i := 0; i < 50; i++ {
		v := e.gaussian(0, 1)
		seen[v] = true
	}
	if len(seen) < 5 {
		t.Error("gaussian should produce diverse values")
	}
}

func TestVariationConfig_Defaults(t *testing.T) {
	vc := VariationConfig{
		Intensity:        0.3,
		TimingJitter:     200,
		PositionJitter:   5,
		SpeedVariation:   0.2,
		MicroCorrections: true,
		ExtraPauses:      true,
	}

	if vc.Intensity < 0 || vc.Intensity > 1 {
		t.Error("Intensity should be in [0, 1]")
	}
	if vc.TimingJitter < 0 {
		t.Error("TimingJitter should be non-negative")
	}
	if vc.PositionJitter < 0 {
		t.Error("PositionJitter should be non-negative")
	}
}

func TestMapRecordedPoint_ScalesByViewport(t *testing.T) {
	recorded := playbackViewport{Width: 1000, Height: 500, DPR: 1, Scale: 1}
	current := playbackViewport{Width: 500, Height: 1000, DPR: 2, Scale: 1}

	x, y := mapRecordedPoint(200, 100, recorded, current)
	if x != 100 || y != 200 {
		t.Fatalf("mapped point = (%v,%v), want (100,200)", x, y)
	}
}

func TestMapRecordedPoint_LegacyMissingViewportIsIdentity(t *testing.T) {
	x, y := mapRecordedPoint(200, 100, playbackViewport{}, playbackViewport{Width: 500, Height: 500})
	if x != 200 || y != 100 {
		t.Fatalf("legacy mapped point = (%v,%v), want identity (200,100)", x, y)
	}
}

func TestRecordingPlaybackViewport_ReadsOptionalDPRScale(t *testing.T) {
	type futureRecording struct {
		ViewportW           int
		ViewportH           int
		DevicePixelRatio    float64
		VisualViewportScale float64
	}

	viewport := recordingPlaybackViewport(futureRecording{
		ViewportW:           1280,
		ViewportH:           720,
		DevicePixelRatio:    2,
		VisualViewportScale: 1.25,
	})

	if viewport.Width != 1280 || viewport.Height != 720 || viewport.DPR != 2 || viewport.Scale != 1.25 {
		t.Fatalf("viewport metadata = %+v, want width/height/dpr/scale preserved", viewport)
	}
}

func TestResolveScrollPoint_UsesRecordedEventCoordinates(t *testing.T) {
	recorded := playbackViewport{Width: 1000, Height: 500}
	current := playbackViewport{Width: 500, Height: 1000}
	evt := RecordedEvent{Type: "scroll", X: 200, Y: 100}

	x, y := resolveScrollPoint(evt, 9, 9, recorded, current)
	if x != 100 || y != 200 {
		t.Fatalf("scroll point = (%v,%v), want mapped event coordinate (100,200)", x, y)
	}
}

func TestResolveScrollPoint_FallsBackToLastOrCenter(t *testing.T) {
	current := playbackViewport{Width: 500, Height: 1000}

	x, y := resolveScrollPoint(RecordedEvent{Type: "scroll"}, 44, 55, playbackViewport{}, current)
	if x != 44 || y != 55 {
		t.Fatalf("scroll fallback last = (%v,%v), want (44,55)", x, y)
	}

	x, y = resolveScrollPoint(RecordedEvent{Type: "scroll"}, 0, 0, playbackViewport{}, current)
	if x != 250 || y != 500 {
		t.Fatalf("scroll fallback center = (%v,%v), want (250,500)", x, y)
	}
}

func TestComputePlaybackDelay_AppliesSpeedVariation(t *testing.T) {
	base := computePlaybackDelayMs(1000, VariationConfig{Intensity: 1}, rand.New(rand.NewSource(1)))
	varied := computePlaybackDelayMs(1000, VariationConfig{
		Intensity:      1,
		SpeedVariation: 0.5,
	}, rand.New(rand.NewSource(1)))

	if base != 1000 {
		t.Fatalf("base delay = %d, want 1000", base)
	}
	if varied == base {
		t.Fatalf("speed variation did not affect delay: got %d", varied)
	}
	if varied < 500 || varied > 1500 {
		t.Fatalf("varied delay = %d, want within [500,1500]", varied)
	}
}

func TestComputePlaybackDelay_JitterIsBoundedPositiveAndNegative(t *testing.T) {
	rng := rand.New(rand.NewSource(42))
	variation := VariationConfig{Intensity: 1, TimingJitter: 100}
	seenFaster := false
	seenSlower := false

	for i := 0; i < 200; i++ {
		delay := computePlaybackDelayMs(1000, variation, rng)
		if delay < 900 || delay > 1100 {
			t.Fatalf("delay with jitter = %d, want within [900,1100]", delay)
		}
		if delay < 1000 {
			seenFaster = true
		}
		if delay > 1000 {
			seenSlower = true
		}
	}

	if !seenFaster || !seenSlower {
		t.Fatalf("jitter should produce both faster and slower delays, seenFaster=%v seenSlower=%v", seenFaster, seenSlower)
	}
}

func TestLinearInterpolation(t *testing.T) {
	// Verify linear interpolation across 5 steps
	fromX, fromY := 0.0, 0.0
	toX, toY := 100.0, 200.0
	steps := 5

	for i := 1; i <= steps; i++ {
		t_ratio := float64(i) / float64(steps)
		ix := fromX + (toX-fromX)*t_ratio
		iy := fromY + (toY-fromY)*t_ratio

		// At step 5 (t=1.0), should be at destination
		if i == steps {
			if math.Abs(ix-toX) > 0.001 || math.Abs(iy-toY) > 0.001 {
				t.Errorf("final step: got (%f, %f), want (%f, %f)", ix, iy, toX, toY)
			}
		}
		// At step 2.5 (t=0.5), should be halfway
		if i == 3 {
			if math.Abs(ix-60) > 1 || math.Abs(iy-120) > 1 {
				t.Errorf("midpoint step: got (%f, %f), want near (60, 120)", ix, iy)
			}
		}
	}
}

func TestJitterApplication(t *testing.T) {
	e := &PlaybackEngine{rng: rand.New(rand.NewSource(99))}
	jitter := e.gaussian(0, 5)
	if jitter == 0 {
		t.Error("position jitter should produce a non-zero Gaussian sample for this seed")
	}
}

func TestPlaybackProgressCallbackReportsRunningAndCompleted(t *testing.T) {
	engine := NewPlaybackEngine(&Recording{
		ID: "rec-progress",
		Events: []RecordedEvent{
			{T: 0, Type: "noop"},
			{T: 1, Type: "noop"},
		},
		ViewportW: 100,
		ViewportH: 100,
	}, VariationConfig{})
	engine.viewport = playbackViewport{Width: 100, Height: 100, DPR: 1, Scale: 1}

	var got []PlaybackProgress
	engine.SetProgressCallback(func(progress PlaybackProgress) {
		got = append(got, progress)
	})

	if err := engine.run(context.Background()); err != nil {
		t.Fatalf("run: %v", err)
	}
	if len(got) != 3 {
		t.Fatalf("progress callback count = %d, want 3", len(got))
	}
	if got[0].RecordingID != "rec-progress" || got[0].EventIndex != 1 || got[0].EventTotal != 2 || got[0].Status != "running" {
		t.Fatalf("first progress = %+v, want first running event", got[0])
	}
	if got[1].EventIndex != 2 || got[1].Percent != 100 || got[1].Status != "running" {
		t.Fatalf("second progress = %+v, want 100 percent running event", got[1])
	}
	if got[2].EventIndex != 2 || got[2].EventTotal != 2 || got[2].Percent != 100 || got[2].Status != "completed" {
		t.Fatalf("completed progress = %+v, want completed at 100 percent", got[2])
	}
}

func TestPlaybackRun_MoveFailuresDoNotAbortStrictDownUp(t *testing.T) {
	engine := newPlaybackTestEngine([]RecordedEvent{
		{T: 0, Type: "move", X: 20, Y: 20},
		{T: 0, Type: "down", X: 20, Y: 20},
		{T: 0, Type: "up", X: 20, Y: 20},
	}, VariationConfig{})

	var strictEvents []string
	engine.sendCommandFn = func(id int, method string, params interface{}) (json.RawMessage, error) {
		eventType := mouseEventType(method, params)
		if eventType == "mouseMoved" {
			return nil, errors.New("cdp move rejected")
		}
		if eventType == "mousePressed" || eventType == "mouseReleased" {
			strictEvents = append(strictEvents, eventType)
		}
		return json.RawMessage(`{"result":{}}`), nil
	}

	if err := engine.run(context.Background()); err != nil {
		t.Fatalf("run should ignore best-effort move failures: %v", err)
	}
	if got, want := strings.Join(strictEvents, ","), "mousePressed,mouseReleased"; got != want {
		t.Fatalf("strict events = %q, want %q", got, want)
	}
}

func TestPlaybackRun_StrictPressReleaseFailuresReturnError(t *testing.T) {
	tests := []struct {
		name       string
		failType   string
		wantErrSub string
	}{
		{name: "press", failType: "mousePressed", wantErrSub: "mouseDown event failed"},
		{name: "release", failType: "mouseReleased", wantErrSub: "mouseUp event failed"},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			engine := newPlaybackTestEngine([]RecordedEvent{
				{T: 0, Type: "down", X: 20, Y: 20},
				{T: 0, Type: "up", X: 20, Y: 20},
			}, VariationConfig{})
			engine.sendCommandFn = func(id int, method string, params interface{}) (json.RawMessage, error) {
				if mouseEventType(method, params) == tt.failType {
					return nil, errors.New("strict cdp failure")
				}
				return json.RawMessage(`{"result":{}}`), nil
			}

			err := engine.run(context.Background())
			if err == nil {
				t.Fatalf("run error = nil, want strict failure")
			}
			if !strings.Contains(err.Error(), tt.wantErrSub) || !strings.Contains(err.Error(), "strict cdp failure") {
				t.Fatalf("run error = %q, want %q with cause", err.Error(), tt.wantErrSub)
			}
		})
	}
}

func TestPlaybackRun_ClickMicroCorrectionMoveFailureDoesNotAbort(t *testing.T) {
	engine := newPlaybackTestEngine([]RecordedEvent{
		{T: 0, Type: "click", X: 20, Y: 20},
	}, VariationConfig{MicroCorrections: true})
	engine.rng = rand.New(rand.NewSource(seedForClickMicroCorrection(t)))

	var strictEvents []string
	moveAttempts := 0
	engine.sendCommandFn = func(id int, method string, params interface{}) (json.RawMessage, error) {
		eventType := mouseEventType(method, params)
		if eventType == "mouseMoved" {
			moveAttempts++
			return nil, errors.New("micro-correction rejected")
		}
		if eventType == "mousePressed" || eventType == "mouseReleased" {
			strictEvents = append(strictEvents, eventType)
		}
		return json.RawMessage(`{"result":{}}`), nil
	}

	if err := engine.run(context.Background()); err != nil {
		t.Fatalf("run should ignore micro-correction move failures: %v", err)
	}
	if got, want := strings.Join(strictEvents, ","), "mousePressed,mouseReleased"; got != want {
		t.Fatalf("strict events = %q, want %q", got, want)
	}
	if moveAttempts == 0 {
		t.Fatalf("micro-correction move was not attempted")
	}
}

func TestPlaybackRun_AskEachTimePausesUntilReviewContinue(t *testing.T) {
	engine := newPlaybackTestEngine([]RecordedEvent{
		{T: 0, Type: "down", X: 20, Y: 20},
		{T: 0, Type: "up", X: 20, Y: 20},
	}, VariationConfig{
		ExecutionPolicy: ptrExecutionPolicy(DefaultExecutionPolicy(PermissionAskEachTime)),
	})

	progressCh := make(chan PlaybackProgress, 4)
	engine.SetProgressCallback(func(progress PlaybackProgress) {
		progressCh <- progress
	})
	var strictEvents []string
	engine.sendCommandFn = func(id int, method string, params interface{}) (json.RawMessage, error) {
		if eventType := mouseEventType(method, params); eventType == "mousePressed" || eventType == "mouseReleased" {
			strictEvents = append(strictEvents, eventType)
		}
		return json.RawMessage(`{"result":{}}`), nil
	}

	done := make(chan error, 1)
	go func() { done <- engine.run(context.Background()) }()

	progress := waitPlaybackProgress(t, progressCh)
	if progress.Status != "needs_review" || !strings.Contains(progress.Reason, "每次询问") {
		t.Fatalf("review progress = %+v, want needs_review with ask reason", progress)
	}
	if len(strictEvents) != 0 {
		t.Fatalf("strict events before continue = %#v, want none", strictEvents)
	}
	if err := engine.SubmitReviewDecision("continue"); err != nil {
		t.Fatalf("continue decision: %v", err)
	}
	if err := <-done; err != nil {
		t.Fatalf("run: %v", err)
	}
	if got, want := strings.Join(strictEvents, ","), "mousePressed,mouseReleased"; got != want {
		t.Fatalf("strict events = %q, want %q", got, want)
	}
}

func TestPlaybackRun_ReviewSkipDownAlsoSkipsPairedRelease(t *testing.T) {
	engine := newPlaybackTestEngine([]RecordedEvent{
		{T: 0, Type: "down", X: 20, Y: 20},
		{T: 0, Type: "up", X: 20, Y: 20},
	}, VariationConfig{
		ExecutionPolicy: ptrExecutionPolicy(DefaultExecutionPolicy(PermissionAskEachTime)),
	})

	progressCh := make(chan PlaybackProgress, 4)
	engine.SetProgressCallback(func(progress PlaybackProgress) {
		progressCh <- progress
	})
	var strictEvents []string
	engine.sendCommandFn = func(id int, method string, params interface{}) (json.RawMessage, error) {
		if eventType := mouseEventType(method, params); eventType == "mousePressed" || eventType == "mouseReleased" {
			strictEvents = append(strictEvents, eventType)
		}
		return json.RawMessage(`{"result":{}}`), nil
	}

	done := make(chan error, 1)
	go func() { done <- engine.run(context.Background()) }()
	progress := waitPlaybackProgress(t, progressCh)
	if progress.Status != "needs_review" {
		t.Fatalf("review progress = %+v, want needs_review", progress)
	}
	if err := engine.SubmitReviewDecision("skip"); err != nil {
		t.Fatalf("skip decision: %v", err)
	}
	if err := <-done; err != nil {
		t.Fatalf("run: %v", err)
	}
	if len(strictEvents) != 0 {
		t.Fatalf("strict events after skip = %#v, want none", strictEvents)
	}
}

func TestPlaybackRun_AutoReviewPausesLowConfidenceClick(t *testing.T) {
	engine := newPlaybackTestEngine([]RecordedEvent{
		{T: 0, Type: "click", X: 20, Y: 20},
	}, VariationConfig{
		ExecutionPolicy: ptrExecutionPolicy(DefaultExecutionPolicy(PermissionAutoReview)),
	})
	progressCh := make(chan PlaybackProgress, 4)
	engine.SetProgressCallback(func(progress PlaybackProgress) {
		progressCh <- progress
	})
	engine.sendCommandFn = func(id int, method string, params interface{}) (json.RawMessage, error) {
		return json.RawMessage(`{"result":{}}`), nil
	}

	done := make(chan error, 1)
	go func() { done <- engine.run(context.Background()) }()
	progress := waitPlaybackProgress(t, progressCh)
	if progress.Status != "needs_review" || !strings.Contains(progress.Reason, "低置信") {
		t.Fatalf("review progress = %+v, want low confidence pause", progress)
	}
	if err := engine.SubmitReviewDecision("stop"); err != nil {
		t.Fatalf("stop decision: %v", err)
	}
	err := <-done
	if err == nil || !strings.Contains(err.Error(), "review decision") {
		t.Fatalf("run error = %v, want review stop", err)
	}
}

func newPlaybackTestEngine(events []RecordedEvent, variation VariationConfig) *PlaybackEngine {
	engine := NewPlaybackEngine(&Recording{
		ID:        "rec-playback-test",
		Events:    events,
		ViewportW: 100,
		ViewportH: 100,
	}, variation)
	engine.viewport = playbackViewport{Width: 100, Height: 100, DPR: 1, Scale: 1}
	engine.rng = rand.New(rand.NewSource(1))
	return engine
}

func ptrExecutionPolicy(policy ExecutionPolicy) *ExecutionPolicy {
	return &policy
}

func waitPlaybackProgress(t *testing.T, ch <-chan PlaybackProgress) PlaybackProgress {
	t.Helper()
	select {
	case progress := <-ch:
		return progress
	case <-time.After(2 * time.Second):
		t.Fatal("timed out waiting for playback progress")
		return PlaybackProgress{}
	}
}

func mouseEventType(method string, params interface{}) string {
	if method != "Input.dispatchMouseEvent" {
		return ""
	}
	values, ok := params.(map[string]interface{})
	if !ok {
		return ""
	}
	eventType, _ := values["type"].(string)
	return eventType
}

func seedForClickMicroCorrection(t *testing.T) int64 {
	t.Helper()
	for seed := int64(1); seed < 1000; seed++ {
		rng := rand.New(rand.NewSource(seed))
		rng.Intn(70)
		if rng.Float64() < 0.3 {
			return seed
		}
	}
	t.Fatal("could not find deterministic seed for click micro-correction")
	return 0
}
