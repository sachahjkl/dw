package update

import (
	"encoding/json"
	"fmt"
	"io"
)

// decodeJSON accepts insignificant trailing whitespace but rejects a second
// value, matching serde_json::from_str rather than Decoder.Decode's stream mode.
func decodeJSON(reader io.Reader, destination any) error {
	decoder := json.NewDecoder(reader)
	if err := decoder.Decode(destination); err != nil {
		return err
	}
	var trailing json.RawMessage
	if err := decoder.Decode(&trailing); err != io.EOF {
		if err == nil {
			return fmt.Errorf("update: trailing-json-value")
		}
		return err
	}
	return nil
}

func limitedReader(reader io.Reader, size int64) io.Reader {
	return io.LimitReader(reader, size+1)
}

func validateContentLength(contentLength, maximum int64, kind string) error {
	if contentLength > maximum {
		return fmt.Errorf("update: %s-too-large: content-length=%d maximum=%d", kind, contentLength, maximum)
	}
	return nil
}

func readLimited(reader io.Reader, maximum int64, kind string) ([]byte, error) {
	contents, err := io.ReadAll(limitedReader(reader, maximum))
	if err != nil {
		return nil, err
	}
	if int64(len(contents)) > maximum {
		return nil, fmt.Errorf("update: %s-too-large: maximum=%d", kind, maximum)
	}
	return contents, nil
}
