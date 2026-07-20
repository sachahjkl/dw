package tui

import (
	"strconv"
	"strings"

	"github.com/sachahjkl/dw/internal/action"
	"github.com/sachahjkl/dw/internal/l10n"
)

type FieldKind uint8

const (
	TextField FieldKind = iota
	ToggleField
)

type FormField struct {
	ID          string
	Label       l10n.ID
	Help        l10n.ID
	Kind        FieldKind
	Value       string
	Required    bool
	Suggestions []string
}

func (f FormField) enabled() bool { return f.Value == "true" }

type FormTemplate struct {
	ID          string
	Label       l10n.ID
	Description l10n.ID
	ActionID    string
	Fields      func(Snapshot) []FormField
}

type FormMode uint8

const (
	ChooseTemplate FormMode = iota
	EditFields
)

type FormState struct {
	Mode          FormMode
	TemplateIndex int
	SelectedField int
	Fields        []FormField
}

func textField(id string, label l10n.ID, value string, required bool, suggestions []string) FormField {
	return FormField{ID: id, Label: label, Help: "tui.field.help.suggest", Kind: TextField, Value: value, Required: required, Suggestions: stableStrings(suggestions)}
}

func toggleField(id string, label l10n.ID, value bool) FormField {
	v := "false"
	if value {
		v = "true"
	}
	return FormField{ID: id, Label: label, Help: "tui.field.help.toggle", Kind: ToggleField, Value: v}
}

func first(values []string) string {
	if len(values) == 0 {
		return ""
	}
	return values[0]
}

func firstWorkspace(s Snapshot) string {
	if len(s.Workspaces) == 0 {
		return ""
	}
	return s.Workspaces[0].Path
}

func firstWorkItem(s Snapshot) string {
	for _, project := range s.ADOProjects {
		if len(project.Items) != 0 {
			return project.Items[0].ID
		}
	}
	for _, workspace := range s.Workspaces {
		if len(workspace.WorkItems) != 0 {
			return workspace.WorkItems[0]
		}
	}
	return ""
}

func firstPR(s Snapshot) (id, project, repository string) {
	for _, item := range s.PullRequests {
		if item.ID != "" {
			return item.ID, item.Project, item.Repository
		}
	}
	return "", first(s.Projects), first(s.Repositories)
}

func firstDB(s Snapshot) (project, database string) {
	if len(s.Databases) == 0 {
		return first(s.Projects), ""
	}
	return s.Databases[0].Project, s.Databases[0].Key
}

