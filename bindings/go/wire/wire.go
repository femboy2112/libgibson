// Package wire is a pure-Go, dependency-free codec for the LibGibson `gibson::wire`
// byte protocol — the SAME transport-agnostic format the Rust presenter and the
// reference Python producer speak (docs/WIRE_PROTOCOL.md).
//
// It is a faithful mirror of bindings/python/wire.py: it links nothing, imports only
// the Go standard library (encoding/binary, math, bytes), and never calls into
// libgibson. It is a PROTOCOL re-implementation, not an FFI binding — it does not
// wrap the C ABI (that is bindings/go/gibson) and does not drive the renderer. Its
// whole job is to prove that an independent Go process can speak wire v1 and is
// byte-identical to the Rust/Python encoders on the committed cross-language golden
// vector (bindings/python/testdata/golden_field_v1.bin). This is a vertical-slice
// interop proof for issue #142; it is NOT full SDK parity, which remains a separate
// later acceptance gate.
//
// The byte format is the contract (little-endian throughout):
//
//	Header (32 bytes):
//	    magic[4]="GWIR" | version u16=1 | msg_type u16 | epoch u32 | sequence u64
//	    | source u64 | payload_len u32
//
//	FIELD payload (msg_type=1):
//	    cols u32 | rows u32 | origin_x f64 | origin_y f64 | cell_w f64 | cell_h f64
//	    | model_tick u64 | flags u32 | data[cols*rows] f64 (row-major) | mask[cols*rows] u8?
//	    flags bit0 = has_mask; mask byte 1 = present/valid, 0 = masked.
//
//	COMMAND payload (msg_type=2): command_id u64 | kind u32 | arg_len u32 | arg[arg_len]
//	ACK     payload (msg_type=3): command_id u64 | status u32
//	RETIRE  payload (msg_type=4): (empty)
//
// Non-finite data (NaN, ±inf) is carried faithfully as its IEEE-754 bit pattern,
// never coerced.
package wire

import (
	"bytes"
	"encoding/binary"
	"fmt"
	"math"
)

// Magic is the 4-byte frame tag.
var Magic = [4]byte{'G', 'W', 'I', 'R'}

const (
	// Version is the wire protocol version. Must match gibson::wire::VERSION.
	Version uint16 = 1
	// HeaderBytes is the fixed frame-header size.
	HeaderBytes = 32
)

// Message kinds (msg_type).
const (
	MsgField   uint16 = 1
	MsgCommand uint16 = 2
	MsgAck     uint16 = 3
	MsgRetire  uint16 = 4
)

// FlagHasMask is field flags bit 0.
const FlagHasMask uint32 = 1

// Ack status codes — must match gibson::wire::AckStatus.
const (
	AckAccepted     uint32 = 0
	AckDelivered    uint32 = 1
	AckAcknowledged uint32 = 2
	AckRefused      uint32 = 3
	AckFailed       uint32 = 4
)

// Default ceilings mirroring gibson::wire::WireLimits.
const (
	DefaultMaxFieldCells   uint64 = 1 << 20
	DefaultMaxPayloadBytes uint64 = DefaultMaxFieldCells*(8+1) + 52 + 64
)

const fieldHeadBytes = 52
const commandHeadBytes = 16
const ackBytes = 12

// FieldBody is a decoded scalar-field observation.
type FieldBody struct {
	Cols, Rows   uint32
	Origin       [2]float64
	CellW, CellH float64
	ModelTick    uint64
	Data         []float64 // flat row-major, len == Cols*Rows
	Mask         []bool    // nil when absent; else len == Cols*Rows, true = present/valid
}

// CommandBody is a decoded edit for the producer.
type CommandBody struct {
	CommandID uint64
	Kind      uint32
	Arg       []byte
}

// AckBody is a decoded acknowledgement.
type AckBody struct {
	CommandID uint64
	Status    uint32
}

// Message is a decoded frame: the source/chronology header plus exactly one typed
// body (the field matching Kind is non-nil; Retire carries no body).
type Message struct {
	Source   uint64
	Epoch    uint32
	Sequence uint64
	Kind     uint16

	Field   *FieldBody
	Command *CommandBody
	Ack     *AckBody
}

func header(buf *bytes.Buffer, msgType uint16, source uint64, epoch uint32, sequence uint64, payloadLen uint32) {
	buf.Write(Magic[:])
	_ = binary.Write(buf, binary.LittleEndian, Version)
	_ = binary.Write(buf, binary.LittleEndian, msgType)
	_ = binary.Write(buf, binary.LittleEndian, epoch)
	_ = binary.Write(buf, binary.LittleEndian, sequence)
	_ = binary.Write(buf, binary.LittleEndian, source)
	_ = binary.Write(buf, binary.LittleEndian, payloadLen)
}

func frame(msgType uint16, source uint64, epoch uint32, sequence uint64, payload []byte) []byte {
	buf := new(bytes.Buffer)
	header(buf, msgType, source, epoch, sequence, uint32(len(payload)))
	buf.Write(payload)
	return buf.Bytes()
}

