# Behavior Recording & Playback - Comprehensive Test Plan

## Overview

This document provides a detailed test plan for the behavior recording and playback feature in AntBrowser. The feature allows users to record browser interactions (mouse movements, clicks, keyboard input, scrolling) and replay them with configurable human-like variations.

## Architecture Summary

- **Backend**: Go-based recording engine using Chrome DevTools Protocol (CDP)
- **Frontend**: React-based UI with TypeScript
- **Storage**: JSON files on disk
- **Recording**: JavaScript injection via CDP to capture DOM events
- **Playback**: CDP Input domain commands with Gaussian distribution for variations

---

## 1. Manual Test Scenarios

### 1.1 Basic Recording Flow

#### Test Case 1.1.1: Start Recording
**Prerequisites**: Browser instance is running

**Steps**:
1. Navigate to Browser List page
2. Start a browser instance (verify `running: true`, `debugPort > 0`)
3. Open the Recording Panel
4. Click "录制" (Record) button

**Expected Results**:
- Button changes to "停止录制" (Stop Recording)
- Red pulsing indicator appears with text "录制中..." (Recording...)
- Toast notification: "录制已开始 - 请在浏览器中操作"
- Backend creates recorder instance for the profile
- JavaScript injection script is loaded into browser page
- `window.__antRecorder` flag is set to `true`
- `window.__antRecordedEvents` array is initialized

**Verification Points**:
- Check browser console for: `[AntRecorder] Recording started`
- Verify CDP WebSocket connection is established on `ws://127.0.0.1:{debugPort}/devtools/browser`
- Confirm no errors in backend logs

---

#### Test Case 1.1.2: Perform Actions During Recording
**Prerequisites**: Recording is active

**Actions to Perform**:
1. Move mouse cursor across the page (various speeds)
2. Click on different elements (buttons, links, text fields)
3. Type text into input fields (including special characters)
4. Scroll up and down the page
5. Right-click to open context menu
6. Use keyboard shortcuts (Ctrl+A, Ctrl+C, etc.)

**Expected Results**:
- All events are captured in `window.__antRecordedEvents` array
- Each event has:
  - `t`: timestamp in milliseconds from recording start
  - `type`: event type ("move", "down", "up", "click", "key", "scroll")
  - Position data (`x`, `y`) for mouse events
  - Button data (`btn`: 0=left, 1=middle, 2=right) for clicks
  - Key data (`key`, `text`) for keyboard events
  - Delta data (`dx`, `dy`) for scroll events
- Mouse move events are throttled to 50ms intervals
- Timestamps are relative to `performance.now()` at recording start

**Verification Points**:
- Open browser DevTools console
- Run: `console.log(window.__antRecordedEvents)`
- Verify event count matches performed actions
- Check timestamp progression is monotonic
- Verify coordinates are within viewport bounds

---

#### Test Case 1.1.3: Stop Recording and Save
**Prerequisites**: Recording is active with captured events

**Steps**:
1. Click "停止录制" (Stop Recording) button
2. Wait for save operation to complete

**Expected Results**:
- Recording stops immediately
- Backend retrieves events via `Runtime.evaluate` CDP command
- Viewport dimensions are captured (`window.innerWidth`, `window.innerHeight`)
- Recording object is created with:
  - Unique ID: `rec-{timestamp}`
  - Name: `录制 {current datetime in zh-CN format}`
  - Events array
  - Duration in milliseconds (last event timestamp)
  - Viewport width and height
  - Creation timestamp (RFC3339 format)
- JSON file is saved to disk: `{recordingDir}/{id}.json`
- Recording appears in the list
- Toast notification: "录制已保存"
- Recording state resets (button returns to "录制")
- Cleanup: `window.__antRecordedEvents` and `window.__antRecorder` are deleted

**Verification Points**:
- Check recording file exists in storage directory
- Verify JSON structure is valid
- Confirm file permissions are 0644
- Verify recording appears in UI list with correct metadata