var formTemplates = [...]FormTemplate{
	{ID: "task-start", Label: "tui.form.task-start", Description: "tui.form.task-start.desc", ActionID: "task.start", Fields: func(s Snapshot) []FormField {
		return []FormField{
			textField("workItemIds", "tui.field.work-item", firstWorkItem(s), false, workItemSuggestions(s)),
			textField("project", "tui.field.project", first(s.Projects), false, s.Projects),
			textField("repositories", "tui.field.repository", "", false, s.Repositories),
			textField("type", "tui.field.type", "feature", false, []string{"feature", "bugfix", "hotfix", "chore"}),
			textField("slug", "tui.field.slug", "", false, nil),
			toggleField("skipAdo", "tui.field.skip-ado", false),
			toggleField("execute", "tui.field.execute", false),
		}
	}},
	{ID: "task-start-pr", Label: "tui.form.task-start-pr", Description: "tui.form.task-start-pr.desc", ActionID: "task.start-pr", Fields: func(s Snapshot) []FormField {
		id, project, repository := firstPR(s)
		return []FormField{
			textField("pullRequest", "tui.field.pull-request", id, true, pullRequestSuggestions(s)),
			textField("project", "tui.field.project", project, true, s.Projects),
			textField("repositories", "tui.field.repository", repository, false, s.Repositories),
			textField("type", "tui.field.type", "feature", false, []string{"feature", "bugfix", "hotfix", "chore"}),
			textField("slug", "tui.field.slug", "", false, nil),
			toggleField("execute", "tui.field.execute", false),
		}
	}},
	{ID: "task-finish", Label: "tui.form.task-finish", Description: "tui.form.task-finish.desc", ActionID: "task.finish", Fields: func(s Snapshot) []FormField {
		return []FormField{
			textField("workspace", "tui.field.workspace", firstWorkspace(s), false, workspaceSuggestions(s)),
			toggleField("continue", "tui.field.continue", false),
			textField("message", "tui.field.message", "", false, nil),
			toggleField("createPr", "tui.field.create-pr", false),
			toggleField("ready", "tui.field.ready", false),
			toggleField("skipVerify", "tui.field.skip-verify", false),
			toggleField("skipAdo", "tui.field.skip-ado", false),
			toggleField("execute", "tui.field.execute", false),
		}
	}},
	{ID: "task-teardown", Label: "tui.form.task-teardown", Description: "tui.form.task-teardown.desc", ActionID: "task.teardown", Fields: func(s Snapshot) []FormField {
		return []FormField{
			textField("workspace", "tui.field.workspace", firstWorkspace(s), false, workspaceSuggestions(s)),
			toggleField("continue", "tui.field.continue", false),
			textField("project", "tui.field.project", first(s.Projects), false, s.Projects),
			textField("workItemIds", "tui.field.work-item", "", false, workItemSuggestions(s)),
			toggleField("execute", "tui.field.execute", false),
		}
	}},
	{ID: "task-prune", Label: "tui.form.task-prune", Description: "tui.form.task-prune.desc", ActionID: "task.prune", Fields: func(s Snapshot) []FormField {
		return []FormField{
			textField("project", "tui.field.project", first(s.Projects), false, s.Projects),
			textField("workItemIds", "tui.field.work-item", "", false, workItemSuggestions(s)),
			toggleField("noSync", "tui.field.no-sync", true),
			toggleField("execute", "tui.field.execute", false),
		}
	}},
	{ID: "task-add-work-item", Label: "tui.form.task-add-work-item", Description: "tui.form.task-add-work-item.desc", ActionID: "task.work-item.add", Fields: func(s Snapshot) []FormField {
		return []FormField{
			textField("workItemIds", "tui.field.work-items", "", false, workItemSuggestions(s)),
			textField("workspace", "tui.field.workspace", firstWorkspace(s), false, workspaceSuggestions(s)),
			toggleField("continue", "tui.field.continue", false),
			textField("project", "tui.field.project", first(s.Projects), false, s.Projects),
			textField("workspaceWorkItemIds", "tui.field.workspace-work-item", "", false, workItemSuggestions(s)),
			textField("type", "tui.field.type", "", false, []string{"feature", "bugfix", "hotfix", "chore"}),
			textField("title", "tui.field.title", "", false, nil),
			textField("state", "tui.field.state", "", false, s.States),
			toggleField("skipAdo", "tui.field.skip-ado", false),
			toggleField("execute", "tui.field.execute", false),
		}
	}},
	{ID: "task-remove-work-item", Label: "tui.form.task-remove-work-item", Description: "tui.form.task-remove-work-item.desc", ActionID: "task.work-item.remove", Fields: func(s Snapshot) []FormField {
		return []FormField{
			textField("workItemIds", "tui.field.work-items", "", false, workItemSuggestions(s)),
			textField("workspace", "tui.field.workspace", firstWorkspace(s), false, workspaceSuggestions(s)),
			toggleField("continue", "tui.field.continue", false),
			textField("project", "tui.field.project", first(s.Projects), false, s.Projects),
			textField("workspaceWorkItemIds", "tui.field.workspace-work-item", "", false, workItemSuggestions(s)),
			toggleField("execute", "tui.field.execute", false),
		}
	}},
	{ID: "task-add-repo", Label: "tui.form.task-add-repo", Description: "tui.form.task-add-repo.desc", ActionID: "task.repo.add", Fields: func(s Snapshot) []FormField {
		return []FormField{
			textField("repository", "tui.field.repository", "", true, s.Repositories),
			textField("workspace", "tui.field.workspace", firstWorkspace(s), false, workspaceSuggestions(s)),
			toggleField("execute", "tui.field.execute", false),
		}
	}},
	{ID: "task-rename", Label: "tui.form.task-rename", Description: "tui.form.task-rename.desc", ActionID: "task.rename", Fields: func(s Snapshot) []FormField {
		return []FormField{
			textField("slug", "tui.field.slug", "", true, nil),
			textField("workspace", "tui.field.workspace", firstWorkspace(s), false, workspaceSuggestions(s)),
			toggleField("continue", "tui.field.continue", false),
			textField("project", "tui.field.project", first(s.Projects), false, s.Projects),
			textField("workItemIds", "tui.field.work-item", "", false, workItemSuggestions(s)),
			toggleField("execute", "tui.field.execute", false),
		}
	}},
	{ID: "ado-assigned", Label: "tui.form.ado-assigned", Description: "tui.form.ado-assigned.desc", ActionID: "ado.assigned", Fields: func(s Snapshot) []FormField {
		return []FormField{
			textField("project", "tui.field.project", first(s.Projects), false, s.Projects),
			textField("top", "tui.field.top", "20", false, nil),
			toggleField("all", "tui.field.include-final", false),
			toggleField("groupByParent", "tui.field.group-parent", false),
		}
	}},
	{ID: "ado-set-state", Label: "tui.form.ado-set-state", Description: "tui.form.ado-set-state.desc", ActionID: "ado.set-state", Fields: func(s Snapshot) []FormField {
		return []FormField{
			textField("workItemIds", "tui.field.work-item-ids", firstWorkItem(s), true, workItemSuggestions(s)),
			textField("project", "tui.field.project", first(s.Projects), false, s.Projects),
			textField("state", "tui.field.destination-state", first(s.States), true, s.States),
			textField("history", "tui.field.ado-note", "tui", false, nil),
		}
	}},
	{ID: "db-schema", Label: "tui.form.db-schema", Description: "tui.form.db-schema.desc", ActionID: "db.schema", Fields: func(s Snapshot) []FormField {
		project, database := firstDB(s)
		return []FormField{textField("project", "tui.field.project", project, false, s.Projects), textField("database", "tui.field.database", database, false, databaseSuggestions(s))}
	}},
	{ID: "db-describe", Label: "tui.form.db-describe", Description: "tui.form.db-describe.desc", ActionID: "db.describe", Fields: func(s Snapshot) []FormField {
		project, database := firstDB(s)
		return []FormField{textField("table", "tui.field.table", "", true, nil), textField("project", "tui.field.project", project, false, s.Projects), textField("database", "tui.field.database", database, false, databaseSuggestions(s))}
	}},
	{ID: "db-query", Label: "tui.form.db-query", Description: "tui.form.db-query.desc", ActionID: "db.query", Fields: func(s Snapshot) []FormField {
		project, database := firstDB(s)
		return []FormField{textField("project", "tui.field.project", project, false, s.Projects), textField("database", "tui.field.database", database, false, databaseSuggestions(s)), textField("sql", "tui.field.sql", "select 1", true, nil), textField("maxRows", "tui.field.max-rows", "100", false, nil)}
	}},
	{ID: "agent-open", Label: "tui.form.agent-open", Description: "tui.form.agent-open.desc", ActionID: "task.open", Fields: func(s Snapshot) []FormField {
		return []FormField{
			textField("workspace", "tui.field.workspace", firstWorkspace(s), false, workspaceSuggestions(s)),
			toggleField("continue", "tui.field.continue", false),
			textField("project", "tui.field.project", first(s.Projects), false, s.Projects),
			textField("workItemIds", "tui.field.work-item", "", false, workItemSuggestions(s)),
			textField("repository", "tui.field.repository", first(s.Repositories), false, s.Repositories),
			textField("agent", "tui.field.agent", "", false, []string{"opencode", "cursor", "claude", "codex", "codex-cli", "copilot"}),
		}
	}},
	{ID: "secret", Label: "tui.form.secret", Description: "tui.form.secret.desc", ActionID: "secret.get", Fields: func(s Snapshot) []FormField {
		return []FormField{textField("key", "tui.field.key", "", true, s.SecretKeys), toggleField("setFromEnv", "tui.field.set-env", false), textField("fromEnv", "tui.field.from-env", "", false, s.Environment), toggleField("delete", "tui.field.delete", false)}
	}},
	{ID: "config-root", Label: "tui.form.config-root", Description: "tui.form.config-root.desc", ActionID: "config.set-root", Fields: func(s Snapshot) []FormField {
		return []FormField{textField("root", "tui.field.root", s.Root, true, nil)}
	}},
}

