package behavior

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"sort"
	"strings"
	"sync"
)

// RecordingStore persists and retrieves recordings.
type RecordingStore interface {
	Save(r *Recording) error
	Get(id string) (*Recording, error)
	List() ([]*Recording, error)
	Delete(id string) error
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

// Save persists a recording to disk.
func (s *FileRecordingStore) Save(r *Recording) error {
	s.mu.Lock()
	defer s.mu.Unlock()

	data, err := json.MarshalIndent(r, "", "  ")
	if err != nil {
		return fmt.Errorf("marshal recording: %w", err)
	}

	filePath := filepath.Join(s.dir, r.ID+".json")
	if err := os.WriteFile(filePath, data, 0644); err != nil {
		return fmt.Errorf("write recording: %w", err)
	}
	return nil
}

// Get loads a single recording by ID.
func (s *FileRecordingStore) Get(id string) (*Recording, error) {
	s.mu.RLock()
	defer s.mu.RUnlock()

	filePath := filepath.Join(s.dir, id+".json")
	data, err := os.ReadFile(filePath)
	if err != nil {
		if os.IsNotExist(err) {
			return nil, fmt.Errorf("recording not found: %s", id)
		}
		return nil, fmt.Errorf("read recording: %w", err)
	}

	var r Recording
	if err := json.Unmarshal(data, &r); err != nil {
		return nil, fmt.Errorf("unmarshal recording: %w", err)
	}
	return &r, nil
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

		filePath := filepath.Join(s.dir, entry.Name())
		data, err := os.ReadFile(filePath)
		if err != nil {
			continue
		}

		var r Recording
		if err := json.Unmarshal(data, &r); err != nil {
			continue
		}
		recordings = append(recordings, &r)
	}

	sort.Slice(recordings, func(i, j int) bool {
		return recordings[i].CreatedAt > recordings[j].CreatedAt
	})

	return recordings, nil
}

// Delete removes a recording by ID.
func (s *FileRecordingStore) Delete(id string) error {
	s.mu.Lock()
	defer s.mu.Unlock()

	filePath := filepath.Join(s.dir, id+".json")
	if err := os.Remove(filePath); err != nil {
		if os.IsNotExist(err) {
			return fmt.Errorf("recording not found: %s", id)
		}
		return fmt.Errorf("delete recording: %w", err)
	}
	return nil
}