---

### 1.2 Playback Flow

#### Test Case 1.2.1: Select Recording for Playback
**Prerequisites**: At least one recording exists

**Steps**:
1. Open Recording Panel
2. Click the dropdown "选择录制..." (Select Recording)
3. Select a recording from the list

**Expected Results**:
- Dropdown shows all recordings with format: `{name} ({duration}s, {eventCount} 事件)`
- Selected recording ID is stored in state
- Play button becomes enabled (if browser is running)

---

#### Test Case 1.2.2: Configure Variation Settings (50% Intensity)
**Prerequisites**: Recording is selected

**Steps**:
1. Click "偏移配置" (Variation Config) to expand settings
2. Set "强度" (Intensity) slider to 50%
3. Verify other settings:
   - 时序抖动 (Timing Jitter): 200ms
   - 位置抖动 (Position Jitter): 5px
   - 速度变化 (Speed Variation): 20%
   - 微修正 (Micro Corrections): checked
   - 额外停顿 (Extra Pauses): checked

**Expected Results**:
- All sliders update in real-time
- Values are displayed next to labels
- Checkboxes toggle correctly
- Configuration is stored in component state

---

#### Test Case 1.2.3: Start Playback
**Prerequisites**: Browser is running, recording is selected, variation is configured

**Steps**:
1. Click Play button (▶)
2. Observe browser window

**Expected Results**:
- Toast notification: "回放已开始"
- "回放中..." (Playing...) indicator appears with pulsing animation
- "停止回放" (Stop Playback) button appears
- Backend creates PlaybackEngine instance
- CDP WebSocket connects to `ws://127.0.0.1:{debugPort}/devtools/page`
- Playback runs in background goroutine
- Events are dispatched via CDP `Input.dispatchMouseEvent` and `Input.dispatchKeyEvent`

**Playback Behavior**:
- Mouse movements use linear interpolation with 5-10 steps
- Position jitter applied using Gaussian distribution (Box-Muller transform)
- Timing jitter added to delays between events
- Click events split into mousePressed + mouseReleased with 30-100ms delay
- Keyboard events dispatch as: rawKeyDown → char (if text) → keyUp
- Scroll events use `window.scrollBy()` + mouseWheel event

**Verification Points**:
- Mouse cursor moves in browser window
- Clicks trigger actual DOM interactions
- Text appears in input fields
- Page scrolls as recorded
- Actions appear human-like with natural variations

---

#### Test Case 1.2.4: Playback with Variations - Multiple Runs
**Prerequisites**: Same recording, 50% intensity

**Steps**:
1. Play recording (Run 1)
2. Wait for completion
3. Play same recording again (Run 2)
4. Wait for completion
5. Play same recording again (Run 3)

**Expected Results**:
- Each playback follows the same general path
- Variations are different each time due to random seed:
  - Mouse positions vary by ±5px (Gaussian distribution)
  - Timing varies by ±200ms (Gaussian distribution)
  - Micro-corrections appear randomly (30% probability after clicks)
  - Extra pauses inserted randomly (20% probability in gaps >1000ms)
- No two playbacks are identical
- All playbacks complete successfully

**Verification Points**:
- Record screen or use browser DevTools Performance tab
- Compare mouse trajectories across runs
- Verify timing differences using timestamps
- Confirm final state is consistent (same page, same inputs)

---

### 1.3 Edge Cases

#### Test Case 1.3.1: Recording with No Events
**Steps**:
1. Start recording
2. Do not perform any actions
3. Stop recording immediately

**Expected Results**:
- Recording is saved with:
  - Empty events array: `[]`
  - Duration: 0ms
  - Valid viewport dimensions
- Recording appears in list
- Playback of this recording completes instantly without errors

---

#### Test Case 1.3.2: Recording While Browser Not Running
**Steps**:
1. Ensure browser instance is stopped
2. Attempt to click "录制" button

