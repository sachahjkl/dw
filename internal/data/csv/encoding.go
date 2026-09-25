package csv

import (
	"bufio"
	"fmt"
	"io"
	"strings"
	"unicode/utf8"
)

// windows1252High maps bytes 0x80-0x9F; undefined positions keep their C1 code point like Windows does.
var windows1252High = [32]rune{
	'€', 0x81, '‚', 'ƒ', '„', '…', '†', '‡', 'ˆ', '‰', 'Š', '‹', 'Œ', 0x8D, 'Ž', 0x8F,
	0x90, '‘', '’', '“', '”', '•', '–', '—', '˜', '™', 'š', '›', 'œ', 0x9D, 'ž', 'Ÿ',
}

func decodingReader(source io.Reader, encoding string) (io.Reader, error) {
	switch strings.ToLower(strings.TrimSpace(encoding)) {
	case "", "utf-8", "utf8":
		return source, nil
	case "windows-1252", "cp1252":
		return &singleByteReader{source: bufio.NewReader(source), windows1252: true}, nil
	case "latin1", "latin-1", "iso-8859-1":
		return &singleByteReader{source: bufio.NewReader(source)}, nil
	default:
		return nil, fmt.Errorf("csv.unsupported-encoding:%s", encoding)
	}
}

type singleByteReader struct {
	source      *bufio.Reader
	windows1252 bool
	pending     []byte
}

func (reader *singleByteReader) Read(target []byte) (int, error) {
	written := 0
	for written < len(target) {
		if len(reader.pending) > 0 {
			count := copy(target[written:], reader.pending)
			reader.pending = reader.pending[count:]
			written += count
			continue
		}
		value, err := reader.source.ReadByte()
		if err != nil {
			if written > 0 {
				return written, nil
			}
			return 0, err
		}
		decoded := rune(value)
		if reader.windows1252 && value >= 0x80 && value <= 0x9F {
			decoded = windows1252High[value-0x80]
		}
		var buffer [utf8.UTFMax]byte
		size := utf8.EncodeRune(buffer[:], decoded)
		reader.pending = append(reader.pending[:0], buffer[:size]...)
	}
	return written, nil
}