func (f *FormState) template() FormTemplate {
	index := f.TemplateIndex
	if index < 0 || index >= len(formTemplates) {
		index = 0
	}
	return formTemplates[index]
}

func (f *FormState) begin(snapshot Snapshot) {
	f.Fields = f.template().Fields(snapshot)
	f.SelectedField = 0
	f.Mode = EditFields
}

func (f *FormState) move(delta int) {
	limit := len(formTemplates)
	current := &f.TemplateIndex
	if f.Mode == EditFields {
		limit = len(f.Fields)
		current = &f.SelectedField
	}
	if limit == 0 {
		*current = 0
		return
	}
	*current += delta
	if *current < 0 {
		*current = 0
	}
	if *current >= limit {
		*current = limit - 1
	}
}

func (f *FormState) input(text string) {
	if f.Mode != EditFields || f.SelectedField >= len(f.Fields) || f.Fields[f.SelectedField].Kind != TextField {
		return
	}
	f.Fields[f.SelectedField].Value += text
}

func (f *FormState) backspace() {
	if f.Mode != EditFields || f.SelectedField >= len(f.Fields) || f.Fields[f.SelectedField].Kind != TextField {
		return
	}
	value := []rune(f.Fields[f.SelectedField].Value)
	if len(value) != 0 {
		f.Fields[f.SelectedField].Value = string(value[:len(value)-1])
	}
}

