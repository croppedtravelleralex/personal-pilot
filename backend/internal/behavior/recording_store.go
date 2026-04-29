package behavior

import (
	"bytes"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"sort"
	"strings"
	"sync"
)

var (
	ErrInvalidRecordingID = errors.New("invalid recording ID")
	ErrRecordingNotFound  = errors.New("recording not found")
)

const (
	recordingManifestVersion  = 1
	recordingIndexDirName     = ".index"
	recordingManifestSuffix   = ".manifest.json"
	recordingEventIndexSuffix = ".events.json"
)

type recordingManifest struct {
	Version   int                 `json:"version"`
	Recording *RecordingSummary   `json:"recording"`
	Stats     RecordingEventStats `json:"stats"`
}

type recordingEventIndex struct {
	Version int                 `json:"version"`
	Events  []recordingEventRef `json:"events"`
}

type recordingEventRef struct {
	Offset int64 `json:"offset"`
	Length int   `json:"length"`
}

// RecordingStore persists and retrieves recordings.
type RecordingStore interface {
	Save(r *Recording) error
	Get(id string) (*Recording, error)
	List() ([]*Recording, error)
	ListSummaries() ([]*RecordingSummary, error)
	Delete(id string) error
	Rename(id string, name string) error
}

// FileRecordingStore stores recordings as JSON files on disk.
type FileRecordingStore struct {
	mu  sync.RWMutex
	dir string
}

// NewFileRecordingStore creates a new file-based recording store.
// The directory is created if it does not exist.
func NewFileRecordingStore(dir string) (*FileRecordingStore, error) {
	if err := os.MkdirAll(dir, 0755); err != nil {
		return nil, fmt.Errorf("create recording dir: %w", err)
	}
	return &FileRecordingStore{dir: dir}, nil
}

// validateID rejects recording IDs that contain path traversal sequences.
func validateID(id string) error {
	if strings.TrimSpace(id) == "" || strings.Contains(id, "..") || strings.Contains(id, "/") || strings.Contains(id, "\\") {
		return ErrInvalidRecordingID
	}
	return nil
}

func (s *FileRecordingStore) recordingPath(id string) string {
	return filepath.Join(s.dir, id+".json")
}

func (s *FileRecordingStore) indexDir() string {
	return filepath.Join(s.dir, recordingIndexDirName)
}

func (s *FileRecordingStore) manifestPath(id string) string {
	return filepath.Join(s.indexDir(), id+recordingManifestSuffix)
}

func (s *FileRecordingStore) eventIndexPath(id string) string {
	return filepath.Join(s.indexDir(), id+recordingEventIndexSuffix)
}

// Save persists a recording to disk.
func (s *FileRecordingStore) Save(r *Recording) error {
	s.mu.Lock()
	defer s.mu.Unlock()

	if r == nil {
		return fmt.Errorf("recording is nil")
	}
	if err := validateID(r.ID); err != nil {
		return err
	}
	return s.writeRecordingLocked(r)
}

func (s *FileRecordingStore) writeRecordingLocked(r *Recording) error {
	copy := *r
	if copy.Events == nil {
		copy.Events = []RecordedEvent{}
	}
	copy.EventCount = len(copy.Events)
	data, err := json.MarshalIndent(&copy, "", "  ")
	if err != nil {
		return fmt.Errorf("marshal recording: %w", err)
	}
	refs, err := buildRecordingEventRefs(data)
	if err != nil {
		return fmt.Errorf("index recording events: %w", err)
	}
	if len(refs) != len(copy.Events) {
		return fmt.Errorf("index recording events: indexed %d events, want %d", len(refs), len(copy.Events))
	}

	filePath := s.recordingPath(r.ID)
	if err := os.WriteFile(filePath, data, 0644); err != nil {
		return fmt.Errorf("write recording: %w", err)
	}
	if err := s.writeRecordingIndexLocked(&copy, refs); err != nil {
		return err
	}
	return nil
}

