package console

import (
	"encoding/json"
	"errors"
	"strings"

	"github.com/sachahjkl/dw/internal/data"
	"github.com/sachahjkl/dw/internal/wirejson"
	"github.com/sachahjkl/dw/internal/workapp"
)

// JSONProjection is an explicit ordered machine projection. Renderers never
// derive JSON from human pages or reflect over domain results.
type JSONProjection struct{ Value wirejson.Value }

func ParseJSONProjection(data []byte) (JSONProjection, error) {
	value, err := wirejson.Parse(data)
	if err != nil {
		return JSONProjection{}, err
	}
	return JSONProjection{Value: value}, nil
}

// JSONProjectionOf converts an explicitly selected machine DTO into the
// ordered wirejson tree used for final deterministic encoding.
func JSONProjectionOf(dto any) (JSONProjection, error) {
	data, err := json.Marshal(dto)
	if err != nil {
		return JSONProjection{}, err
	}
	return ParseJSONProjection(data)
}

func RenderJSON(projection JSONProjection) (Output, error) {
	data, err := wirejson.Pretty(projection.Value)
	if err != nil {
		return Output{}, err
	}
	return Output{Format: FormatJSON, Body: data}, nil
}

type JSONField struct {
	Name  string
	Value wirejson.Value
}

func JSONObject(fields ...JSONField) JSONProjection {
	members := make([]wirejson.Member, len(fields))
	for i := range fields {
		members[i] = wirejson.Member{Name: fields[i].Name, Value: fields[i].Value}
	}
	return JSONProjection{Value: wirejson.ObjectValue(members...)}
}

func QueryJSONProjection(table data.Table) JSONProjection {
	columns := make([]wirejson.Value, len(table.Columns))
	for i := range table.Columns {
		columns[i] = wirejson.StringValue(table.Columns[i].Name)
	}
	rows := make([]wirejson.Value, len(table.Rows))
	for rowIndex, row := range table.Rows {
		cells := make([]wirejson.Value, len(row))
		for columnIndex := range row {
			cells[columnIndex] = row[columnIndex].JSONValue()
		}
		rows[rowIndex] = wirejson.ArrayValue(cells...)
	}
	return JSONObject(
		JSONField{Name: "columns", Value: wirejson.ArrayValue(columns...)},
		JSONField{Name: "rows", Value: wirejson.ArrayValue(rows...)},
		JSONField{Name: "truncated", Value: wirejson.BoolValue(table.Truncated)},
	)
}

// WorkAIContextJSONProjection preserves the established AI context wire schema
// while workapp remains provider-neutral.
func WorkAIContextJSONProjection(items []workapp.RichContextItem) (JSONProjection, error) {
	projection, err := JSONProjectionOf(items)
	if err != nil {
		return JSONProjection{}, err
	}
	values, ok := projection.Value.ArrayValues()
	if !ok {
		return JSONProjection{}, errors.New("console.invalid-ai-context-projection")
	}
	for index := range values {
		item := &values[index]
		if err := item.Set("schemaVersion", wirejson.StringValue("dw.ado.ai-context.v1")); err != nil {
			return JSONProjection{}, err
		}
		attachments, ok := item.ObjectAt("attachments")
		if !ok {
			continue
		}
		rewriteAttachmentDirectory(attachments, "directoryHint")
		attachmentItems, ok := attachments.Lookup("items")
		if !ok {
			continue
		}
		entries, ok := attachmentItems.ArrayValues()
		if !ok {
			continue
		}
		for entryIndex := range entries {
			rewriteAttachmentDirectory(&entries[entryIndex], "directoryHint")
		}
	}
	return projection, nil
}

func rewriteAttachmentDirectory(object *wirejson.Value, name string) {
	value, ok := object.Lookup(name)
	if !ok {
		return
	}
	directory, ok := value.AsString()
	if !ok || !strings.HasPrefix(directory, workapp.AttachmentDirectoryPrefix) {
		return
	}
	_ = object.Set(name, wirejson.StringValue("attachments/ado/"+strings.TrimPrefix(directory, workapp.AttachmentDirectoryPrefix)))
}

// RenderCompactJSON is for single-line machine protocols such as completion.
func RenderCompactJSON(projection JSONProjection) ([]byte, error) {
	return wirejson.Compact(projection.Value)
}