func (f *FormState) toggle() {
	if f.Mode != EditFields || f.SelectedField >= len(f.Fields) || f.Fields[f.SelectedField].Kind != ToggleField {
		return
	}
	f.Fields[f.SelectedField].Value = strconv.FormatBool(!f.Fields[f.SelectedField].enabled())
}

func (f *FormState) suggest() (string, bool) {
	if f.Mode != EditFields || f.SelectedField >= len(f.Fields) {
		return "", false
	}
	field := &f.Fields[f.SelectedField]
	if field.Kind != TextField || len(field.Suggestions) == 0 {
		return "", false
	}
	next := 0
	for i := range field.Suggestions {
		if field.Suggestions[i] == strings.TrimSpace(field.Value) {
			next = (i + 1) % len(field.Suggestions)
			break
		}
	}
	field.Value = field.Suggestions[next]
	return field.Value, true
}

func (f FormState) validation(localizer l10n.Localizer) []string {
	var issues []string
	for _, field := range f.Fields {
		if field.Required && strings.TrimSpace(field.Value) == "" {
			issues = append(issues, localizer.Render(l10n.M("tui.validation.required", l10n.A("field", localizer.Text(field.Label)))))
		}
	}
	for _, field := range f.Fields {
		if (field.ID == "top" || field.ID == "maxRows") && strings.TrimSpace(field.Value) != "" {
			if _, err := strconv.ParseUint(strings.TrimSpace(field.Value), 10, 64); err != nil {
				issues = append(issues, localizer.Render(l10n.M("tui.validation.integer", l10n.A("field", localizer.Text(field.Label)))))
			}
		}
		if field.ID == "agent" && strings.TrimSpace(field.Value) != "" && !contains([]string{"opencode", "cursor", "claude", "codex", "codex-cli", "copilot"}, strings.TrimSpace(field.Value)) {
			issues = append(issues, localizer.Text("tui.validation.agent"))
		}
	}
	if f.template().ID == "secret" && fieldBool(f.Fields, "setFromEnv") && fieldValue(f.Fields, "fromEnv") == "" {
		issues = append(issues, localizer.Render(l10n.M("tui.validation.required", l10n.A("field", localizer.Text("tui.field.from-env")))))
	}
	return issues
}