// Get loads a single recording by ID.
func (s *FileRecordingStore) Get(id string) (*Recording, error) {
	if err := validateID(id); err != nil {
		return nil, err
	}
	s.mu.RLock()
	defer s.mu.RUnlock()

	return s.readRecordingLocked(id)
}

func (s *FileRecordingStore) readRecordingLocked(id string) (*Recording, error) {
	filePath := s.recordingPath(id)
	data, err := os.ReadFile(filePath)
	if err != nil {
		if os.IsNotExist(err) {
			return nil, fmt.Errorf("%w: %s", ErrRecordingNotFound, id)
		}
		return nil, fmt.Errorf("read recording: %w", err)
	}

	var r Recording
	if err := json.Unmarshal(data, &r); err != nil {
		return nil, fmt.Errorf("unmarshal recording: %w", err)
	}
	r.EventCount = len(r.Events)
	return &r, nil
}

// GetDetailPage loads a bounded event page. New indexed recordings avoid full event reads;
// legacy JSON recordings fall back to the complete recording for compatibility.
func (s *FileRecordingStore) GetDetailPage(id string, eventOffset int, eventLimit int) (*RecordingDetailPage, error) {
	if err := validateID(id); err != nil {
		return nil, err
	}
	s.mu.RLock()
	defer s.mu.RUnlock()

	if page, err := s.getDetailPageFromIndexLocked(id, eventOffset, eventLimit); err == nil {
		return page, nil
	}

	r, err := s.readRecordingLocked(id)
	if err != nil {
		return nil, err
	}
	return r.DetailPage(eventOffset, eventLimit), nil
}

// List returns all recordings sorted by creation time (newest first).
func (s *FileRecordingStore) List() ([]*Recording, error) {
	s.mu.RLock()
	defer s.mu.RUnlock()

	entries, err := os.ReadDir(s.dir)
	if err != nil {
		if os.IsNotExist(err) {
			return []*Recording{}, nil
		}
		return nil, fmt.Errorf("read recording dir: %w", err)
	}

	var recordings []*Recording
	for _, entry := range entries {
		if entry.IsDir() || !strings.HasSuffix(entry.Name(), ".json") {
			continue
		}

		id := strings.TrimSuffix(entry.Name(), ".json")
		if err := validateID(id); err != nil {
			continue
		}
		r, err := s.readRecordingLocked(id)
		if err == nil {
			recordings = append(recordings, r)
		}
	}

	sort.Slice(recordings, func(i, j int) bool {
		return recordings[i].CreatedAt > recordings[j].CreatedAt
	})

	return recordings, nil
}

// ListSummaries returns recording metadata sorted by creation time (newest first).
func (s *FileRecordingStore) ListSummaries() ([]*RecordingSummary, error) {
	s.mu.RLock()
	defer s.mu.RUnlock()

	entries, err := os.ReadDir(s.dir)
	if err != nil {
		if os.IsNotExist(err) {
			return []*RecordingSummary{}, nil
		}
		return nil, fmt.Errorf("read recording dir: %w", err)
	}

	var summaries []*RecordingSummary
	for _, entry := range entries {
		if entry.IsDir() || !strings.HasSuffix(entry.Name(), ".json") {
			continue
		}

		id := strings.TrimSuffix(entry.Name(), ".json")
		if err := validateID(id); err != nil {
			continue
		}

		if summary, err := s.readRecordingSummaryLocked(id); err == nil {
			summaries = append(summaries, summary)
			continue
		}

		r, err := s.readRecordingLocked(id)
		if err != nil {
			continue
		}
		summary := r.Summary()
		if summary != nil {
			summaries = append(summaries, summary)
		}
	}

	sort.Slice(summaries, func(i, j int) bool {
		return summaries[i].CreatedAt > summaries[j].CreatedAt
	})

	return summaries, nil
}

