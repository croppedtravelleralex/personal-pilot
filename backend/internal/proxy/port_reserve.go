package proxy

import (
	"fmt"
	"net"
	"sync"
)

var (
	portReserveMu sync.Mutex
	reservedPorts = make(map[int]struct{})
)

// ReservePortNumber picks a localhost port and holds a logical reservation until
// release() or ReleaseReservedPort is called. Callers must release after the
// target process binds or when the allocation attempt fails.
func ReservePortNumber() (port int, release func(), err error) {
	return reservePortNumber()
}

// ReleaseReservedPort clears a logical port reservation by number.
func ReleaseReservedPort(port int) {
	if port <= 0 {
		return
	}
	portReserveMu.Lock()
	delete(reservedPorts, port)
	portReserveMu.Unlock()
}

// reservePortNumber picks a localhost port and holds a logical reservation until
// release() is called. Callers must release after the target process binds or
// when the allocation attempt fails.
func reservePortNumber() (port int, release func(), err error) {
	portReserveMu.Lock()
	defer portReserveMu.Unlock()

	for attempt := 0; attempt < 32; attempt++ {
		ln, listenErr := net.Listen("tcp", "127.0.0.1:0")
		if listenErr != nil {
			continue
		}
		candidate := ln.Addr().(*net.TCPAddr).Port
		_ = ln.Close()
		if _, held := reservedPorts[candidate]; held {
			continue
		}
		reservedPorts[candidate] = struct{}{}
		return candidate, func() {
			portReserveMu.Lock()
			delete(reservedPorts, candidate)
			portReserveMu.Unlock()
		}, nil
	}
	return 0, nil, fmt.Errorf("无法分配可用端口")
}
