package wire

import (
	"bytes"
	"math"
	"os"
	"path/filepath"
	"testing"
)

// The committed cross-language fixture, decoded+re-encoded identically by the Rust
// (tests/wire_conformance.rs) and Python (bindings/python/test_wire.py) halves. This
// Go half proves a third, independent implementation agrees byte-for-byte.
func goldenPath() string {
	return filepath.Join("..", "..", "python", "testdata", "golden_field_v1.bin")
}

func floatsEqual(a, b []float64) bool {
	if len(a) != len(b) {
		return false
	}
	for i := range a {
		// Exact bit-for-bit compare so ±inf / NaN payloads are not glossed over.
		if math.Float64bits(a[i]) != math.Float64bits(b[i]) {
			return false
		}
	}
	return true
}

func boolsEqual(a, b []bool) bool {
	if len(a) != len(b) {
		return false
	}
	for i := range a {
		if a[i] != b[i] {
			return false
		}
	}
	return true
}

// Decode every frame of the committed golden and assert its exact contents — the Go
// half of the cross-language conformance gate.
func TestGoldenRoundTrips(t *testing.T) {
	data, err := os.ReadFile(goldenPath())
	if err != nil {
		t.Fatalf("read golden: %v", err)
	}
	var msgs []*Message
	off := 0
	for off < len(data) {
		msg, consumed, err := Decode(data[off:])
		if err != nil {
			t.Fatalf("decode at offset %d: %v", off, err)
		}
		if msg == nil {
			t.Fatalf("golden vector truncated at offset %d", off)
		}
		msgs = append(msgs, msg)
		off += consumed
	}
	if off != len(data) {
		t.Fatalf("consumed %d bytes, file is %d", off, len(data))
	}
	if len(msgs) != 5 {
		t.Fatalf("expected 5 frames, got %d", len(msgs))
	}

	f1 := msgs[0]
	if f1.Kind != MsgField || f1.Epoch != 1 || f1.Sequence != 1 {
		t.Errorf("f1 header: kind=%d epoch=%d seq=%d", f1.Kind, f1.Epoch, f1.Sequence)
	}
	if !floatsEqual(f1.Field.Data, []float64{0, 1, 2, 3, 4, 5}) {
		t.Errorf("f1 data = %v", f1.Field.Data)
	}
	if f1.Field.Mask != nil {
		t.Errorf("f1 mask should be nil, got %v", f1.Field.Mask)
	}
	if f1.Field.Cols != 3 || f1.Field.Rows != 2 || f1.Field.Origin != [2]float64{-1.0, 2.0} ||
		f1.Field.CellW != 0.5 || f1.Field.CellH != 0.25 || f1.Field.ModelTick != 100 {
		t.Errorf("f1 geometry: %+v", f1.Field)
	}

	f2 := msgs[1]
	if f2.Kind != MsgField || f2.Epoch != 1 || f2.Sequence != 2 {
		t.Errorf("f2 header: kind=%d epoch=%d seq=%d", f2.Kind, f2.Epoch, f2.Sequence)
	}
	if f2.Field.Data[0] != 42.0 {
		t.Errorf("f2[0] = %v, want 42", f2.Field.Data[0])
	}
	if !math.IsInf(f2.Field.Data[1], 1) {
		t.Errorf("f2[1] = %v, want +inf", f2.Field.Data[1])
	}
	if !math.IsInf(f2.Field.Data[2], -1) {
		t.Errorf("f2[2] = %v, want -inf", f2.Field.Data[2])
	}
	if f2.Field.Data[3] != 7.5 {
		t.Errorf("f2[3] = %v, want 7.5", f2.Field.Data[3])
	}
	if !boolsEqual(f2.Field.Mask, []bool{true, false, true, true}) {
		t.Errorf("f2 mask = %v", f2.Field.Mask)
	}

	f3 := msgs[2]
	if f3.Kind != MsgCommand || f3.Command.CommandID != 7 || f3.Command.Kind != 2 ||
		!bytes.Equal(f3.Command.Arg, []byte("hi")) {
		t.Errorf("f3 command = %+v", f3.Command)
	}

	f4 := msgs[3]
	if f4.Kind != MsgAck || f4.Ack.Status != AckAcknowledged || f4.Ack.CommandID != 7 {
		t.Errorf("f4 ack = %+v", f4.Ack)
	}

	f5 := msgs[4]
	if f5.Kind != MsgRetire {
		t.Errorf("f5 kind = %d, want retire", f5.Kind)
	}
}

