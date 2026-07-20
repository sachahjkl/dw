package update

import (
	"bytes"
	"encoding/json"
	"fmt"
	"io"
)

// ParseManifest intentionally ignores schema, channel, and unknown fields. This
// matches serde's permissive release reader while retaining required fields.
func ParseManifest(reader io.Reader) (Manifest, error) {
	var wire struct {
		Version *string `json:"version"`
		Commit  *string `json:"commit"`
		Assets  *[]struct {
			RID      *string         `json:"rid"`
			FileName *string         `json:"fileName"`
			SHA256   *string         `json:"sha256"`
			URL      json.RawMessage `json:"url"`
		} `json:"assets"`
	}
	if err := decodeJSON(reader, &wire); err != nil {
		return Manifest{}, fmt.Errorf("update: decode-manifest: %w", err)
	}
	if wire.Version == nil || wire.Commit == nil || wire.Assets == nil {
		return Manifest{}, fmt.Errorf("update: manifest-missing-required-field")
	}
	manifest := Manifest{Version: *wire.Version, Commit: *wire.Commit, Assets: make([]Asset, 0, len(*wire.Assets))}
	for index, item := range *wire.Assets {
		if item.RID == nil || item.FileName == nil || item.SHA256 == nil {
			return Manifest{}, fmt.Errorf("update: manifest-asset-%d-missing-required-field", index)
		}
		url := ""
		if len(item.URL) != 0 {
			trimmedURL := bytes.TrimSpace(item.URL)
			if bytes.Equal(trimmedURL, []byte("null")) || json.Unmarshal(trimmedURL, &url) != nil {
				return Manifest{}, fmt.Errorf("update: manifest-asset-%d-invalid-url", index)
			}
		}
		manifest.Assets = append(manifest.Assets, Asset{RID: *item.RID, FileName: *item.FileName, SHA256: *item.SHA256, URL: url})
	}
	return manifest, nil
}
