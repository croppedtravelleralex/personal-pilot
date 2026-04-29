package behavior

import (
	"ant-chrome/backend/internal/logger"
	"context"
	"fmt"
	"math/rand"
	"strings"
	"sync"
	"time"
)

// Engine runs behavioral simulation for a single browser instance.
type Engine struct {
	mu        sync.Mutex
	ctx       context.Context
	cancel    context.CancelFunc
	cdp       *cdpConn
	profile   *Profile
	state     State
	debugPort int
}

// NewEngine creates a behavior engine for the given browser debug port.
func NewEngine(debugPort int, profile *Profile) *Engine {
	return &Engine{
		debugPort: debugPort,
		profile:   profile,
	}
}

// Start connects to the browser CDP and begins behavioral simulation.
func (e *Engine) Start(ctx context.Context) error {
	log := logger.New("Behavior")

	cdp, err := connectCDP(e.debugPort)
	if err != nil {
		return fmt.Errorf("behavior engine connect: %w", err)
	}
	e.cdp = cdp

	e.ctx, e.cancel = context.WithCancel(ctx)
	e.state.Running = true
	e.state.StartedAt = time.Now()

	go e.loop()

	log.Info("行为模拟引擎已启动",
		logger.F("profile_id", e.profile.ID),
		logger.F("debug_port", e.debugPort),
		logger.F("persona", e.profile.Name),
	)
	return nil
}

// Stop gracefully shuts down the behavior engine.
func (e *Engine) Stop() {
	e.mu.Lock()
	defer e.mu.Unlock()

	if e.cancel != nil {
		e.cancel()
	}
	if e.cdp != nil {
		e.cdp.Close()
	}
	e.state.Running = false
}

// State returns a copy of the current engine state.
func (e *Engine) State() State {
	e.mu.Lock()
	defer e.mu.Unlock()
	return e.state
}

// Profile returns the active behavioral profile.
func (e *Engine) Profile() *Profile {
	return e.profile
}

// loop is the main simulation loop, running in a background goroutine.
func (e *Engine) loop() {
	if e.profile == nil {
		return
	}

	mouseTicker := e.mouseTicker()
	scrollTicker := e.scrollTicker()
	defer mouseTicker.Stop()
	defer scrollTicker.Stop()

	// Track virtual mouse position (0,0 at top-left of viewport).
	curX := 300.0 + float64(rand.Intn(900))
	curY := 200.0 + float64(rand.Intn(500))

	for {
		select {
		case <-e.ctx.Done():
			return
		case <-mouseTicker.C:
			if !e.profile.Mouse.Enabled || !e.profile.Idle.MouseWander {
				e.resetMouseTicker(mouseTicker)
				continue
			}
			targetX := 100.0 + float64(rand.Intn(1000))
			targetY := 100.0 + float64(rand.Intn(600))
			if err := e.cdp.mouseMove(curX, curY, targetX, targetY, e.profile.Mouse); err != nil {
				if isCDPClosed(err) {
					return
				}
			}
			curX, curY = targetX, targetY
			e.mu.Lock()
			e.state.LastMoveAt = time.Now()
			e.mu.Unlock()
			e.resetMouseTicker(mouseTicker)

		case <-scrollTicker.C:
			if !e.profile.Scroll.Enabled {
				continue
			}
			// Only scroll probabilistically
			if rand.Float64() > e.profile.Scroll.IdleScrollProb {
				continue
			}
			if err := e.cdp.scrollHuman(e.profile.Scroll); err != nil {
				if isCDPClosed(err) {
					return
				}
			}
			e.mu.Lock()
			e.state.LastScrollAt = time.Now()
			e.mu.Unlock()
		}
	}
}

// mouseTicker creates a ticker with an interval from the profile's idle move range.
func (e *Engine) mouseTicker() *time.Ticker {
	d := parseRangeDuration(e.profile.Mouse.IdleMoveInterval, 10*time.Second)
	return time.NewTicker(d)
}

func (e *Engine) resetMouseTicker(t *time.Ticker) {
	d := parseRangeDuration(e.profile.Mouse.IdleMoveInterval, 10*time.Second)
	t.Reset(d)
}

// scrollTicker creates a ticker for scroll events.
func (e *Engine) scrollTicker() *time.Ticker {
	return time.NewTicker(2 * time.Second)
}