// EncodeField encodes a scalar-field observation. data is a flat row-major slice of
// cols*rows floats (index = iy*cols + ix); mask, if non-nil, is a parallel slice where
// true means present/valid. Returns an error if the lengths do not match cols*rows.
func EncodeField(source uint64, epoch uint32, sequence uint64, cols, rows uint32, origin [2]float64, cellW, cellH float64, modelTick uint64, data []float64, mask []bool) ([]byte, error) {
	cells := uint64(cols) * uint64(rows)
	if uint64(len(data)) != cells {
		return nil, fmt.Errorf("wire: data has %d values, expected %d", len(data), cells)
	}
	hasMask := mask != nil
	if hasMask && uint64(len(mask)) != cells {
		return nil, fmt.Errorf("wire: mask has %d values, expected %d", len(mask), cells)
	}
	var flags uint32
	if hasMask {
		flags = FlagHasMask
	}
	p := new(bytes.Buffer)
	_ = binary.Write(p, binary.LittleEndian, cols)
	_ = binary.Write(p, binary.LittleEndian, rows)
	_ = binary.Write(p, binary.LittleEndian, origin[0])
	_ = binary.Write(p, binary.LittleEndian, origin[1])
	_ = binary.Write(p, binary.LittleEndian, cellW)
	_ = binary.Write(p, binary.LittleEndian, cellH)
	_ = binary.Write(p, binary.LittleEndian, modelTick)
	_ = binary.Write(p, binary.LittleEndian, flags)
	for _, v := range data {
		// binary.Write uses math.Float64bits, preserving ±inf / NaN bit patterns.
		_ = binary.Write(p, binary.LittleEndian, v)
	}
	if hasMask {
		b := make([]byte, cells)
		for i, m := range mask {
			if m {
				b[i] = 1
			}
		}
		p.Write(b)
	}
	return frame(MsgField, source, epoch, sequence, p.Bytes()), nil
}

// EncodeCommand encodes an edit for the producer.
func EncodeCommand(source uint64, epoch uint32, sequence uint64, commandID uint64, kind uint32, arg []byte) []byte {
	p := new(bytes.Buffer)
	_ = binary.Write(p, binary.LittleEndian, commandID)
	_ = binary.Write(p, binary.LittleEndian, kind)
	_ = binary.Write(p, binary.LittleEndian, uint32(len(arg)))
	p.Write(arg)
	return frame(MsgCommand, source, epoch, sequence, p.Bytes())
}

// EncodeAck encodes an acknowledgement.
func EncodeAck(source uint64, epoch uint32, sequence uint64, commandID uint64, status uint32) []byte {
	p := new(bytes.Buffer)
	_ = binary.Write(p, binary.LittleEndian, commandID)
	_ = binary.Write(p, binary.LittleEndian, status)
	return frame(MsgAck, source, epoch, sequence, p.Bytes())
}

// EncodeRetire encodes a source retirement (no payload).
func EncodeRetire(source uint64, epoch uint32, sequence uint64) []byte {
	return frame(MsgRetire, source, epoch, sequence, nil)
}

// Decode decodes one frame from the front of buf using the default ceilings. It
// returns (msg, consumed, nil) when a whole frame is present, (nil, 0, nil) when more
// bytes are needed, and (nil, 0, err) on a malformed or out-of-contract frame — the
// same three-way contract as the Python decode_message.
func Decode(buf []byte) (*Message, int, error) {
	return DecodeWithLimits(buf, DefaultMaxPayloadBytes, DefaultMaxFieldCells)
}

// DecodeWithLimits is Decode with explicit pre-allocation ceilings. The payload-length
// cap is enforced on the header's length field BEFORE the full frame is required, so a
// hostile oversized declaration is refused without buffering it.
func DecodeWithLimits(buf []byte, maxPayload, maxCells uint64) (*Message, int, error) {
	if len(buf) < HeaderBytes {
		return nil, 0, nil // need more
	}
	if !bytes.Equal(buf[0:4], Magic[:]) {
		return nil, 0, fmt.Errorf("wire: bad magic %q", buf[0:4])
	}
	version := binary.LittleEndian.Uint16(buf[4:6])
	msgType := binary.LittleEndian.Uint16(buf[6:8])
	epoch := binary.LittleEndian.Uint32(buf[8:12])
	sequence := binary.LittleEndian.Uint64(buf[12:20])
	source := binary.LittleEndian.Uint64(buf[20:28])
	payloadLen := uint64(binary.LittleEndian.Uint32(buf[28:32]))
	if version != Version {
		return nil, 0, fmt.Errorf("wire: unsupported version %d", version)
	}
	if payloadLen > maxPayload {
		return nil, 0, fmt.Errorf("wire: payload %d exceeds cap %d", payloadLen, maxPayload)
	}
	frameLen := uint64(HeaderBytes) + payloadLen
	if uint64(len(buf)) < frameLen {
		return nil, 0, nil // need more
	}
	p := buf[HeaderBytes:frameLen]

	msg := &Message{Source: source, Epoch: epoch, Sequence: sequence, Kind: msgType}
	switch msgType {
	case MsgField:
		fb, err := decodeField(p, maxCells)
		if err != nil {
			return nil, 0, err
		}
		msg.Field = fb
	case MsgCommand:
		cb, err := decodeCommand(p)
		if err != nil {
			return nil, 0, err
		}
		msg.Command = cb
	case MsgAck:
		ab, err := decodeAck(p)
		if err != nil {
			return nil, 0, err
		}
		msg.Ack = ab
	case MsgRetire:
		if payloadLen != 0 {
			return nil, 0, fmt.Errorf("wire: retire frame must have an empty payload")
		}
	default:
		return nil, 0, fmt.Errorf("wire: unknown message type %d", msgType)
	}
	return msg, int(frameLen), nil
}

