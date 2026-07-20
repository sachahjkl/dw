package workspace

import (
	"encoding/json"
	"github.com/sachahjkl/dw/internal/l10n"
	"os"
	"path/filepath"
	"sort"
	"strings"
)

func ParseHandoff(text, expectedRepository string) (HandoffSummary, error) {
	lines := strings.Split(strings.ReplaceAll(text, "\r\n", "\n"), "\n")
	start := -1
	end := -1
	for index, line := range lines {
		if strings.EqualFold(strings.TrimSpace(line), "```yaml") {
			start = index
			break
		}
	}
	if start < 0 {
		return HandoffSummary{}, localized("workspace.error.handoff-missing-yaml")
	}
	for index := start + 1; index < len(lines); index++ {
		if strings.TrimSpace(lines[index]) == "```" {
			end = index
			break
		}
	}
	if end < 0 {
		return HandoffSummary{}, localized("workspace.error.handoff-missing-yaml-end")
	}
	status := ""
	repository := ""
	section := ""
	key := ""
	values := map[string]map[string][]string{"summary": {"done": {}, "decisions": {}, "risks": {}, "blockers": {}, "follow_up": {}}, "verification": {"commands": {}, "manual_checks": {}}, "artifacts": {"files": {}, "screenshots": {}, "attachments": {}}}
	for _, line := range lines[start+1 : end] {
		if strings.TrimSpace(line) == "" {
			continue
		}
		indent := len(line) - len(strings.TrimLeft(line, " "))
		trimmed := strings.TrimSpace(line)
		if indent == 0 {
			key = ""
			if trimmed == "summary:" || trimmed == "verification:" || trimmed == "artifacts:" {
				section = strings.TrimSuffix(trimmed, ":")
				continue
			}
			name, value, ok := splitYAML(trimmed)
			if !ok {
				continue
			}
			if equalFold(name, "status") {
				status = trimScalar(value)
			} else if equalFold(name, "repository") {
				repository = trimScalar(value)
			}
			continue
		}
		if indent == 2 {
			bucket, ok := values[section]
			if !ok {
				return HandoffSummary{}, localized("workspace.error.handoff-unknown-section", l10n.A("line", trimmed))
			}
			name, value, ok := splitYAML(trimmed)
			if !ok {
				return HandoffSummary{}, localized("workspace.error.handoff-unknown-key", l10n.A("section", section), l10n.A("line", trimmed))
			}
			list, ok := bucket[name]
			if !ok {
				return HandoffSummary{}, localized("workspace.error.handoff-unknown-key", l10n.A("section", section), l10n.A("line", trimmed))
			}
			key = name
			if value != "[]" && trimScalar(value) != "" {
				bucket[name] = append(list, trimScalar(value))
			}
			continue
		}
		if indent >= 4 && strings.HasPrefix(trimmed, "- ") {
			if section == "" || key == "" {
				return HandoffSummary{}, localized("workspace.error.handoff-list-outside-section", l10n.A("line", trimmed))
			}
			values[section][key] = append(values[section][key], trimScalar(strings.TrimPrefix(trimmed, "- ")))
			continue
		}
		return HandoffSummary{}, localized("workspace.error.handoff-unsupported-line", l10n.A("line", trimmed))
	}
	if strings.TrimSpace(status) == "" {
		return HandoffSummary{}, localized("workspace.error.handoff-missing-status")
	}
	if strings.TrimSpace(repository) == "" {
		return HandoffSummary{}, localized("workspace.error.handoff-missing-repository")
	}
	if !equalFold(repository, expectedRepository) {
		return HandoffSummary{}, localized("workspace.error.handoff-repository-mismatch", l10n.A("expected", expectedRepository), l10n.A("actual", repository))
	}
	normalized := HandoffStatus(strings.ReplaceAll(strings.ToLower(strings.TrimSpace(status)), "-", "_"))
	if normalized != HandoffTodo && normalized != HandoffInProgress && normalized != HandoffDone && normalized != HandoffBlocked {
		return HandoffSummary{}, localized("workspace.error.handoff-invalid-status", l10n.A("status", status))
	}
	return HandoffSummary{Repository: repository, Status: normalized, Done: values["summary"]["done"], Decisions: values["summary"]["decisions"], Risks: values["summary"]["risks"], Blockers: values["summary"]["blockers"], FollowUp: values["summary"]["follow_up"]}, nil
}
func splitYAML(value string) (string, string, bool) {
	index := strings.IndexByte(value, ':')
	if index < 0 {
		return "", "", false
	}
	return strings.TrimSpace(value[:index]), strings.TrimSpace(value[index+1:]), true
}
func trimScalar(value string) string {
	value = strings.TrimSpace(value)
	if len(value) >= 2 && ((value[0] == '"' && value[len(value)-1] == '"') || (value[0] == '\'' && value[len(value)-1] == '\'')) {
		return value[1 : len(value)-1]
	}
	return value
}
func ValidateHandoffs(workspace string) (HandoffValidationReport, error) {
	manifest, err := ReadManifest(filepath.Join(workspace, ManifestFile))
	if err != nil {
		return HandoffValidationReport{}, err
	}
	items := make([]HandoffValidationItem, 0, len(manifest.Repositories))
	valid := true
	for _, repository := range manifest.Repositories {
		path := filepath.Join(workspace, HandoffPrefix+repository+".md")
		data, readErr := os.ReadFile(path)
		if os.IsNotExist(readErr) {
			valid = false
			items = append(items, HandoffValidationItem{Repository: repository, Path: path, Status: "missing", Detail: HandoffValidationDetail{Kind: "missing-file"}})
			continue
		}
		if readErr != nil {
			return HandoffValidationReport{}, localizedOperation("read handoff", readErr)
		}
		summary, parseErr := ParseHandoff(string(data), repository)
		if parseErr != nil {
			valid = false
			items = append(items, HandoffValidationItem{Repository: repository, Path: path, Status: "invalid", Detail: HandoffValidationDetail{Kind: "invalid-file", Reason: parseErr.Error()}})
			continue
		}
		itemValid := summary.Status == HandoffDone
		status, detail := string(summary.Status), "not-finish-ready"
		if itemValid {
			status, detail = "valid", "valid"
		} else {
			valid = false
		}
		items = append(items, HandoffValidationItem{Repository: repository, Path: path, Status: status, Valid: itemValid, Detail: HandoffValidationDetail{Kind: detail}, DoneCount: len(summary.Done), DecisionCount: len(summary.Decisions), RiskCount: len(summary.Risks), BlockerCount: len(summary.Blockers), FollowUpCount: len(summary.FollowUp)})
	}
	return HandoffValidationReport{SchemaVersion: HandoffValidationVersion, Workspace: workspace, Project: manifest.Project, Items: items, IsValid: valid}, nil
}