// Delete removes a recording by ID.
func (s *FileRecordingStore) Delete(id string) error {
	if err := validateID(id); err != nil {
		return err
	}
	s.mu.Lock()
	defer s.mu.Unlock()

	filePath := filepath.Join(s.dir, id+".json")
	if err := os.Remove(filePath); err != nil {
		if os.IsNotExist(err) {
			return fmt.Errorf("%w: %s", ErrRecordingNotFound, id)
		}
		return fmt.Errorf("delete recording: %w", err)
	}
	if err := s.deleteRecordingIndexLocked(id); err != nil {
		return err
	}
	return nil
}

// Rename updates the name of a recording in-place.
func (s *FileRecordingStore) Rename(id string, name string) error {
	if err := validateID(id); err != nil {
		return err
	}
	s.mu.Lock()
	defer s.mu.Unlock()

	filePath := filepath.Join(s.dir, id+".json")
	data, err := os.ReadFile(filePath)
	if err != nil {
		if os.IsNotExist(err) {
			return fmt.Errorf("%w: %s", ErrRecordingNotFound, id)
		}
		return fmt.Errorf("read recording: %w", err)
	}

	var r Recording
	if err := json.Unmarshal(data, &r); err != nil {
		return fmt.Errorf("unmarshal recording: %w", err)
	}

	r.Name = name
	r.EventCount = len(r.Events)
	return s.writeRecordingLocked(&r)
}

func (s *FileRecordingStore) writeRecordingIndexLocked(r *Recording, refs []recordingEventRef) error {
	if err := os.MkdirAll(s.indexDir(), 0755); err != nil {
		return fmt.Errorf("create recording index dir: %w", err)
	}

	manifest := recordingManifest{
		Version:   recordingManifestVersion,
		Recording: r.Summary(),
		Stats:     BuildRecordingEventStats(r.Events),
	}
	manifestData, err := json.MarshalIndent(manifest, "", "  ")
	if err != nil {
		return fmt.Errorf("marshal recording manifest: %w", err)
	}
	if err := os.WriteFile(s.manifestPath(r.ID), manifestData, 0644); err != nil {
		return fmt.Errorf("write recording manifest: %w", err)
	}

	indexData, err := json.MarshalIndent(recordingEventIndex{
		Version: recordingManifestVersion,
		Events:  refs,
	}, "", "  ")
	if err != nil {
		return fmt.Errorf("marshal recording event index: %w", err)
	}
	if err := os.WriteFile(s.eventIndexPath(r.ID), indexData, 0644); err != nil {
		return fmt.Errorf("write recording event index: %w", err)
	}
	return nil
}

func (s *FileRecordingStore) deleteRecordingIndexLocked(id string) error {
	for _, path := range []string{s.manifestPath(id), s.eventIndexPath(id)} {
		if err := os.Remove(path); err != nil && !os.IsNotExist(err) {
			return fmt.Errorf("delete recording index: %w", err)
		}
	}
	return nil
}

func (s *FileRecordingStore) readRecordingSummaryLocked(id string) (*RecordingSummary, error) {
	manifest, err := s.readRecordingManifestLocked(id)
	if err != nil {
		return nil, err
	}
	summary := *manifest.Recording
	return &summary, nil
}

func (s *FileRecordingStore) readRecordingManifestLocked(id string) (*recordingManifest, error) {
	data, err := os.ReadFile(s.manifestPath(id))
	if err != nil {
		return nil, err
	}
	var manifest recordingManifest
	if err := json.Unmarshal(data, &manifest); err != nil {
		return nil, err
	}
	if manifest.Version != recordingManifestVersion || manifest.Recording == nil || manifest.Recording.ID != id {
		return nil, fmt.Errorf("invalid recording manifest")
	}
	return &manifest, nil
}

func (s *FileRecordingStore) readRecordingEventIndexLocked(id string) ([]recordingEventRef, error) {
	data, err := os.ReadFile(s.eventIndexPath(id))
	if err != nil {
		return nil, err
	}
	var index recordingEventIndex
	if err := json.Unmarshal(data, &index); err != nil {
		return nil, err
	}
	if index.Version != recordingManifestVersion {
		return nil, fmt.Errorf("invalid recording event index")
	}
	for _, ref := range index.Events {
		if ref.Offset < 0 || ref.Length <= 0 {
			return nil, fmt.Errorf("invalid recording event index entry")
		}
	}
	return index.Events, nil
}