**Expected Results**:
- Button is disabled
- Tooltip shows: "请先启动浏览器实例"
- No backend call is made

---

#### Test Case 1.3.3: Playback on Different Viewport Size
**Setup**:
1. Record actions on 1920x1080 viewport
2. Resize browser window to 1280x720
3. Play recording

**Expected Results**:
- Playback proceeds without errors
- Mouse coordinates may be outside visible area (no automatic scaling)
- Events that target off-screen elements may not trigger visible effects
- No crashes or exceptions

**Note**: Current implementation does not scale coordinates. This is expected behavior.

---

#### Test Case 1.3.4: Stop Playback Mid-Execution
**Steps**:
1. Start playback of a long recording (>10 seconds)
2. After 3 seconds, click "停止回放" button

**Expected Results**:
- Playback stops immediately
- Context is cancelled via `context.WithCancel`
- Goroutine exits cleanly
- WebSocket connection closes
- Toast notification: "回放已停止"
- UI returns to idle state
- No orphaned goroutines or connections

**Verification Points**:
- Check backend logs for clean shutdown
- Verify no goroutine leaks (use `runtime.NumGoroutine()` if instrumented)

---

#### Test Case 1.3.5: Delete Recording While Playing
**Steps**:
1. Start playback of a recording
2. While playback is running, click delete button for the same recording

**Expected Results**:
- Recording is deleted from storage
- Recording disappears from list
- Playback continues until completion (uses in-memory copy)
- After playback completes, selected recording ID is cleared
- No errors or crashes

**Alternative Scenario** (if deletion is blocked):
- Delete operation returns error: "recording is currently playing"
- Recording remains in list until playback completes

---

#### Test Case 1.3.6: Multiple Recordings Running Simultaneously
**Setup**: Two browser instances running

**Steps**:
1. Start playback on Instance A with Recording 1
2. Immediately start playback on Instance B with Recording 2

**Expected Results**:
- Both playbacks run concurrently
- Each playback uses its own CDP connection
- No interference between playbacks
- Both complete successfully
- Backend maintains separate `playbacks` map entries keyed by `profileId`

**Verification Points**:
- Check backend `playbacks` map has two entries
- Verify each WebSocket connection is independent
- Confirm no race conditions or deadlocks

---

## 2. Verification Points for Recording JSON Structure

### Sample Recording File Structure

```json
{
  "id": "rec-1714234567890123456",
  "name": "录制 2026-04-25 14:30:15",
  "description": "",
  "events": [
    {
      "t": 0,
      "type": "move",
      "x": 450.5,
      "y": 320.8
    },
    {
      "t": 523,
      "type": "click",
      "x": 450.5,
      "y": 320.8,
      "btn": 0
    },
    {
      "t": 1245,
      "type": "key",
      "key": "h",
      "text": "h"
    },
    {
      "t": 3890,
      "type": "scroll",
      "dx": 0,
      "dy": 120
    }
  ],
  "durationMs": 3890,
  "viewportW": 1920,
  "viewportH": 1080,
  "createdAt": "2026-04-25T14:30:18+08:00"
}
```

### Validation Checklist

- [ ] `id` matches pattern `rec-{nanosecond timestamp}`
- [ ] `name` is non-empty string
- [ ] `events` is array (may be empty)
- [ ] Each event has `t` (int64, milliseconds)
- [ ] Each event has `type` (string: move|down|up|click|key|scroll)
- [ ] Mouse events have `x`, `y` (float64)
- [ ] Click events have `btn` (int: 0|1|2)
- [ ] Key events have `key` (string), optionally `text`
- [ ] Scroll events have `dx`, `dy` (float64)
- [ ] `durationMs` equals last event's `t` value (or 0 if no events)
- [ ] `viewportW` and `viewportH` are positive integers
- [ ] `createdAt` is valid RFC3339 timestamp
- [ ] File is valid JSON (no syntax errors)
- [ ] File permissions are 0644

