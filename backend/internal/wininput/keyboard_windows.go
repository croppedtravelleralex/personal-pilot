//go:build windows

package wininput

import (
	"fmt"
	"math/rand"
	"time"
	"unsafe"

	"golang.org/x/sys/windows"
)

const (
	inputKeyboard = 1
	keyeventfKeyup = 0x0002
	keyeventfUnicode = 0x0004
	keyeventfScancode = 0x0008
)

// KeyboardSender injects keyboard events via SendInput at the system level.
// SendInput generates isTrusted: true events because they arrive through
// the hardware input queue, indistinguishable from physical keystrokes.
type KeyboardSender struct {
	wpm int
	rng *rand.Rand
}

// NewKeyboardSender creates a KeyboardSender with the given typing speed.
// wpm defaults to 45 if zero.
func NewKeyboardSender(wpm int) *KeyboardSender {
	if wpm <= 0 {
		wpm = 45
	}
	return &KeyboardSender{
		wpm: wpm,
		rng: rand.New(rand.NewSource(time.Now().UnixNano())),
	}
}

// TypeString types a string character by character with human-like inter-key delays.
// Non-ASCII characters skip to maintaining realism boundaries.
func (k *KeyboardSender) TypeString(text string) error {
	baseDelay := float64(60000/k.wpm) * float64(time.Millisecond)
	if baseDelay < 30*float64(time.Millisecond) {
		baseDelay = 30 * float64(time.Millisecond)
	}
	if baseDelay > 250*float64(time.Millisecond) {
		baseDelay = 250 * float64(time.Millisecond)
	}

	for _, ch := range text {
		if ch > 127 {
			continue // skip non-ASCII for SendInput
		}

		if err := k.sendChar(ch); err != nil {
			return fmt.Errorf("type char %q: %w", ch, err)
		}

		// Human-like inter-key delay with burst patterns
		jitter := float64(k.rng.Intn(80)) - 40 // -40 to +40ms
		delay := baseDelay + jitter
		// Occasionally burst (fast consecutive keys)
		if k.rng.Float64() < 0.1 {
			delay = delay * 0.4
		}
		// Occasionally pause (thinking)
		if k.rng.Float64() < 0.03 {
			delay += float64(200+k.rng.Intn(500)) * float64(time.Millisecond)
		}
		if delay < 15*float64(time.Millisecond) {
			delay = 15 * float64(time.Millisecond)
		}

		time.Sleep(time.Duration(delay))
	}
	return nil
}

// KeyDown sends a key down event for the given virtual key code.
func (k *KeyboardSender) KeyDown(vk uint16) error {
	var inputs [1]keybdInput
	inputs[0] = keybdInput{
		typ:   inputKeyboard,
		wVk:   vk,
		dwFlags: 0,
	}
	return sendInputs(inputs[:])
}

// KeyUp sends a key up event for the given virtual key code.
func (k *KeyboardSender) KeyUp(vk uint16) error {
	var inputs [1]keybdInput
	inputs[0] = keybdInput{
		typ:     inputKeyboard,
		wVk:     vk,
		dwFlags: keyeventfKeyup,
	}
	return sendInputs(inputs[:])
}

// PressKey presses and releases a key.
func (k *KeyboardSender) PressKey(vk uint16) error {
	if err := k.KeyDown(vk); err != nil {
		return err
	}
	time.Sleep(30 * time.Millisecond)
	return k.KeyUp(vk)
}

// Combo sends a key combination (modifier + key).
func (k *KeyboardSender) Combo(modVk, keyVk uint16) error {
	if err := k.KeyDown(modVk); err != nil {
		return err
	}
	time.Sleep(20 * time.Millisecond)
	if err := k.KeyDown(keyVk); err != nil {
		k.KeyUp(modVk)
		return err
	}
	time.Sleep(30 * time.Millisecond)
	if err := k.KeyUp(keyVk); err != nil {
		return err
	}
	time.Sleep(20 * time.Millisecond)
	return k.KeyUp(modVk)
}

// ─── internal ──────────────────────────────────────────────────────────────────────

