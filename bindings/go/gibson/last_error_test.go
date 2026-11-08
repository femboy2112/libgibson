package gibson_test

import (
	"fmt"
	"runtime"
	"sync"
	"sync/atomic"
	"testing"

	"github.com/femboy2112/libgibson/bindings/go/gibson"
	"github.com/femboy2112/libgibson/bindings/go/internal/rawcgo"
)

// Each worker passes its own invalid mode (100+i), which gibson_create_context rejects with
// "Invalid render mode: <mode>" before any terminal is touched, so this runs the real NewContext
// failure path with or without a TTY, and each worker can recognize its own message.
func modeMsg(mode int) string { return fmt.Sprintf("Invalid render mode: %d", mode) }

// TestNewContextErrorIsFromItsOwnCall hammers NewContext from many goroutines while polluter
// goroutines keep overwriting the last error of whatever OS thread they run on with a different
// text. The last error is thread-local in Rust. If NewContext read it with a second cgo call that
// landed on another thread, it would return an empty message (a thread with no error yet) or a
// foreign one: another worker's mode, or the polluters' text. The polluters are not locked to
// threads, so the threads they write are the same ones the workers use. Detection is
// probabilistic: a broken wrapper is caught only when the scheduler moves a goroutine between
// the two calls, so a single run can pass on a broken wrapper; a fixed one cannot fail this way.
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
			for n := 0; atomic.LoadInt32(&stop) == 0; n++ {
				rawcgo.PolluteLastError()
				if n%16 == 0 {
					runtime.Gosched() // move on, leaving this thread's last error polluted
				}
			}
		}()
	}

	var bad, empty, foreign, total int64 // atomic (typed atomics need Go 1.19; go.mod says 1.18)
	var first atomic.Value
	var ww sync.WaitGroup
	for i := 0; i < workers; i++ {
		ww.Add(1)
		mode := 100 + i
		want := modeMsg(mode)
		go func() {
			defer ww.Done()
			for j := 0; j < perWorker; j++ {
				_, err := gibson.NewContext(gibson.RenderMode(mode))
				atomic.AddInt64(&total, 1)
				switch {
				case err == nil:
					atomic.AddInt64(&bad, 1)
					first.CompareAndSwap(nil, "<nil error>")
				case err.Error() == "":
					atomic.AddInt64(&empty, 1)
					atomic.AddInt64(&bad, 1)
					first.CompareAndSwap(nil, "<empty>")
				case err.Error() != want:
					atomic.AddInt64(&foreign, 1)
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

	t.Logf("trials=%d wrong=%d (empty=%d foreign=%d) first=%v", atomic.LoadInt64(&total),
		atomic.LoadInt64(&bad), atomic.LoadInt64(&empty), atomic.LoadInt64(&foreign), first.Load())
	if atomic.LoadInt64(&bad) != 0 {
		t.Fatalf("NewContext returned a wrong error message in %d of %d trials (first: %v)",
			atomic.LoadInt64(&bad), atomic.LoadInt64(&total), first.Load())
	}
}