---

## 3. Variation Algorithm Verification

### 3.1 Position Jitter (Gaussian Distribution)

**Test Method**:
1. Record a single click at position (500, 300)
2. Play recording 100 times with 50% intensity, 10px jitter
3. Collect all actual click positions

**Expected Statistical Properties**:
- Mean X ≈ 500 (±2px tolerance)
- Mean Y ≈ 300 (±2px tolerance)
- Standard deviation ≈ 5px (50% of 10px jitter)
- Distribution follows Gaussian curve (use chi-square test)
- ~68% of points within ±5px
- ~95% of points within ±10px

**Implementation Note**: Uses Box-Muller transform:
```go
z := math.Sqrt(-2*math.Log(u1+1e-10)) * math.Cos(2*math.Pi*u2)
return mean + z*stddev
```

---

### 3.2 Timing Jitter

**Test Method**:
1. Record two clicks 1000ms apart
2. Play recording 100 times with 200ms timing jitter
3. Measure actual delays

**Expected Results**:
- Mean delay ≈ 1000ms
- Standard deviation ≈ 100ms (intensity * jitter)
- All delays are positive (absolute value applied)
- Distribution is Gaussian

---

### 3.3 Bezier Curves for Mouse Movement

**Current Implementation**: Linear interpolation with 5-10 steps

**Test Method**:
1. Record mouse move from (100, 100) to (500, 500)
2. Play recording and capture intermediate positions

**Expected Results**:
- 5-10 intermediate points generated
- Each point has small Gaussian noise (±1px if intensity > 0)
- Path is approximately straight line
- 8-16ms delay between steps

**Note**: Despite the test plan request, the current implementation uses linear interpolation, not Bezier curves. This is the actual behavior in `playback.go:185-207`.

---

### 3.4 Micro-Corrections

**Test Method**:
1. Record a single click
2. Play recording 100 times with micro-corrections enabled

**Expected Results**:
- ~30% of playbacks include a micro-correction
- Micro-correction occurs 50-200ms after click
- Overshoot is ±1-2 pixels from original position
- Implemented as additional `mouseMoved` event

**Code Reference**: `playback.go:151-163`

---

### 3.5 Extra Pauses

**Test Method**:
1. Record actions with a 2-second gap between events
2. Play recording 100 times with extra pauses enabled

**Expected Results**:
- ~20% of playbacks insert extra pause in the gap
- Pause duration: 200-800ms
- Only inserted in gaps >1000ms
- Does not affect final timing significantly

**Code Reference**: `playback.go:166-179`

---

## 4. Error Handling & Recovery

### 4.1 CDP Connection Failures

**Test Scenarios**:

#### 4.1.1 Browser Closes During Recording
**Steps**:
1. Start recording
2. Close browser window manually
3. Attempt to stop recording

**Expected Results**:
- Backend detects CDP connection closed
- Error returned: "read events response: EOF" or similar
- Recording is not saved
- Toast error displayed
- UI returns to idle state
- No memory leaks

---

#### 4.1.2 Browser Closes During Playback
**Steps**:
1. Start playback
2. Close browser window manually

**Expected Results**:
- Playback goroutine detects context cancellation or WebSocket error
- Goroutine exits cleanly
- Backend removes entry from `playbacks` map
- No crash or panic

---

### 4.2 Storage Failures

#### 4.2.1 Disk Full
**Setup**: Fill disk to capacity

**Expected Results**:
- Save operation fails with error
- Error message displayed to user
- No partial/corrupted files written
- Application remains stable

---

#### 4.2.2 Permission Denied
**Setup**: Remove write permissions from recording directory

**Expected Results**:
- Save operation fails with error: "write recording: permission denied"
- User-friendly error message displayed
- Application remains stable

---

### 4.3 Concurrent Access

