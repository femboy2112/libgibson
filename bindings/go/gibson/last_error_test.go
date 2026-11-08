package gibson_test

import (
	"runtime"
	"sync"
	"sync/atomic"
	"testing"

	"github.com/femboy2112/libgibson/bindings/go/gibson"
	"github.com/femboy2112/libgibson/bindings/go/internal/rawcgo"
)

// wantMsg is what gibson_create_context(99, &out) records. Mode 99 is rejected before any terminal
// is touched, so this runs the real NewContext failure path with or without a TTY.
const wantMsg = "Invalid render mode: 99"

// TestNewContextErrorIsFromItsOwnCall hammers NewContext from many goroutines while polluter
// goroutines, each locked to its own OS thread, keep overwriting their thread's last-error with a
// different text. The last error is thread-local in Rust, so if NewContext read it with a second
// cgo call that landed on another thread, it would return an empty or foreign message.
func TestNewContextErrorIsFromItsOwnCall(t *testing.T) {
	prev := runtime.GOMAXPROCS(8)
	defer runtime.GOMAXPROCS(prev)

	const (
		workers   = 64
		perWorker = 2000
		polluters = 4
	)
	var stop int32
	var pw sync.WaitGroup
	for i := 0; i < polluters; i++ {
		pw.Add(1)
		go func() {
			defer pw.Done()
			runtime.LockOSThread()
			defer runtime.UnlockOSThread()
			for atomic.LoadInt32(&stop) == 0 {
				rawcgo.PolluteLastError()
			}
		}()
	}

	var bad, empty, total int64 // atomic (sync/atomic typed values need Go 1.19; go.mod says 1.18)
	var first atomic.Value
	var ww sync.WaitGroup
	for i := 0; i < workers; i++ {
		ww.Add(1)
		go func() {
			defer ww.Done()
			for j := 0; j < perWorker; j++ {
				_, err := gibson.NewContext(gibson.RenderMode(99))
				atomic.AddInt64(&total, 1)
				switch {
				case err == nil:
					atomic.AddInt64(&bad, 1)
					first.CompareAndSwap(nil, "<nil error>")
				case err.Error() == "":
					atomic.AddInt64(&empty, 1)
					atomic.AddInt64(&bad, 1)
					first.CompareAndSwap(nil, "<empty>")
				case err.Error() != wantMsg:
					atomic.AddInt64(&bad, 1)
					first.CompareAndSwap(nil, err.Error())
				}
				if j%64 == 0 {
					runtime.Gosched() // invite the scheduler to move us between threads
				}
			}
		}()
	}
	ww.Wait()
	atomic.StoreInt32(&stop, 1)
	pw.Wait()

	t.Logf("trials=%d wrong=%d (empty=%d) first=%v", atomic.LoadInt64(&total), atomic.LoadInt64(&bad), atomic.LoadInt64(&empty), first.Load())
	if atomic.LoadInt64(&bad) != 0 {
		t.Fatalf("NewContext returned a wrong error message in %d of %d trials (first: %v)",
			atomic.LoadInt64(&bad), atomic.LoadInt64(&total), first.Load())
	}
}