func (f FormState) action(localizer l10n.Localizer) (Action, bool) {
	if f.Mode != EditFields || len(f.validation(localizer)) != 0 {
		return Action{}, false
	}
	template := f.template()
	actionID := template.ActionID
	risk := Safe
	if template.ID == "agent-open" {
		risk = External
	} else if template.ID == "ado-set-state" || template.ID == "config-root" || (template.ID == "secret" && fieldBool(f.Fields, "delete")) || fieldBool(f.Fields, "execute") {
		risk = Destructive
	} else if strings.HasPrefix(template.ID, "task-") {
		risk = Preview
	}
	parameters := make([]Parameter, 0, len(f.Fields))
	for _, field := range f.Fields {
		var value any = strings.TrimSpace(field.Value)
		if field.Kind == ToggleField {
			value = field.enabled()
		} else if field.ID == "workItemIds" || field.ID == "workspaceWorkItemIds" || field.ID == "repositories" {
			value = splitValues(field.Value)
		} else if (field.ID == "top" || field.ID == "maxRows") && strings.TrimSpace(field.Value) != "" {
			parsed, _ := strconv.Atoi(strings.TrimSpace(field.Value))
			value = parsed
		}
		parameters = append(parameters, Parameter{Name: field.ID, Value: value})
	}
	return Action{
		ID:    action.ID(actionID),
		Label: localizer.Text(template.Label), Description: localizer.Text(template.Description), Risk: risk,
		Active:              true,
		Request:             FormRequest{Action: action.ID(actionID), Parameters: parameters},
		RefreshAfterSuccess: risk == Destructive, OpenResult: risk != External,
		BlocksUntilDone: (template.ID == "task-start" || template.ID == "task-start-pr") && fieldBool(f.Fields, "execute"),
	}, true
}

func fieldValue(fields []FormField, id string) string {
	for _, field := range fields {
		if field.ID == id {
			return strings.TrimSpace(field.Value)
		}
	}
	return ""
}

func fieldBool(fields []FormField, id string) bool {
	for _, field := range fields {
		if field.ID == id {
			return field.enabled()
		}
	}
	return false
}

func splitValues(value string) []string {
	parts := strings.Split(value, ",")
	result := make([]string, 0, len(parts))
	for _, part := range parts {
		if part = strings.TrimSpace(part); part != "" {
			result = append(result, part)
		}
	}
	return result
}

func stableStrings(values []string) []string {
	seen := make(map[string]struct{}, len(values))
	result := make([]string, 0, len(values))
	for _, value := range values {
		if value = strings.TrimSpace(value); value == "" {
			continue
		}
		if _, ok := seen[value]; ok {
			continue
		}
		seen[value] = struct{}{}
		result = append(result, value)
	}
	return result
}

func contains(values []string, value string) bool {
	for _, candidate := range values {
		if candidate == value {
			return true
		}
	}
	return false
}

func workspaceSuggestions(s Snapshot) []string {
	values := make([]string, 0, len(s.Workspaces))
	for _, item := range s.Workspaces {
		values = append(values, item.Path)
	}
	return stableStrings(values)
}

func workItemSuggestions(s Snapshot) []string {
	var values []string
	for _, item := range s.Workspaces {
		values = append(values, item.WorkItems...)
	}
	for _, project := range s.ADOProjects {
		for _, item := range project.Items {
			values = append(values, item.ID)
		}
	}
	return stableStrings(values)
}

func pullRequestSuggestions(s Snapshot) []string {
	values := make([]string, 0, len(s.PullRequests))
	for _, item := range s.PullRequests {
		values = append(values, item.ID)
	}
	return stableStrings(values)
}

func databaseSuggestions(s Snapshot) []string {
	values := make([]string, 0, len(s.Databases))
	for _, item := range s.Databases {
		values = append(values, item.Key)
	}
	return stableStrings(values)
}