#### 4.3.1 Start Recording Twice on Same Profile
**Steps**:
1. Start recording on profile A
2. Attempt to start recording again on profile A

**Expected Results**:
- Second call returns error: "already recording on profile {id}"
- First recording continues unaffected
- Error toast displayed

---

#### 4.3.2 Start Playback Twice on Same Profile
**Steps**:
1. Start playback on profile A
2. Attempt to start playback again on profile A

**Expected Results**:
- Second call returns error: "already playing on profile {id}"
- First playback continues unaffected
- Error toast displayed

---

## 5. Performance & Scalability

### 5.1 Large Recordings

**Test Case**: Record 10 minutes of continuous activity

**Expected Results**:
- Recording file size: ~500KB - 2MB (depending on activity)
- Save operation completes in <1 second
- Load operation completes in <500ms
- Playback starts without delay
- Memory usage remains stable during playback

---

### 5.2 Many Recordings

**Test Case**: Create 100 recordings

**Expected Results**:
- List operation completes in <1 second
- Recordings sorted by creation time (newest first)
- UI remains responsive
- No pagination needed for <100 recordings

---

### 5.3 High-Frequency Events

**Test Case**: Record rapid mouse movements (shake mouse vigorously)

**Expected Results**:
- Mouse move events throttled to 50ms intervals
- Maximum ~20 move events per second
- No event loss or buffer overflow
- Recording file size remains manageable

---

## 6. Cross-Browser Compatibility

### 6.1 Different Browser Cores

**Test Matrix**:
- Chrome 120+
- Chrome 115-119
- Chromium-based browsers (Edge, Brave, Opera)

**Expected Results**:
- Recording works on all Chromium-based browsers
- CDP protocol compatibility maintained
- Event capture is consistent across versions

---

## 7. Security Considerations

### 7.1 Sensitive Data in Recordings

**Test Case**: Record typing password into input field

**Expected Results**:
- Password characters are captured in `text` field
- Recording JSON file contains plaintext password
- **Security Warning**: Users should be warned not to share recordings containing sensitive data

**Recommendation**: Add UI warning when starting recording.

---

### 7.2 Recording File Access

**Verification**:
- Recording files stored in application data directory
- File permissions: 0644 (owner read/write, group/others read)
- No world-writable permissions
- Directory permissions: 0755

---

## 8. UI/UX Testing

### 8.1 Responsive Design

**Test Cases**:
- Resize window to minimum width
- Expand/collapse advanced settings
- Long recording names (>100 characters)

**Expected Results**:
- No layout breaks
- Text truncates with ellipsis
- Scrollbars appear when needed
- Buttons remain accessible

---

### 8.2 Loading States

**Verification Points**:
- Loading spinner appears during async operations
- Buttons disabled during loading
- No double-submission possible
- Loading state clears on error

---

### 8.3 Toast Notifications

**Verification**:
- Success messages are green
- Error messages are red
- Messages auto-dismiss after 3-5 seconds
- Multiple toasts stack vertically

---

## 9. Regression Testing Checklist

Before each release, verify:

- [ ] Recording starts and stops cleanly
- [ ] All event types are captured (move, click, key, scroll)
- [ ] Recordings are saved with correct JSON structure
- [ ] Recordings list loads and displays correctly
- [ ] Playback executes recorded actions
- [ ] Variations are applied (position, timing, micro-corrections, pauses)
- [ ] Multiple playbacks of same recording produce different variations
- [ ] Stop playback works mid-execution
- [ ] Delete recording removes file and updates UI
- [ ] Error handling works for all edge cases
- [ ] No memory leaks or goroutine leaks
- [ ] CDP connections close properly
- [ ] UI remains responsive during operations

---

## 10. Automated Testing Recommendations

### 10.1 Unit Tests (Go)

**Files to Test**:
- `backend/internal/behavior/recorder.go`
- `backend/internal/behavior/playback.go`
- `backend/internal/behavior/recording_store.go`

