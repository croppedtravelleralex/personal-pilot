package proxy

import (
	"sync"
	"testing"
)

func TestReservePortNumberReleasesAfterCallback(t *testing.T) {
	t.Parallel()

	port, release, err := ReservePortNumber()
	if err != nil {
		t.Fatalf("reservePortNumber failed: %v", err)
	}
	if port <= 0 {
		t.Fatalf("expected positive port, got %d", port)
	}
	release()

	port2, release2, err := reservePortNumber()
	if err != nil {
		t.Fatalf("second reservePortNumber failed: %v", err)
	}
	release2()
	if port2 <= 0 {
		t.Fatalf("expected positive port on second allocation")
	}
}

func TestReservePortNumberParallelAllocations(t *testing.T) {
	t.Parallel()

	const workers = 16
	ports := make(chan int, workers)
	var wg sync.WaitGroup
	for i := 0; i < workers; i++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			port, release, err := ReservePortNumber()
			if err != nil {
				t.Errorf("reservePortNumber failed: %v", err)
				return
			}
			ports <- port
			release()
		}()
	}
	wg.Wait()
	close(ports)

	seen := make(map[int]struct{})
	for port := range ports {
		if _, dup := seen[port]; dup {
			t.Fatalf("duplicate port reserved concurrently: %d", port)
		}
		seen[port] = struct{}{}
	}
}
