package gibson_test

import (
	"bytes"
	"errors"
	"fmt"
	"runtime"
	"sync"
	"testing"

	"github.com/femboy2112/libgibson/bindings/go/gibson"
	"github.com/femboy2112/libgibson/bindings/go/internal/rawcgo"
)

// Existing exported signatures stay as they were: these assignments fail to compile if one changes.
var (
	_ func(gibson.RenderMode) (*gibson.Context, error)       = gibson.NewContext
	_ func(*gibson.Context, *gibson.Node)                    = (*gibson.Context).SetRoot
	_ func(*gibson.Context) error                            = (*gibson.Context).Render
	_ func(string) *gibson.Node                              = gibson.NewTextNode
	_ func(*gibson.Node, *gibson.Node) *gibson.Node          = (*gibson.Node).AddChild
	_ func(*gibson.Context)                                  = (*gibson.Context).Close
	_ func(*gibson.Line, string, *gibson.Style) *gibson.Line = (*gibson.Line).AddStyledSpan
)

func headless(t *testing.T, cols, rows uint16) *gibson.Context {
	t.Helper()
	ctx, err := gibson.NewHeadlessContext(gibson.ModeFullscreen, cols, rows)
	if err != nil {
		t.Fatalf("NewHeadlessContext: %v", err)
	}
	t.Cleanup(ctx.Close)
	return ctx
}

func must(t *testing.T, err error) {
	t.Helper()
	if err != nil {
		t.Fatal(err)
	}
}

func mount(t *testing.T, ctx *gibson.Context, style *gibson.Style, lines ...string) {
	t.Helper()
	root := gibson.NewColNode()
	for _, l := range lines {
		n, err := gibson.NewStyledTextNode(l, style, gibson.WrapNone)
		must(t, err)
		root.AddChild(n)
	}
	must(t, ctx.SetRootNode(root))
	must(t, ctx.Render())
}

func asError(t *testing.T, err error) *gibson.Error {
	t.Helper()
	var ge *gibson.Error
	if !errors.As(err, &ge) {
		t.Fatalf("want *gibson.Error, got %T %v", err, err)
	}
	return ge
}

func TestConstants(t *testing.T) {
	if gibson.KeyEnter != 0x1001 || gibson.KeyPageDown != 0x100D || gibson.EventQueueCapacity != 1024 {
		t.Fatal("constants drifted from gibson.h")
	}
	if gibson.KeyKindChar != 1 || gibson.KeyKindSpecial != 2 || gibson.ModAlt != 4 || gibson.StatusQueueFull != -5 {
		t.Fatal("constants drifted from gibson.h")
	}
}

func TestEmptyQueue(t *testing.T) {
	ctx := headless(t, 40, 12)
	ev, err := ctx.PollEventKind(0)
	must(t, err)
	if ev != (gibson.Event{HasKind: true, KeyKind: gibson.KeyKindNone}) {
		t.Fatalf("empty poll: %+v", ev)
	}
	ev, err = ctx.PollEvent(0)
	must(t, err)
	if ev != (gibson.Event{}) {
		t.Fatalf("kind-less empty poll: %+v", ev)
	}
}

