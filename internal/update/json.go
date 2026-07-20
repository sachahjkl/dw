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
