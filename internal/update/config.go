package update

import (
	"bytes"
	"encoding/json"
	"fmt"
	"io"
	"strings"
)

func DefaultConfig() Config {
	return Config{
		Owner:      DefaultOwner,
		Repository: DefaultRepository,
		AssetName:  DefaultManifestAsset,
	}
}

// ParseWorkflowConfig reads only workflow.json's updates object. Other workflow
// fields and unknown update fields remain the config package's responsibility.
func ParseWorkflowConfig(reader io.Reader) (Config, error) {
	var workflow struct {
		Updates json.RawMessage `json:"updates"`
	}
	if err := decodeJSON(reader, &workflow); err != nil {
		return Config{}, fmt.Errorf("update: decode-workflow: %w", err)
	}
	return ResolveConfig(workflow.Updates)
}

// ResolveConfig applies the Rust updater's defaults. Blank or non-string values
// are treated as absent, preserving existing permissive workflow files.
func ResolveConfig(raw json.RawMessage) (Config, error) {
	config := DefaultConfig()
	trimmed := bytes.TrimSpace(raw)
	if len(trimmed) == 0 || bytes.Equal(trimmed, []byte("null")) {
		return config, nil
	}
	if !json.Valid(trimmed) {
		return Config{}, fmt.Errorf("update: decode-updates-config")
	}
	if trimmed[0] != '{' {
		return config, nil
	}
	var value map[string]json.RawMessage
	if err := json.Unmarshal(trimmed, &value); err != nil {
		return Config{}, fmt.Errorf("update: decode-updates-config: %w", err)
	}
	readString := func(key string) string {
		var text string
		if field, ok := value[key]; ok && json.Unmarshal(field, &text) == nil {
			return strings.TrimSpace(text)
		}
		return ""
	}
	if owner := readString("owner"); owner != "" {
		config.Owner = owner
	}
	if repository := readString("repository"); repository != "" {
		config.Repository = repository
	}
	if asset := readString("assetName"); asset != "" {
		config.AssetName = asset
	}
	if field, ok := value["includePrerelease"]; ok {
		_ = json.Unmarshal(field, &config.IncludePrerelease)
	}
	return config, nil
}

func normalizeConfig(config Config) (Config, error) {
	defaults := DefaultConfig()
	config.Owner = strings.TrimSpace(config.Owner)
	config.Repository = strings.TrimSpace(config.Repository)
	config.AssetName = strings.TrimSpace(config.AssetName)
	if config.Owner == "" {
		config.Owner = defaults.Owner
	}
	if config.Repository == "" {
		config.Repository = defaults.Repository
	}
	if config.AssetName == "" {
		config.AssetName = defaults.AssetName
	}
	return config, nil
}