// parseRangeDuration parses a "min-max" duration string like "5s-20s".
// Returns the default if parsing fails.
func parseRangeDuration(s string, defaultDur time.Duration) time.Duration {
	if s == "" {
		return defaultDur
	}
	parts := strings.SplitN(s, "-", 2)
	if len(parts) != 2 {
		d, err := time.ParseDuration(strings.TrimSpace(s))
		if err != nil {
			return defaultDur
		}
		return d
	}
	minD, err1 := time.ParseDuration(strings.TrimSpace(parts[0]))
	maxD, err2 := time.ParseDuration(strings.TrimSpace(parts[1]))
	if err1 != nil || err2 != nil || minD >= maxD {
		return defaultDur
	}
	delta := maxD - minD
	return minD + time.Duration(rand.Int63n(int64(delta)))
}

// isCDPClosed returns true if the error indicates the CDP connection is gone.
func isCDPClosed(err error) bool {
	if err == nil {
		return false
	}
	s := err.Error()
	return strings.Contains(s, "closed") ||
		strings.Contains(s, "connection") ||
		strings.Contains(s, "EOF") ||
		strings.Contains(s, "broken pipe")
}

// InjectTyping simulates typing a string with human-like rhythm.
// This is called externally (not part of the idle loop) when automated input is needed.
func (e *Engine) InjectTyping(text string) error {
	if e.cdp == nil || !e.profile.Keyboard.Enabled {
		return nil
	}

	kbd := e.profile.Keyboard
	for i, ch := range text {
		// Insert occasional typo
		if rand.Float64() < kbd.TypoProb && i > 0 {
			wrongChar := rune('a' + rand.Intn(26))
			e.dispatchKeyEvent("char", string(wrongChar))
			time.Sleep(time.Duration(kbd.BaseDelayMs) * time.Millisecond)
			// Backspace to fix
			e.dispatchKeyEvent("keyDown", "Backspace")
			e.dispatchKeyEvent("keyUp", "Backspace")
			time.Sleep(time.Duration(kbd.BaseDelayMs) * time.Millisecond)
		}

		e.dispatchKeyEvent("char", string(ch))

		// Variable delay between keystrokes
		delay := kbd.BaseDelayMs + rand.Intn(kbd.DelayStdDevMs*2+1) - kbd.DelayStdDevMs
		if kbd.BigramDelays && i > 0 {
			// Common bigrams get extra delay
			prev := rune(text[i-1])
			if isCommonBigram(prev, ch) {
				delay += 30 + rand.Intn(40)
			}
		}
		// Burst: occasionally faster sequence
		if rand.Float64() < kbd.BurstProb && i+1 < len(text) {
			for b := 0; b < kbd.BurstKeys && i+1 < len(text); b++ {
				i++
				e.dispatchKeyEvent("char", string(rune(text[i])))
				time.Sleep(time.Duration(20+rand.Intn(30)) * time.Millisecond)
			}
		}
		if delay < 10 {
			delay = 10
		}
		time.Sleep(time.Duration(delay) * time.Millisecond)
	}

	e.mu.Lock()
	e.state.LastTypeAt = time.Now()
	e.mu.Unlock()
	return nil
}

func (e *Engine) dispatchKeyEvent(typ, text string) error {
	_, err := e.cdp.sendCommand("Input.dispatchKeyEvent", map[string]interface{}{
		"type": typ,
		"text": text,
	})
	return err
}

// common bigrams that slow down real typists
var commonBigrams = map[rune]map[rune]bool{
	't': {'h': true}, 'h': {'e': true, 'i': true},
	'e': {'r': true, 'n': true, 'd': true},
	'a': {'n': true, 'l': true, 's': true},
	'i': {'n': true, 'o': true, 's': true},
	'o': {'n': true, 'u': true, 'f': true},
	'r': {'e': true, 'i': true, 'o': true},
	'n': {'g': true, 'd': true, 't': true},
	's': {'t': true, 'e': true, 'i': true},
	'c': {'h': true, 'k': true, 'e': true},
}

func isCommonBigram(prev, curr rune) bool {
	m, ok := commonBigrams[prev]
	if !ok {
		return false
	}
	return m[curr]
}
