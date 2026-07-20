package workspace

import (
	"fmt"
	"path/filepath"
	"strings"
	"unicode"

	"github.com/sachahjkl/dw/internal/agent"
)

func NormalizeSlug(value string) string {
	var output strings.Builder
	previousDash := false
	for _, r := range strings.TrimSpace(value) {
		r = foldRune(r)
		lower := unicode.ToLower(r)
		if lower >= 'a' && lower <= 'z' || lower >= '0' && lower <= '9' {
			output.WriteRune(lower)
			previousDash = false
		} else if !previousDash {
			output.WriteByte('-')
			previousDash = true
		}
	}
	slug := strings.Trim(output.String(), "-")
	if len(slug) > 50 {
		slug = strings.Trim(slug[:50], "-")
	}
	return slug
}
func SlugOrFallback(value, fallback string) string {
	slug := NormalizeSlug(value)
	if slug == "" {
		return NormalizeSlug(fallback)
	}
	return slug
}
func BuildBranchName(kind string, ids []string, slug string) string {
	kind = strings.ToLower(strings.TrimSpace(kind))
	if kind == "" {
		kind = "feat"
	}
	return fmt.Sprintf("%s/%s-%s", kind, strings.Join(distinctCSV(ids), "-"), NormalizeSlug(slug))
}
func BuildSubjectName(kind string, ids []string, slug string) string {
	kind = strings.ToLower(strings.TrimSpace(kind))
	if kind == "" {
		kind = "feat"
	}
	return fmt.Sprintf("%s-%s-%s", kind, strings.Join(distinctCSV(ids), "-"), NormalizeSlug(slug))
}
func foldRune(r rune) rune {
	switch r {
	case 'à', 'á', 'â', 'ã', 'ä', 'å', 'À', 'Á', 'Â', 'Ã', 'Ä', 'Å':
		return 'a'
	case 'ç', 'Ç':
		return 'c'
	case 'è', 'é', 'ê', 'ë', 'È', 'É', 'Ê', 'Ë':
		return 'e'
	case 'ì', 'í', 'î', 'ï', 'Ì', 'Í', 'Î', 'Ï':
		return 'i'
	case 'ñ', 'Ñ':
		return 'n'
	case 'ò', 'ó', 'ô', 'õ', 'ö', 'Ò', 'Ó', 'Ô', 'Õ', 'Ö':
		return 'o'
	case 'ù', 'ú', 'û', 'ü', 'Ù', 'Ú', 'Û', 'Ü':
		return 'u'
	case 'ý', 'ÿ', 'Ý':
		return 'y'
	default:
		return r
	}
}

func PlanMarkdown(manifest Manifest) string {
	ids := make([]string, 0)
	for _, item := range manifest.ParentWorkItems() {
		ids = append(ids, "#"+item.ID)
	}
	repositories := make([]string, 0)
	for _, repository := range manifest.Repositories {
		repositories = append(repositories, "- "+repository+": TODO")
	}
	return fmt.Sprintf("# Plan — Work Items %s\n\nProject: `%s`\n\n## Functional Summary\n\nTODO\n\n## Affected Repositories\n\n%s\n\n## Code Analysis\n\nTODO\n\n## Technical Plan\n\nTODO\n\n## Risks\n\nTODO\n\n## Verification\n\nTODO\n", strings.Join(ids, ", "), manifest.Project, strings.Join(repositories, "\n"))
}
func HandoffMarkdown(manifest Manifest, repository string) string {
	ids := make([]string, 0)
	for _, item := range manifest.ParentWorkItems() {
		ids = append(ids, "`#"+item.ID+"`")
	}
	return fmt.Sprintf("# Handoff %s\n\n## Context\n\n- Project: `%s`\n- Repository: `%s`\n- Branch: `%s`\n- Parent work items: %s\n- Known child tasks: (none)\n\n## Deterministic Inputs\n\n1. `task.json`\n2. `plan.md`\n3. `AGENTS.md`\n4. Work-provider AI context for every parent work item\n5. Workspace preflight report\n\n## Scope\n\nDescribe in `plan.md` what belongs to `%s` and what this handoff must deliver.\n\n## Constraints\n\n- Preserve exact domain terminology\n- Treat screenshots, mockups, and attachments as factual sources\n- Ask the user instead of guessing when context is missing\n- Verify API impacts and front/back contracts when relevant\n\n## Expected Work\n\n- Limit work to `%s`\n- List affected files and areas clearly\n- Report dependencies on other domains\n- Update the structured summary below\n\n## Required Structured Summary\n\nFill this block without changing its keys.\n\n```yaml\nstatus: todo\nrepository: %s\nsummary:\n  done: []\n  decisions: []\n  risks: []\n  blockers: []\n  follow_up: []\nverification:\n  commands: []\n  manual_checks: []\nartifacts:\n  files: []\n  screenshots: []\n  attachments: []\n```\n", repository, manifest.Project, repository, manifest.BranchName, strings.Join(ids, ", "), repository, repository, repository)
}

func AgentFiles(manifest Manifest) []agent.WorkspaceConfigFile {
	items := make([]agent.WorkspaceWorkItemRef, 0)
	for _, item := range manifest.ParentWorkItems() {
		items = append(items, agent.WorkspaceWorkItemRef{ID: item.ID, Kind: item.Type, Title: item.Title})
	}
	return agent.WorkspaceConfigFiles(agent.WorkspaceConfigRequest{WorkItems: items, Project: manifest.Project})
}
func WriteGeneratedFiles(workspace string, manifest Manifest) error {
	for _, file := range AgentFiles(manifest) {
		if err := writeFileAtomic(filepath.Join(workspace, file.RelativePath), []byte(file.Content), 0o644); err != nil {
			return err
		}
	}
	return nil
}