type keybdInput struct {
	typ      uint32
	wVk      uint16
	wScan    uint16
	dwFlags  uint32
	time     uint32
	dwExtraInfo uintptr
}

func sendInputs(inputs []keybdInput) error {
	size := unsafe.Sizeof(inputs[0])
	for i := range inputs {
		ret, _, err := windows.NewLazyDLL("user32.dll").NewProc("SendInput").Call(
			1,
			uintptr(unsafe.Pointer(&inputs[i])),
			size,
		)
		if ret == 0 {
			return fmt.Errorf("SendInput keybd_input[%d]: %w", i, err)
		}
	}
	return nil
}

func (k *KeyboardSender) sendChar(ch rune) error {
	if ch >= 'A' && ch <= 'Z' {
		return k.sendShiftedChar(byte(ch))
	}

	shifted := map[rune]byte{
		'!': '1', '@': '2', '#': '3', '$': '4', '%': '5',
		'^': '6', '&': '7', '*': '8', '(': '9', ')': '0',
		'_': '-', '+': '=', '{': '[', '}': ']', '|': '\\',
		':': ';', '"': '\'', '<': ',', '>': '.', '?': '/',
		'~': '`',
	}
	if base, ok := shifted[ch]; ok {
		return k.sendShiftedChar(base)
	}

	vk := charToVK(byte(ch))
	if vk == 0 {
		return fmt.Errorf("no VK mapping for char %q", ch)
	}

	var inputs [2]keybdInput
	inputs[0] = keybdInput{typ: inputKeyboard, wVk: vk, dwFlags: 0}
	inputs[1] = keybdInput{typ: inputKeyboard, wVk: vk, dwFlags: keyeventfKeyup}
	return sendInputs(inputs[:])
}

func (k *KeyboardSender) sendShiftedChar(base byte) error {
	vk := charToVK(base)
	if vk == 0 {
		return fmt.Errorf("no VK mapping for char %q", base)
	}

	var inputs [4]keybdInput
	inputs[0] = keybdInput{typ: inputKeyboard, wVk: windows.VK_SHIFT, dwFlags: 0}
	inputs[1] = keybdInput{typ: inputKeyboard, wVk: vk, dwFlags: 0}
	inputs[2] = keybdInput{typ: inputKeyboard, wVk: vk, dwFlags: keyeventfKeyup}
	inputs[3] = keybdInput{typ: inputKeyboard, wVk: windows.VK_SHIFT, dwFlags: keyeventfKeyup}
	return sendInputs(inputs[:])
}

func charToVK(c byte) uint16 {
	switch {
	case c >= 'a' && c <= 'z':
		return uint16(c - 'a' + 'A')
	case c >= 'A' && c <= 'Z':
		return uint16(c)
	case c >= '0' && c <= '9':
		return uint16(c)
	case c == ' ':
		return windows.VK_SPACE
	case c == '-':
		return windows.VK_OEM_MINUS
	case c == '=':
		return windows.VK_OEM_PLUS
	case c == '[':
		return windows.VK_OEM_4
	case c == ']':
		return windows.VK_OEM_6
	case c == '\\':
		return windows.VK_OEM_5
	case c == ';':
		return windows.VK_OEM_1
	case c == '\'':
		return windows.VK_OEM_7
	case c == ',':
		return windows.VK_OEM_COMMA
	case c == '.':
		return windows.VK_OEM_PERIOD
	case c == '/':
		return windows.VK_OEM_2
	case c == '`':
		return windows.VK_OEM_3
	case c == '\t':
		return windows.VK_TAB
	case c == '\r', c == '\n':
		return windows.VK_RETURN
	case c == '\b':
		return windows.VK_BACK
	default:
		return 0
	}
}

// Common virtual key codes for convenience.
const (
	VK_TAB   = windows.VK_TAB
	VK_ENTER = windows.VK_RETURN
	VK_ESC   = windows.VK_ESCAPE
	VK_CTRL  = windows.VK_CONTROL
	VK_SHIFT = windows.VK_SHIFT
	VK_ALT   = windows.VK_MENU
)