func TestKeyKinds(t *testing.T) {
	ctx := headless(t, 40, 12)
	for _, e := range []gibson.Event{
		gibson.CharKey('a', gibson.ModShift|gibson.ModControl),
		gibson.CharKey(0x1001, 0),             // a typed U+1001 (Myanmar letter)
		gibson.SpecialKey(gibson.KeyEnter, 0), // the constant with the same value
		gibson.CharKey(0x1F600, gibson.ModAlt),
		gibson.LegacyKey(0x1001, 0), // kind-less: always the special key
		gibson.LegacyKey('z', 0),
	} {
		must(t, ctx.PushEvent(e))
	}
	var got [6]gibson.Event
	for i := range got {
		ev, err := ctx.PollEventKind(0)
		must(t, err)
		got[i] = ev
	}
	if got[0].KeyCode != 'a' || got[0].KeyKind != gibson.KeyKindChar || got[0].Modifiers != gibson.ModShift|gibson.ModControl || !got[0].HasKind {
		t.Errorf("char a: %+v", got[0])
	}
	if got[1].KeyCode != 0x1001 || got[1].KeyKind != gibson.KeyKindChar {
		t.Errorf("typed U+1001 must stay CHAR: %+v", got[1])
	}
	if got[2].KeyCode != 0x1001 || got[2].KeyKind != gibson.KeyKindSpecial {
		t.Errorf("KeyEnter must stay SPECIAL: %+v", got[2])
	}
	if got[3].KeyCode != 0x1F600 || got[3].Modifiers != gibson.ModAlt {
		t.Errorf("astral char: %+v", got[3])
	}
	if got[4].KeyKind != gibson.KeyKindSpecial || got[4].KeyCode != gibson.KeyEnter {
		t.Errorf("kind-less 0x1001 is ENTER: %+v", got[4])
	}
	if got[5].KeyKind != gibson.KeyKindChar {
		t.Errorf("kind-less 'z' is CHAR: %+v", got[5])
	}
	if ev, _ := ctx.PollEventKind(0); ev.Type != gibson.EventNone {
		t.Errorf("queue not empty: %+v", ev)
	}

	must(t, ctx.PushEvent(gibson.CharKey(0x1001, 0)))
	must(t, ctx.PushEvent(gibson.SpecialKey(gibson.KeyEnter, 0)))
	k1, _ := ctx.PollEvent(0)
	k2, _ := ctx.PollEvent(0)
	if k1.KeyCode != 0x1001 || k2.KeyCode != 0x1001 || k1.HasKind || k2.HasKind {
		t.Errorf("kind-less poll conflates them, per gibson.h: %+v %+v", k1, k2)
	}
}

func TestPaste(t *testing.T) {
	ctx := headless(t, 40, 12)
	text := "héllo 世界\n\U0001F600"
	must(t, ctx.PushEvent(gibson.PasteText(text)))
	ev, err := ctx.PollEventKind(0)
	must(t, err)
	if ev.Type != gibson.EventPaste || !ev.HasPaste || ev.Paste != text || ev.KeyKind != gibson.KeyKindNone {
		t.Fatalf("paste: %+v", ev)
	}
	must(t, ctx.PushEvent(gibson.PasteText("")))
	if ev, _ = ctx.PollEventKind(0); !ev.HasPaste || ev.Paste != "" {
		t.Fatalf("empty paste must be present, not absent: %+v", ev)
	}
	ge := asError(t, ctx.PushEvent(gibson.PasteText("\xc3\x28")))
	if ge.Status != gibson.StatusInvalidParam || ge.LastError == "" {
		t.Errorf("invalid UTF-8: %+v", ge)
	}
	ge = asError(t, ctx.PushEvent(gibson.Event{Type: gibson.EventPaste})) // HasPaste false: NULL paste_str
	if ge.Status != gibson.StatusInvalidParam {
		t.Errorf("NULL paste: %+v", ge)
	}
	if err := ctx.PushEvent(gibson.PasteText("a\x00b")); err == nil {
		t.Error("NUL in paste must be refused")
	}
	if ev, _ = ctx.PollEventKind(0); ev.Type != gibson.EventNone {
		t.Errorf("refused pastes were queued: %+v", ev)
	}
}

func TestResize(t *testing.T) {
	ctx := headless(t, 40, 12)
	mount(t, ctx, nil, "x")
	if c, r, _ := ctx.FrameSize(); c != 40 || r != 12 {
		t.Fatalf("initial %dx%d", c, r)
	}
	must(t, ctx.PushEvent(gibson.ResizeEvent(60, 16)))
	// A queued resize does nothing, even across a render, until it is polled.
	must(t, ctx.Render())
	if c, r, _ := ctx.FrameSize(); c != 40 || r != 12 {
		t.Fatalf("queued resize changed the frame to %dx%d", c, r)
	}
	ev, err := ctx.PollEventKind(0)
	must(t, err)
	if ev.Type != gibson.EventResize || ev.Cols != 60 || ev.Rows != 16 || ev.KeyKind != gibson.KeyKindNone {
		t.Fatalf("resize event: %+v", ev)
	}
	must(t, ctx.Render())
	if c, r, _ := ctx.FrameSize(); c != 60 || r != 16 {
		t.Fatalf("polled resize gave %dx%d", c, r)
	}
	for _, z := range [][2]uint16{{0, 12}, {12, 0}, {0, 0}} {
		ge := asError(t, ctx.PushEvent(gibson.ResizeEvent(z[0], z[1])))
		if ge.Status != gibson.StatusInvalidParam {
			t.Errorf("resize %v: %+v", z, ge)
		}
	}
	if ev, _ = ctx.PollEventKind(0); ev.Type != gibson.EventNone {
		t.Errorf("refused resizes were queued: %+v", ev)
	}
}