func (s *FileRecordingStore) getDetailPageFromIndexLocked(id string, eventOffset int, eventLimit int) (*RecordingDetailPage, error) {
	manifest, err := s.readRecordingManifestLocked(id)
	if err != nil {
		return nil, err
	}
	refs, err := s.readRecordingEventIndexLocked(id)
	if err != nil {
		return nil, err
	}
	if manifest.Recording.EventCount != len(refs) || manifest.Stats.Total != len(refs) {
		return nil, fmt.Errorf("recording index count mismatch")
	}

	offset, limit, end := normalizeRecordingEventPage(eventOffset, eventLimit, len(refs))
	events, err := s.readIndexedEventsLocked(id, refs[offset:end])
	if err != nil {
		return nil, err
	}
	summary := *manifest.Recording
	return &RecordingDetailPage{
		Recording:   &summary,
		Events:      events,
		EventOffset: offset,
		EventLimit:  limit,
		EventTotal:  len(refs),
		Stats:       manifest.Stats,
	}, nil
}

func (s *FileRecordingStore) readIndexedEventsLocked(id string, refs []recordingEventRef) ([]RecordedEvent, error) {
	file, err := os.Open(s.recordingPath(id))
	if err != nil {
		if os.IsNotExist(err) {
			return nil, fmt.Errorf("%w: %s", ErrRecordingNotFound, id)
		}
		return nil, fmt.Errorf("open recording: %w", err)
	}
	defer file.Close()

	events := make([]RecordedEvent, 0, len(refs))
	for _, ref := range refs {
		raw := make([]byte, ref.Length)
		n, err := file.ReadAt(raw, ref.Offset)
		if err != nil && !errors.Is(err, io.EOF) {
			return nil, fmt.Errorf("read indexed event: %w", err)
		}
		if n != ref.Length {
			return nil, fmt.Errorf("read indexed event: %w", io.ErrUnexpectedEOF)
		}
		var event RecordedEvent
		if err := json.Unmarshal(raw, &event); err != nil {
			return nil, fmt.Errorf("unmarshal indexed event: %w", err)
		}
		events = append(events, event)
	}
	return events, nil
}

func buildRecordingEventRefs(data []byte) ([]recordingEventRef, error) {
	decoder := json.NewDecoder(bytes.NewReader(data))
	token, err := decoder.Token()
	if err != nil {
		return nil, err
	}
	if delim, ok := token.(json.Delim); !ok || delim != '{' {
		return nil, fmt.Errorf("recording root must be object")
	}

	for decoder.More() {
		token, err := decoder.Token()
		if err != nil {
			return nil, err
		}
		key, ok := token.(string)
		if !ok {
			return nil, fmt.Errorf("recording object key must be string")
		}
		if key != "events" {
			var skipped json.RawMessage
			if err := decoder.Decode(&skipped); err != nil {
				return nil, err
			}
			continue
		}

		token, err = decoder.Token()
		if err != nil {
			return nil, err
		}
		if delim, ok := token.(json.Delim); !ok || delim != '[' {
			return nil, fmt.Errorf("recording events must be array")
		}

		var refs []recordingEventRef
		for decoder.More() {
			start := int(decoder.InputOffset())
			var raw json.RawMessage
			if err := decoder.Decode(&raw); err != nil {
				return nil, err
			}
			end := int(decoder.InputOffset())
			relativeStart := bytes.Index(data[start:end], raw)
			if relativeStart < 0 {
				return nil, fmt.Errorf("recording event offset not found")
			}
			refs = append(refs, recordingEventRef{
				Offset: int64(start + relativeStart),
				Length: len(raw),
			})
		}
		token, err = decoder.Token()
		if err != nil {
			return nil, err
		}
		if delim, ok := token.(json.Delim); !ok || delim != ']' {
			return nil, fmt.Errorf("recording events array not closed")
		}
		return refs, nil
	}
	return nil, fmt.Errorf("recording events field not found")
}
