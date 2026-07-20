package console

import (
	"errors"
	"strconv"

	"github.com/sachahjkl/dw/internal/update"
	"github.com/sachahjkl/dw/internal/wirejson"
)

func RegisterUpdateRenderers(results *Registry, events *EventRegistry) error {
	if err := RegisterPageResult(results, ResultUpgrade, updatePage); err != nil {
		return err
	}
	if err := RegisterPageResult(results, update.ActionID, updatePage); err != nil {
		return err
	}
	return RegisterEvent(events, update.ActionID, updateEventProjection)
}

func updatePage(report update.Report) Page {
	page := Page{Title: "update.title"}
	switch report.Kind {
	case "check":
		page.Badge = "update.check.badge"
		page.Status = StatusSuccess
		if report.Check != nil {
			page.Summary = []Field{
				{Label: "update.version", Value: report.Check.Version, Style: ValueSuccess},
				{Label: "update.release", Value: report.Check.ReleaseTag},
				{Label: "update.commit", Value: report.Check.Commit},
			}
			rows := make([][]string, len(report.Check.Assets))
			for i, asset := range report.Check.Assets {
				rows[i] = []string{asset.RID, asset.FileName, asset.SHA256}
			}
			page.Sections = []Section{{Title: "update.assets", Table: &Table{Columns: []MessageID{"update.rid", "update.file", "update.sha256"}, Rows: rows}}}
		}
	case "installed":
		page.Badge = "update.installed.badge"
		page.Status = StatusSuccess
		if report.Installed != nil {
			page.Summary = []Field{
				{Label: "update.version", Value: report.Installed.Version, Style: ValueSuccess},
				{Label: "update.commit", Value: report.Installed.Commit},
				{Label: "update.executable", Value: report.Installed.ExecutablePath, Style: ValuePath},
			}
			if report.Installed.DeferredWindowsReplacement {
				page.Sections = []Section{{Panels: []Panel{{Title: "update.replacement", Body: ""}}}}
			}
		}
	default:
		page.Badge = "update.invalid.badge"
		page.Status = StatusFailure
	}
	return page
}

func updateEventProjection(event update.Event) EventProjection {
	projection := EventProjection{ActionID: event.ActionID(), Transient: event.Kind == "downloaded-asset-bytes"}
	switch event.Kind {
	case "fetching-release":
		projection.Fields = []EventField{{Key: "repository", Value: event.Owner + "/" + event.Repository}}
	case "fetching-manifest":
		projection.Fields = []EventField{{Key: "asset", Value: event.AssetName}}
	case "selecting-asset":
		projection.Fields = []EventField{{Key: "rid", Value: event.RID}}
	case "downloading-asset":
		projection.Fields = []EventField{{Key: "file", Value: event.FileName}}
	case "downloaded-asset-bytes":
		projection.Fields = []EventField{{Key: "file", Value: event.FileName}, {Key: "received", Value: strconv.FormatInt(event.Received, 10)}}
		if event.Total != nil {
			projection.Fields = append(projection.Fields, EventField{Key: "total", Value: strconv.FormatInt(*event.Total, 10)})
		}
	case "verifying-checksum":
		projection.Fields = []EventField{{Key: "file", Value: event.FileName}, {Key: "expected_sha256", Value: event.ExpectedSHA256}}
	case "preparing-executable":
		projection.Fields = []EventField{{Key: "file", Value: event.FileName}, {Key: "rid", Value: event.RID}}
	case "replacing-executable":
		projection.Fields = []EventField{{Key: "path", Value: event.ExecutablePath}}
	case "completed":
		projection.Fields = []EventField{{Key: "version", Value: event.Version}}
	}
	return projection
}

func UpdateJSONProjection(report update.Report) (JSONProjection, error) {
	if report.Kind == "check" && report.Check != nil {
		assets := make([]wirejson.Value, len(report.Check.Assets))
		for i, asset := range report.Check.Assets {
			assets[i] = JSONObject(
				JSONField{Name: "rid", Value: wirejson.StringValue(asset.RID)},
				JSONField{Name: "file_name", Value: wirejson.StringValue(asset.FileName)},
				JSONField{Name: "sha256", Value: wirejson.StringValue(asset.SHA256)},
			).Value
		}
		return JSONObject(
			JSONField{Name: "kind", Value: wirejson.StringValue("check")},
			JSONField{Name: "release_tag", Value: wirejson.StringValue(report.Check.ReleaseTag)},
			JSONField{Name: "version", Value: wirejson.StringValue(report.Check.Version)},
			JSONField{Name: "commit", Value: wirejson.StringValue(report.Check.Commit)},
			JSONField{Name: "assets", Value: wirejson.ArrayValue(assets...)},
		), nil
	}
	if report.Kind == "installed" && report.Installed != nil {
		return JSONObject(
			JSONField{Name: "kind", Value: wirejson.StringValue("installed")},
			JSONField{Name: "version", Value: wirejson.StringValue(report.Installed.Version)},
			JSONField{Name: "commit", Value: wirejson.StringValue(report.Installed.Commit)},
			JSONField{Name: "executable_path", Value: wirejson.StringValue(report.Installed.ExecutablePath)},
			JSONField{Name: "deferred_windows_replacement", Value: wirejson.BoolValue(report.Installed.DeferredWindowsReplacement)},
		), nil
	}
	return JSONProjection{}, errors.New("console.invalid-update-report")
}