func TestQueueFull(t *testing.T) {
	ctx := headless(t, 40, 12)
	// Distinguishable events (U+4E00 + i), so a displaced, reordered or inserted event shows up.
	queued := make([]gibson.Event, gibson.EventQueueCapacity)
	for i := range queued {
		queued[i] = gibson.CharKey(uint32(0x4E00+i), 0)
		must(t, ctx.PushEvent(queued[i]))
	}
	overflow := gibson.CharKey('Z', 0)
	for i := 0; i < 2; i++ {
		err := ctx.PushEvent(overflow)
		ge := asError(t, err)
		if ge.Status != gibson.StatusQueueFull || !errors.Is(err, gibson.ErrQueueFull) || ge.LastError == "" {
			t.Fatalf("overflow: %+v", ge)
		}
	}
	var drained []gibson.Event
	for {
		ev, err := ctx.PollEventKind(0)
		must(t, err)
		if ev.Type == gibson.EventNone {
			break
		}
		drained = append(drained, ev)
	}
	if len(drained) != len(queued) {
		t.Fatalf("drained %d, queued %d", len(drained), len(queued))
	}
	for i := range queued {
		if drained[i] != queued[i] {
			t.Fatalf("event %d: %+v, want %+v (no 'Z', nothing displaced)", i, drained[i], queued[i])
		}
	}
	must(t, ctx.PushEvent(gibson.CharKey('r', 0)))
	if ev, _ := ctx.PollEventKind(0); ev.KeyCode != 'r' {
		t.Errorf("queue refused after drain: %+v", ev)
	}
}

func TestInvalidParams(t *testing.T) {
	ctx := headless(t, 40, 12)
	bad := gibson.CharKey('a', 0)
	bad.KeyKind = 9
	for name, ev := range map[string]gibson.Event{
		"surrogate as CHAR":   gibson.CharKey(0xD800, 0),
		"non-special SPECIAL": gibson.SpecialKey('a', 0),
		"unknown kind":        bad,
		"reserved modifier":   gibson.CharKey('a', 0x80),
	} {
		err := ctx.PushEvent(ev)
		ge := asError(t, err)
		if ge.Status != gibson.StatusInvalidParam || errors.Is(err, gibson.ErrQueueFull) {
			t.Errorf("%s: %+v", name, ge)
		}
	}
	if ev, _ := ctx.PollEventKind(0); ev.Type != gibson.EventNone {
		t.Errorf("refused keys were queued: %+v", ev)
	}
	if _, err := gibson.NewHeadlessContext(gibson.ModeFullscreen, 0, 12); asError(t, err).Status != gibson.StatusInvalidParam {
		t.Error("zero cols")
	}
	if _, err := gibson.NewHeadlessContext(gibson.RenderMode(7), 40, 12); asError(t, err).LastError == "" {
		t.Error("bad mode must carry the last-error text")
	}
	if _, err := ctx.FrameCell(0, 0); asError(t, err).Status != gibson.StatusInvalidParam {
		t.Error("no frame yet")
	}
}