// Re-encode the five frames from scratch and assert the bytes equal the committed
// golden — the strict byte-for-byte interop proof (mirrors the Rust/Python halves).
func TestReencodeMatchesGolden(t *testing.T) {
	golden, err := os.ReadFile(goldenPath())
	if err != nil {
		t.Fatalf("read golden: %v", err)
	}
	var buf bytes.Buffer

	f1, err := EncodeField(1, 1, 1, 3, 2, [2]float64{-1.0, 2.0}, 0.5, 0.25, 100,
		[]float64{0, 1, 2, 3, 4, 5}, nil)
	if err != nil {
		t.Fatal(err)
	}
	buf.Write(f1)

	f2, err := EncodeField(1, 1, 2, 2, 2, [2]float64{0, 0}, 1.0, 1.0, 101,
		[]float64{42, math.Inf(1), math.Inf(-1), 7.5}, []bool{true, false, true, true})
	if err != nil {
		t.Fatal(err)
	}
	buf.Write(f2)

	buf.Write(EncodeCommand(1, 0, 1, 7, 2, []byte("hi")))
	buf.Write(EncodeAck(1, 0, 2, 7, AckAcknowledged))
	buf.Write(EncodeRetire(1, 1, 3))

	if !bytes.Equal(buf.Bytes(), golden) {
		t.Fatalf("re-encode diverged from golden: produced %d bytes, golden %d",
			buf.Len(), len(golden))
	}
}

// A NaN payload survives encode→decode bit-for-bit, including a non-canonical one.
func TestNaNIsBitPreserved(t *testing.T) {
	const nanBits = uint64(0x7ff8000000000001)
	nan := math.Float64frombits(nanBits)
	f, err := EncodeField(1, 0, 0, 1, 1, [2]float64{0, 0}, 1.0, 1.0, 0, []float64{nan}, nil)
	if err != nil {
		t.Fatal(err)
	}
	msg, consumed, err := Decode(f)
	if err != nil || msg == nil {
		t.Fatalf("decode: msg=%v err=%v", msg, err)
	}
	if consumed != len(f) {
		t.Fatalf("consumed %d != %d", consumed, len(f))
	}
	if got := math.Float64bits(msg.Field.Data[0]); got != nanBits {
		t.Fatalf("NaN bits not preserved: got %#x, want %#x", got, nanBits)
	}
}

// A header declaring a huge payload is refused on the length field, before the frame
// is buffered (the pre-allocation guard).
func TestOversizedLengthRefusedBeforeBuffering(t *testing.T) {
	var hdr bytes.Buffer
	header(&hdr, MsgField, 0, 0, 0, 1_000_000_000) // 1e9-byte payload claim
	msg, consumed, err := DecodeWithLimits(hdr.Bytes(), 10_000, DefaultMaxFieldCells)
	if err == nil {
		t.Fatalf("expected error for oversized declared length; got msg=%v consumed=%d", msg, consumed)
	}
}

// A partial buffer asks for more bytes (nil msg, nil error), never a false decode.
func TestPartialBufferNeedsMoreBytes(t *testing.T) {
	retire := EncodeRetire(1, 1, 3) // a full 32-byte header, no payload
	if msg, consumed, err := Decode(retire[:20]); msg != nil || consumed != 0 || err != nil {
		t.Fatalf("partial header should need more: msg=%v consumed=%d err=%v", msg, consumed, err)
	}
	field, err := EncodeField(1, 1, 1, 3, 2, [2]float64{-1, 2}, 0.5, 0.25, 100,
		[]float64{0, 1, 2, 3, 4, 5}, nil)
	if err != nil {
		t.Fatal(err)
	}
	if msg, consumed, err := Decode(field[:len(field)-1]); msg != nil || consumed != 0 || err != nil {
		t.Fatalf("truncated payload should need more: msg=%v consumed=%d err=%v", msg, consumed, err)
	}
}

// Malformed frames are rejected, not silently accepted.
func TestRejectsMalformedFrames(t *testing.T) {
	good := EncodeRetire(1, 1, 3)

	// Bad magic.
	bad := append([]byte{}, good...)
	bad[0] = 'X'
	if _, _, err := Decode(bad); err == nil {
		t.Error("bad magic accepted")
	}

	// Unsupported version.
	bad = append([]byte{}, good...)
	bad[4] = 9
	if _, _, err := Decode(bad); err == nil {
		t.Error("bad version accepted")
	}

	// Unknown message type.
	bad = append([]byte{}, good...)
	bad[6] = 99
	if _, _, err := Decode(bad); err == nil {
		t.Error("unknown message type accepted")
	}

	// Retire with a non-empty declared payload.
	bad = append([]byte{}, good...)
	bad[28] = 4 // payload_len = 4, but no payload bytes follow
	bad = append(bad, 0, 0, 0, 0)
	if _, _, err := Decode(bad); err == nil {
		t.Error("retire with non-empty payload accepted")
	}

	// A field whose mask byte is neither 0 nor 1.
	f, err := EncodeField(1, 1, 1, 1, 1, [2]float64{0, 0}, 1.0, 1.0, 0, []float64{1.0}, []bool{true})
	if err != nil {
		t.Fatal(err)
	}
	f[len(f)-1] = 2 // corrupt the single mask byte
	if _, _, err := Decode(f); err == nil {
		t.Error("illegal mask byte accepted")
	}

	// Degenerate geometry (zero rows) with a self-consistent length.
	degen, err := EncodeField(1, 1, 1, 1, 1, [2]float64{0, 0}, 0.0, 1.0, 0, []float64{1.0}, nil)
	if err != nil {
		t.Fatal(err)
	}
	if _, _, err := Decode(degen); err == nil {
		t.Error("degenerate cell_w=0 geometry accepted")
	}
}
