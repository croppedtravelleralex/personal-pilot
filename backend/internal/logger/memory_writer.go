package logger

import (
	"fmt"
	"strings"
	"sync"
)

const defaultMemoryBufferSize = 500
const defaultMemoryPageLimit = 100
const maxMemoryPageLimit = 200

// MemoryLogEntry 内存日志条目（供前端消费）
type MemoryLogEntry struct {
	Time      string                 `json:"time"`
	Level     string                 `json:"level"`
	Component string                 `json:"component"`
	Message   string                 `json:"message"`
	Fields    map[string]interface{} `json:"fields,omitempty"`
}

// MemoryLogPage is a paged view of the in-memory log buffer.
type MemoryLogPage struct {
	Entries []MemoryLogEntry `json:"entries"`
	Total   int              `json:"total"`
	Offset  int              `json:"offset"`
	Limit   int              `json:"limit"`
	HasMore bool             `json:"hasMore"`
}

// MemoryWriter 内存环形缓冲写入器，线程安全
type MemoryWriter struct {
	mu      sync.RWMutex
	entries []MemoryLogEntry
	maxSize int
}

var globalMemoryWriter *MemoryWriter

func init() {
	globalMemoryWriter = &MemoryWriter{
		entries: make([]MemoryLogEntry, 0, defaultMemoryBufferSize),
		maxSize: defaultMemoryBufferSize,
	}
}

// GetMemoryWriter 获取全局内存写入器
func GetMemoryWriter() *MemoryWriter {
	return globalMemoryWriter
}

func (w *MemoryWriter) Write(entry *LogEntry) error {
	if entry == nil {
		return nil
	}
	w.mu.Lock()
	defer w.mu.Unlock()

	item := MemoryLogEntry{
		Time:      entry.Timestamp.Format("2006-01-02 15:04:05"),
		Level:     entry.Level.String(),
		Component: entry.Component,
		Message:   entry.Message,
		Fields:    entry.Fields,
	}
	if len(w.entries) >= w.maxSize {
		w.entries = w.entries[1:]
	}
	w.entries = append(w.entries, item)
	return nil
}

func (w *MemoryWriter) Close() error { return nil }

// GetEntries 返回所有缓冲日志（最新在后）
func (w *MemoryWriter) GetEntries() []MemoryLogEntry {
	w.mu.RLock()
	defer w.mu.RUnlock()
	result := make([]MemoryLogEntry, len(w.entries))
	copy(result, w.entries)
	return result
}

// GetPage returns a filtered page of buffered logs, oldest first.
// A negative offset means the latest page after filters are applied.
func (w *MemoryWriter) GetPage(offset int, limit int, level string, keyword string) MemoryLogPage {
	w.mu.RLock()
	defer w.mu.RUnlock()

	limit = normalizeMemoryPageLimit(limit)
	level = normalizeMemoryLogLevel(level)
	keyword = strings.ToLower(strings.TrimSpace(keyword))

	filtered := make([]MemoryLogEntry, 0, minInt(len(w.entries), limit))
	for _, entry := range w.entries {
		if !matchesMemoryLogFilters(entry, level, keyword) {
			continue
		}
		filtered = append(filtered, entry)
	}

	total := len(filtered)
	offset = normalizeMemoryPageOffset(offset, limit, total)
	end := minInt(offset+limit, total)
	if offset > end {
		offset = end
	}

	entries := make([]MemoryLogEntry, end-offset)
	copy(entries, filtered[offset:end])

	return MemoryLogPage{
		Entries: entries,
		Total:   total,
		Offset:  offset,
		Limit:   limit,
		HasMore: end < total,
	}
}

// Clear 清空缓冲
func (w *MemoryWriter) Clear() {
	w.mu.Lock()
	defer w.mu.Unlock()
	w.entries = w.entries[:0]
}

func normalizeMemoryPageLimit(limit int) int {
	if limit <= 0 {
		return defaultMemoryPageLimit
	}
	if limit > maxMemoryPageLimit {
		return maxMemoryPageLimit
	}
	return limit
}

func normalizeMemoryPageOffset(offset int, limit int, total int) int {
	if total <= 0 {
		return 0
	}
	if offset < 0 {
		return maxInt(0, total-limit)
	}
	if offset >= total {
		return ((total - 1) / limit) * limit
	}
	return offset
}

func normalizeMemoryLogLevel(level string) string {
	level = strings.ToUpper(strings.TrimSpace(level))
	if level == "ALL" {
		return ""
	}
	return level
}

func matchesMemoryLogFilters(entry MemoryLogEntry, level string, keyword string) bool {
	if level != "" && !strings.EqualFold(entry.Level, level) {
		return false
	}
	if keyword == "" {
		return true
	}
	if strings.Contains(strings.ToLower(entry.Message), keyword) ||
		strings.Contains(strings.ToLower(entry.Component), keyword) ||
		strings.Contains(strings.ToLower(entry.Level), keyword) {
		return true
	}
	for key, value := range entry.Fields {
		if strings.Contains(strings.ToLower(key), keyword) ||
			strings.Contains(strings.ToLower(fmt.Sprint(value)), keyword) {
			return true
		}
	}
	return false
}

func minInt(a int, b int) int {
	if a < b {
		return a
	}
	return b
}

func maxInt(a int, b int) int {
	if a > b {
		return a
	}
	return b
}