func TestPollThenPushUnchanged(t *testing.T) {
	ctx := headless(t, 40, 12)
	in := []gibson.Event{
		gibson.CharKey('a', gibson.ModShift), gibson.CharKey(0x1001, 0),
		gibson.SpecialKey(gibson.KeyTab, gibson.ModControl), gibson.PasteText("round\ntrip 世"),
		gibson.ResizeEvent(33, 9),
	}
	for _, e := range in {
		must(t, ctx.PushEvent(e))
	}
	var first, second []gibson.Event
	for range in {
		ev, err := ctx.PollEventKind(0)
		must(t, err)
		first = append(first, ev)
	}
	for _, e := range first {
		must(t, ctx.PushEvent(e)) // as polled, nothing rebuilt
	}
	for range in {
		ev, err := ctx.PollEventKind(0)
		must(t, err)
		second = append(second, ev)
	}
	for i := range first {
		if first[i] != second[i] {
			t.Errorf("event %d changed: %+v -> %+v", i, first[i], second[i])
		}
	}
	if first[1].KeyKind != gibson.KeyKindChar || first[2].KeyKind != gibson.KeyKindSpecial {
		t.Error("kinds did not survive")
	}
	must(t, ctx.PushEvent(gibson.CharKey(0x1001, 0)))
	a, _ := ctx.PollEvent(0)
	must(t, ctx.PushEvent(a))
	if b, _ := ctx.PollEvent(0); b != a {
		t.Errorf("kind-less loop: %+v -> %+v", a, b)
	}
	// An empty-queue EventNone is not pushable: the C ABI refuses it.
	none, _ := ctx.PollEventKind(0)
	if none.Type != gibson.EventNone {
		t.Fatal("queue should be empty")
	}
	if ge := asError(t, ctx.PushEvent(none)); ge.Status != gibson.StatusInvalidParam {
		t.Errorf("pushing NONE: %+v", ge)
	}
}

func TestReadbackAndCapture(t *testing.T) {
	ctx := headless(t, 20, 4)
	if c, r, err := ctx.FrameSize(); err != nil || c != 0 || r != 0 {
		t.Fatalf("no frame yet: %d %d %v", c, r, err)
	}
	if _, err := ctx.FrameCells(0, 0, -1); asError(t, err).Status != gibson.StatusInvalidParam {
		t.Error("FrameCells before a frame")
	}
	style := &gibson.Style{Fg: gibson.Color{Type: gibson.ColorRGB, R: 10, G: 20, B: 30}, Bold: true}
	zwj := "\U0001F468‍\U0001F469‍\U0001F467‍\U0001F466"
	root := gibson.NewColNode()
	for _, n := range []struct {
		s  string
		st *gibson.Style
	}{{"ab", style}, {zwj, nil}} {
		node, err := gibson.NewStyledTextNode(n.s, n.st, gibson.WrapNone)
		must(t, err)
		root.AddChild(node)
	}
	must(t, ctx.SetRootNode(root))
	must(t, ctx.Render())

	if c, r, _ := ctx.FrameSize(); c != 20 || r != 4 {
		t.Fatalf("frame %dx%d", c, r)
	}
	c0, err := ctx.FrameCell(0, 0)
	must(t, err)
	if string(c0.Glyph) != "a" || !c0.Style.Bold || c0.Style.Fg != (gibson.Color{Type: gibson.ColorRGB, R: 10, G: 20, B: 30}) {
		t.Errorf("styled cell: %+v", c0)
	}
	all, err := ctx.FrameCells(0, 0, -1)
	must(t, err)
	if len(all) != 80 {
		t.Errorf("all cells: %d", len(all))
	}
	if cs, _ := ctx.FrameCells(0, 3, -1); len(cs) != 20 {
		t.Errorf("last row: %d", len(cs))
	}
	if cs, _ := ctx.FrameCells(0, 0, 5); len(cs) != 5 || string(cs[1].Glyph) != "b" {
		t.Errorf("first five: %d", len(cs))
	}
	if cs, err := ctx.FrameCells(0, 0, 0); err != nil || len(cs) != 0 {
		t.Errorf("valid coordinates, nothing asked for: %d %v", len(cs), err)
	}
	// The C ABI judges every call, including one that returns no cells.
	for _, p := range [][3]int{{0, 4, -1}, {20, 0, -1}, {25, 0, 0}, {0, 4, 0}} {
		if _, err := ctx.FrameCells(uint16(p[0]), uint16(p[1]), p[2]); asError(t, err).Status != gibson.StatusInvalidParam {
			t.Errorf("FrameCells%v must be refused", p)
		}
	}
	z, err := ctx.FrameCell(0, 1)
	must(t, err)
	if !z.GlyphTruncated || len(z.Glyph) >= len(zwj) {
		t.Errorf("ZWJ cell should be truncated inline: %+v", z)
	}
	if g, err := ctx.FrameCellGlyph(0, 1); err != nil || string(g) != zwj {
		t.Errorf("full glyph: %q %v", g, err)
	}
	if g, err := ctx.FullGlyph(z, 0, 1); err != nil || string(g) != zwj {
		t.Errorf("FullGlyph: %q %v", g, err)
	}
	if g, _ := ctx.FullGlyph(c0, 0, 0); string(g) != "a" {
		t.Errorf("FullGlyph inline: %q", g)
	}

	wire, err := ctx.RenderedOutput()
	must(t, err)
	if again, _ := ctx.RenderedOutput(); len(wire) == 0 || !bytes.Equal(wire, again) {
		t.Error("RenderedOutput must copy, not drain")
	}
	taken, err := ctx.TakeRenderedOutput()
	must(t, err)
	if !bytes.Equal(taken, wire) {
		t.Error("take must return what get returned")
	}
	if left, err := ctx.RenderedOutput(); err != nil || len(left) != 0 {
		t.Errorf("capture after take: %d bytes, err %v; want 0 bytes, no error", len(left), err)
	}
	if again, err := ctx.TakeRenderedOutput(); err != nil || len(again) != 0 {
		t.Errorf("second take: %d bytes, err %v; want 0 bytes, no error", len(again), err)
	}
	// A changed frame, so there are bytes to discard (an unchanged frame writes none).
	mount(t, ctx, nil, "cd")
	have, err := ctx.RenderedOutput()
	must(t, err)
	n, err := ctx.ClearRenderedOutput()
	must(t, err)
	if n == 0 || n != len(have) {
		t.Errorf("clear removed %d of %d", n, len(have))
	}
	if left, err := ctx.RenderedOutput(); err != nil || len(left) != 0 {
		t.Errorf("capture after clear: %d bytes, err %v; want 0 bytes, no error", len(left), err)
	}
	if n, err := ctx.ClearRenderedOutput(); err != nil || n != 0 {
		t.Errorf("second clear: removed %d, err %v; want 0, no error", n, err)
	}
}