func decodeField(p []byte, maxCells uint64) (*FieldBody, error) {
	if len(p) < fieldHeadBytes {
		return nil, fmt.Errorf("wire: field payload shorter than its head")
	}
	cols := binary.LittleEndian.Uint32(p[0:4])
	rows := binary.LittleEndian.Uint32(p[4:8])
	ox := math.Float64frombits(binary.LittleEndian.Uint64(p[8:16]))
	oy := math.Float64frombits(binary.LittleEndian.Uint64(p[16:24]))
	cw := math.Float64frombits(binary.LittleEndian.Uint64(p[24:32]))
	ch := math.Float64frombits(binary.LittleEndian.Uint64(p[32:40]))
	modelTick := binary.LittleEndian.Uint64(p[40:48])
	flags := binary.LittleEndian.Uint32(p[48:52])
	if flags&^FlagHasMask != 0 {
		return nil, fmt.Errorf("wire: unknown field flags %#x", flags)
	}
	hasMask := flags&FlagHasMask != 0
	cells := uint64(cols) * uint64(rows)
	if cells > maxCells {
		return nil, fmt.Errorf("wire: field %d cells exceeds cap %d", cells, maxCells)
	}
	var maskBytes uint64
	if hasMask {
		maskBytes = cells
	}
	computed := uint64(fieldHeadBytes) + cells*8 + maskBytes
	if computed != uint64(len(p)) {
		return nil, fmt.Errorf("wire: declared payload %d != computed %d", len(p), computed)
	}
	// NaN cw/ch fail these comparisons (matching the Python decoder), so a non-finite
	// geometry is not spuriously rejected here; zero/negative extents are.
	if cw <= 0.0 || ch <= 0.0 || cols == 0 || rows == 0 {
		return nil, fmt.Errorf("wire: degenerate field geometry")
	}
	data := make([]float64, cells)
	off := uint64(fieldHeadBytes)
	for i := uint64(0); i < cells; i++ {
		data[i] = math.Float64frombits(binary.LittleEndian.Uint64(p[off : off+8]))
		off += 8
	}
	var mask []bool
	if hasMask {
		mask = make([]bool, cells)
		for i := uint64(0); i < cells; i++ {
			b := p[off]
			if b != 0 && b != 1 {
				return nil, fmt.Errorf("wire: mask byte %d is neither 0 nor 1", b)
			}
			mask[i] = b == 1
			off++
		}
	}
	return &FieldBody{
		Cols: cols, Rows: rows, Origin: [2]float64{ox, oy},
		CellW: cw, CellH: ch, ModelTick: modelTick, Data: data, Mask: mask,
	}, nil
}

func decodeCommand(p []byte) (*CommandBody, error) {
	if len(p) < commandHeadBytes {
		return nil, fmt.Errorf("wire: command payload shorter than its head")
	}
	commandID := binary.LittleEndian.Uint64(p[0:8])
	kind := binary.LittleEndian.Uint32(p[8:12])
	argLen := uint64(binary.LittleEndian.Uint32(p[12:16]))
	computed := uint64(commandHeadBytes) + argLen
	if computed != uint64(len(p)) {
		return nil, fmt.Errorf("wire: declared payload %d != computed %d", len(p), computed)
	}
	arg := make([]byte, argLen)
	copy(arg, p[commandHeadBytes:])
	return &CommandBody{CommandID: commandID, Kind: kind, Arg: arg}, nil
}

func decodeAck(p []byte) (*AckBody, error) {
	if len(p) != ackBytes {
		return nil, fmt.Errorf("wire: ack payload wrong size")
	}
	commandID := binary.LittleEndian.Uint64(p[0:8])
	status := binary.LittleEndian.Uint32(p[8:12])
	if status > AckFailed {
		return nil, fmt.Errorf("wire: ack status %d out of range", status)
	}
	return &AckBody{CommandID: commandID, Status: status}, nil
}