func DiscoverAIContextFiles(workspace string) []string {
	result := make([]string, 0)
	_ = filepath.WalkDir(workspace, func(path string, entry os.DirEntry, err error) error {
		if err != nil {
			return nil
		}
		if !entry.IsDir() && strings.HasPrefix(entry.Name(), "ai-context") && strings.HasSuffix(entry.Name(), ".json") {
			result = append(result, path)
		}
		return nil
	})
	sort.Strings(result)
	return result
}
func BuildPreflight(workspace string, files []string) (PreflightReport, error) {
	manifest, err := ReadManifest(filepath.Join(workspace, ManifestFile))
	if err != nil {
		return PreflightReport{}, err
	}
	if len(files) == 0 {
		files = DiscoverAIContextFiles(workspace)
	}
	if len(files) == 0 {
		return PreflightReport{}, localized("workspace.error.preflight-no-context")
	}
	issues := make([]PreflightIssue, 0)
	for _, path := range files {
		data, readErr := os.ReadFile(path)
		if readErr != nil {
			return PreflightReport{}, localizedCause("workspace.error.preflight-context-missing", readErr, l10n.A("path", path))
		}
		var contextItem aiContext
		if err = json.Unmarshal(data, &contextItem); err != nil {
			return PreflightReport{}, localizedCause("workspace.error.preflight-context-invalid", err, l10n.A("path", path))
		}
		var manifestItem *WorkItem
		for _, item := range manifest.ParentWorkItems() {
			if item.ID == contextItem.WorkItem.ID {
				copy := item
				manifestItem = &copy
				break
			}
		}
		if manifestItem != nil {
			reasons := make([]string, 0)
			if !pointerEqual(manifestItem.Title, contextItem.WorkItem.Title) {
				reasons = append(reasons, "title")
			}
			if !pointerEqual(manifestItem.State, contextItem.WorkItem.State) {
				reasons = append(reasons, "state")
			}
			if !pointerEqual(manifestItem.Type, contextItem.WorkItem.Type) {
				reasons = append(reasons, "kind")
			}
			if len(reasons) > 0 {
				detail, _ := json.Marshal(struct {
					Kind    string   `json:"kind"`
					Reasons []string `json:"reasons"`
				}{"workspace-provider-context-stale", reasons})
				issues = append(issues, PreflightIssue{Code: "workspace.provider-context.stale", Severity: "warning", WorkItemID: contextItem.WorkItem.ID, Detail: detail, RelatedIDs: []string{contextItem.WorkItem.ID}})
			}
		}
		if len(contextItem.Attachments.Items) > 0 {
			names := make([]string, 0)
			for _, attachment := range contextItem.Attachments.Items {
				if attachment.Name != nil {
					names = append(names, *attachment.Name)
				}
			}
			detail, _ := json.Marshal(struct {
				Kind          string   `json:"kind"`
				DirectoryHint string   `json:"directoryHint"`
				Names         []string `json:"names"`
			}{"provider-attachments-present", contextItem.Attachments.DirectoryHint, names})
			issues = append(issues, PreflightIssue{Code: "provider.attachments.present", Severity: "warning", WorkItemID: contextItem.WorkItem.ID, Detail: detail, RelatedIDs: []string{contextItem.WorkItem.ID}})
		}
	}
	blocking := false
	for _, issue := range issues {
		if issue.Severity == "blocking" {
			blocking = true
		}
	}
	ids := make([]string, 0)
	for _, item := range manifest.ParentWorkItems() {
		ids = append(ids, item.ID)
	}
	return PreflightReport{SchemaVersion: PreflightVersion, Workspace: workspace, Project: manifest.Project, WorkItemIDs: ids, Issues: issues, HasBlockingIssues: blocking}, nil
}

type aiContext struct {
	WorkItem struct {
		ID    string  `json:"id"`
		Type  *string `json:"type"`
		Title *string `json:"title"`
		State *string `json:"state"`
	} `json:"workItem"`
	Attachments struct {
		DirectoryHint string `json:"directoryHint"`
		Items         []struct {
			Name *string `json:"name"`
		} `json:"items"`
	} `json:"attachments"`
}

func pointerEqual(left, right *string) bool {
	if left == nil || right == nil {
		return left == nil && right == nil
	}
	return *left == *right
}
