package ado

import (
	"bytes"
	"encoding/json"
	"fmt"
	"strings"
)

func decodeObject(data []byte) (map[string]any, error) {
	decoder := json.NewDecoder(bytes.NewReader(data))
	decoder.UseNumber()
	var value map[string]any
	if err := decoder.Decode(&value); err != nil {
		return nil, &Error{Kind: ErrorJSON, Detail: err.Error(), Cause: err}
	}
	return value, nil
}

func object(value any) map[string]any {
	result, _ := value.(map[string]any)
	return result
}

func array(value any) []any {
	result, _ := value.([]any)
	return result
}

func elementText(value any) *string {
	if value == nil {
		return nil
	}
	var text string
	switch value := value.(type) {
	case string:
		text = value
	case json.Number:
		text = value.String()
	case float64:
		text = fmt.Sprint(value)
	case bool:
		text = fmt.Sprint(value)
	case map[string]any:
		name, ok := value["displayName"].(string)
		if !ok {
			return nil
		}
		text = name
	default:
		encoded, err := json.Marshal(value)
		if err != nil {
			return nil
		}
		text = string(encoded)
	}
	return &text
}

func fieldText(fields map[string]any, name string) *string { return elementText(fields[name]) }

func identityText(value any) *string {
	if identity := object(value); identity != nil {
		if displayName, ok := identity["displayName"].(string); ok {
			return &displayName
		}
	}
	return elementText(value)
}

func workItemIDFromRelationURL(value string) *string {
	const marker = "/workItems/"
	index := strings.Index(value, marker)
	if index < 0 {
		return nil
	}
	id := value[index+len(marker):]
	if delimiter := strings.IndexAny(id, "/?"); delimiter >= 0 {
		id = id[:delimiter]
	}
	return &id
}

func cleanText(value *string) *string {
	if value == nil {
		return nil
	}
	var output strings.Builder
	inTag := false
	for _, character := range *value {
		switch character {
		case '<':
			inTag = true
		case '>':
			inTag = false
		default:
			if !inTag {
				output.WriteRune(character)
			}
		}
	}
	cleaned := strings.TrimSpace(strings.ReplaceAll(output.String(), "&nbsp;", " "))
	if cleaned == "" {
		return nil
	}
	return &cleaned
}

func snapshotFromObject(value map[string]any) WorkItemSnapshot {
	fields := object(value["fields"])
	id := elementText(value["id"])
	result := WorkItemSnapshot{Type: fieldText(fields, "System.WorkItemType"), State: fieldText(fields, "System.State"), Title: fieldText(fields, "System.Title"), URL: fieldText(value, "url")}
	if id != nil {
		result.ID = *id
	}
	return result
}

func boolValue(value any) bool { result, _ := value.(bool); return result }

func int64Value(value any) (int64, bool) {
	switch value := value.(type) {
	case json.Number:
		result, err := value.Int64()
		return result, err == nil
	case float64:
		return int64(value), value == float64(int64(value))
	default:
		return 0, false
	}
}