**Test Coverage Goals**: 80%+

**Key Test Cases**:
```go
func TestRecorder_StartRecording(t *testing.T)
func TestRecorder_StopRecording(t *testing.T)
func TestPlaybackEngine_GaussianDistribution(t *testing.T)
func TestPlaybackEngine_TimingJitter(t *testing.T)
func TestFileRecordingStore_SaveAndLoad(t *testing.T)
func TestFileRecordingStore_List(t *testing.T)
func TestFileRecordingStore_Delete(t *testing.T)
```

---

### 10.2 Integration Tests (Go)

**Test Scenarios**:
- Full recording → save → load → playback cycle
- Concurrent recordings on different profiles
- CDP connection failure handling
- Storage I/O error handling

---

### 10.3 E2E Tests (Playwright)

**Test Scenarios**:
```typescript
test('should record and playback user actions', async ({ page }) => {
  // 1. Start browser instance
  // 2. Start recording
  // 3. Perform actions
  // 4. Stop recording
  // 5. Verify recording appears in list
  // 6. Start playback
  // 7. Verify actions are replayed
})

test('should apply variations to playback', async ({ page }) => {
  // 1. Record single click
  // 2. Play 3 times
  // 3. Verify positions differ
})
```

---

## 11. Known Limitations

1. **No Coordinate Scaling**: Recordings played on different viewport sizes do not scale coordinates automatically.

2. **No Event Filtering**: All events are recorded, including potentially sensitive data (passwords, credit cards).

3. **Single Page Only**: Recording captures events on the active page only. Navigation to new pages may break playback.

4. **No iFrame Support**: Events inside iframes may not be captured correctly.

5. **Linear Interpolation**: Mouse movements use linear interpolation, not Bezier curves (despite test plan request).

6. **No Playback Speed Control**: Playback speed cannot be adjusted (1x only).

---

## 12. Future Enhancements

1. **Coordinate Scaling**: Automatically scale coordinates for different viewport sizes
2. **Event Filtering**: Option to exclude sensitive input fields from recording
3. **Multi-Page Support**: Handle navigation and page transitions
4. **Playback Speed Control**: 0.5x, 1x, 2x speed options
5. **Recording Editing**: Trim, split, merge recordings
6. **Export/Import**: Share recordings between users
7. **Scheduled Playback**: Run recordings on schedule
8. **Loop Playback**: Repeat recording N times

---

## Appendix A: Test Data

### Sample Recording Files

Create these test recordings for regression testing:

1. **simple-click.json**: Single click at (500, 300)
2. **typing-test.json**: Type "Hello World" into text field
3. **scroll-test.json**: Scroll down 1000px
4. **complex-flow.json**: Navigate, click, type, scroll (30 seconds)
5. **empty.json**: No events (0ms duration)

---

## Appendix B: Performance Benchmarks

Target performance metrics:

| Operation | Target | Acceptable |
|-----------|--------|------------|
| Start recording | <100ms | <500ms |
| Stop recording | <500ms | <2s |
| Save recording | <200ms | <1s |
| Load recording list | <300ms | <1s |
| Start playback | <100ms | <500ms |
| Playback 1000 events | <30s | <60s |

---

## Appendix C: Error Messages

Standard error messages for user display:

- "请先启动浏览器实例" - Browser not running
- "录制已开始 - 请在浏览器中操作" - Recording started
- "录制已保存" - Recording saved
- "开始录制失败: {error}" - Recording start failed
- "停止录制失败: {error}" - Recording stop failed
- "回放已开始" - Playback started
- "回放已停止" - Playback stopped
- "回放失败: {error}" - Playback failed
- "删除失败: {error}" - Delete failed
- "录制已删除" - Recording deleted

---

**Document Version**: 1.0  
**Last Updated**: 2026-04-25  
**Author**: Claude (Anthropic)  
**Status**: Ready for Review