func TestLifecycle(t *testing.T) {
	ctx, err := gibson.NewHeadlessContext(gibson.ModeInline, 10, 3)
	must(t, err)
	ctx.Close()
	ctx.Close() // a second Close is a no-op
	n, err := gibson.NewStyledTextNode("x", nil, gibson.WrapNone)
	must(t, err)
	if !n.Valid() {
		t.Fatal("fresh node should be valid")
	}
	// A closed context refuses the node and leaves it with the caller, who can still free it.
	err = ctx.SetRootNode(n)
	var ge *gibson.Error
	if !errors.As(err, &ge) || ge.Status != gibson.StatusInvalidParam {
		t.Fatalf("SetRootNode on a closed context: %v, want a *gibson.Error with StatusInvalidParam", err)
	}
	if !n.Valid() {
		t.Fatal("a node refused by a closed context must stay with the caller")
	}
	n.Free()
	if n.Valid() {
		t.Fatal("freed node should not be valid")
	}
}

// The headless wrappers read the last error in the same cgo call as the failing call. Pollute the
// error of whatever thread the goroutines run on and require every worker to get its own text.
func TestHeadlessErrorIsFromItsOwnCall(t *testing.T) {
	prev := runtime.GOMAXPROCS(8)
	defer runtime.GOMAXPROCS(prev)
	stop := make(chan struct{})
	var pw sync.WaitGroup
	for i := 0; i < 4; i++ {
		pw.Add(1)
		go func() {
			defer pw.Done()
			for n := 0; ; n++ {
				select {
				case <-stop:
					return
				default:
				}
				rawcgo.PolluteLastError()
				if n%16 == 0 {
					runtime.Gosched()
				}
			}
		}()
	}
	var ww sync.WaitGroup
	errs := make(chan string, 64)
	for i := 0; i < 32; i++ {
		ww.Add(1)
		mode := 100 + i
		go func() {
			defer ww.Done()
			want := fmt.Sprintf("Invalid render mode: %d", mode)
			for j := 0; j < 1000; j++ {
				_, err := gibson.NewHeadlessContext(gibson.RenderMode(mode), 40, 12)
				var ge *gibson.Error
				if !errors.As(err, &ge) || ge.LastError != want {
					select {
					case errs <- fmt.Sprintf("mode %d: %v", mode, err):
					default:
					}
					return
				}
				if j%64 == 0 {
					runtime.Gosched()
				}
			}
		}()
	}
	ww.Wait()
	close(stop)
	pw.Wait()
	close(errs)
	for e := range errs {
		t.Error(e)
		break
	}
}
