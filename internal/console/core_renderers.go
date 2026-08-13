package console

import (
	"path/filepath"
	"strconv"
	"strings"

	"github.com/sachahjkl/dw/internal/config"
	"github.com/sachahjkl/dw/internal/contract"
	"github.com/sachahjkl/dw/internal/data"
	"github.com/sachahjkl/dw/internal/dataapp"
	"github.com/sachahjkl/dw/internal/doctor"
	"github.com/sachahjkl/dw/internal/secret"
	"github.com/sachahjkl/dw/internal/workapp"
)

func RegisterCoreRenderers(results *Registry) error {
	registrations := []func() error{
		func() error {
			return RegisterPageResult(results, config.ActionInit, initPage)
		},
		func() error {
			return RegisterPageResult(results, config.ActionRefresh, func(r config.RefreshReport) Page {
				return actionPage(r.ActionID(), Field{Label: "result.root", Value: r.Root, Style: ValuePath}, Field{Label: "result.profile", Value: r.Profile})
			})
		},
		func() error { return RegisterPageResult(results, config.ActionShow, configShowPage) },
		func() error { return RegisterPageResult(results, config.ActionDoctor, configDoctorPage) },
		func() error {
			return RegisterPageResult(results, config.ActionRootSet, func(r config.RootSetReport) Page {
				return actionPage(r.ActionID(), Field{Label: "result.root", Value: r.Root, Style: ValuePath})
			})
		},
		func() error {
			return RegisterPageResult(results, config.ActionColorSet, func(r config.ColorSetReport) Page {
				return actionPage(r.ActionID(), Field{Label: "result.mode", Value: string(r.Mode)})
			})
		},
		func() error {
			return RegisterPageResult(results, config.ActionAgentConfig, func(r config.AgentConfigReport) Page {
				return actionPage(r.ActionID(), Field{Label: "result.root", Value: r.Root, Style: ValuePath}, Field{Label: "result.agent", Value: string(r.Agent)})
			})
		},
		func() error {
			return RegisterPageResult(results, config.ActionAgentShow, func(r config.AgentShowReport) Page {
				return actionPage(r.ActionID(), Field{Label: "result.root", Value: r.Root, Style: ValuePath}, Field{Label: "result.agent", Value: string(r.Agent)})
			})
		},
		func() error {
			return RegisterPageResult(results, config.ActionAgentDefaultSet, func(r config.AgentDefaultSetReport) Page {
				return actionPage(r.ActionID(), Field{Label: "result.root", Value: r.Root, Style: ValuePath}, Field{Label: "result.agent", Value: string(r.Agent), Style: ValueSuccess})
			})
		},
		func() error { return RegisterPageResult(results, doctor.ActionDoctor, doctorPage) },
		func() error { return RegisterPageResult(results, doctor.ActionAgentDoctor, agentDoctorPage) },
		func() error { return RegisterPageResult(results, secret.ActionList, secretListPage) },
		func() error {
			return RegisterPageResult(results, secret.ActionSet, func(r secret.SetReport) Page {
				return actionPage(r.ActionID(), Field{Label: "result.key", Value: string(r.Key)}, Field{Label: "result.storage", Value: string(r.Storage)}, boolStatus("result.masked", r.ValueMasked))
			})
		},
		func() error {
			return RegisterPageResult(results, secret.ActionGet, func(r secret.GetReport) Page {
				return actionPage(r.ActionID(), Field{Label: "result.key", Value: string(r.Key)}, boolStatus("result.exists", r.Exists))
			})
		},
		func() error {
			return RegisterPageResult(results, secret.ActionDelete, func(r secret.DeleteReport) Page {
				return actionPage(r.ActionID(), Field{Label: "result.key", Value: string(r.Key)}, boolStatus("result.deleted", r.DeletedIfPresent))
			})
		},
		func() error {
			return RegisterPageResult(results, dataapp.ActionDataSourceList, dataSourceListPage)
		},
		func() error {
			return RegisterPageResult(results, dataapp.ActionDataSourceCollect, func(r dataapp.DataSourceCollectResult) Page {
				return actionPage(r.ActionID(), Field{Label: "result.root", Value: r.Root, Style: ValuePath}, countField("result.workspaces", r.ScannedWorkspaces), countField("result.files", r.ScannedFiles), countField("result.items", len(r.Findings)), countField("result.saved", r.SavedCount))
			})
		},
		func() error { return RegisterPageResult(results, dataapp.ActionDataGuard, guardPage) },
		func() error {
			if err := RegisterResult(results, dataapp.ActionDataCatalog, func(c RenderContext, r dataapp.CatalogResult) (Output, error) {
				return renderDataQuery(r.NativeQueryReport, c, "data.catalog.title"), nil
			}); err != nil {
				return err
			}
			return results.RegisterPage(dataapp.ActionDataCatalog, PageProjectorFor(func(r dataapp.CatalogResult) Page { return nativeQueryPage(r.NativeQueryReport, "data.catalog.title") }))
		},
		func() error {
			if err := RegisterResult(results, dataapp.ActionDataQuery, func(c RenderContext, r dataapp.DataQueryResult) (Output, error) {
				return renderDataQuery(r.NativeQueryReport, c, "data.query.title"), nil
			}); err != nil {
				return err
			}
			return results.RegisterPage(dataapp.ActionDataQuery, PageProjectorFor(func(r dataapp.DataQueryResult) Page { return nativeQueryPage(r.NativeQueryReport, "data.query.title") }))
		},
		func() error {
			if err := RegisterResult(results, dataapp.ActionDataRead, func(c RenderContext, r dataapp.DataReadResult) (Output, error) {
				return renderDataQuery(r.NativeQueryReport, c, "data.read.title"), nil
			}); err != nil {
				return err
			}
			return results.RegisterPage(dataapp.ActionDataRead, PageProjectorFor(func(r dataapp.DataReadResult) Page { return nativeQueryPage(r.NativeQueryReport, "data.read.title") }))
		},
		func() error {
			if err := RegisterResult(results, dataapp.ActionDataDescribe, func(c RenderContext, r dataapp.DescribeResult) (Output, error) {
				if r.Result == nil {
					return Output{}, nil
				}
				return renderDataQuery(*r.Result, c, "data.describe.title"), nil
			}); err != nil {
				return err
			}
			return results.RegisterPage(dataapp.ActionDataDescribe, PageProjectorFor(func(r dataapp.DescribeResult) Page {
				if r.Result == nil {
					return Page{Title: "data.describe.title"}
				}
				return nativeQueryPage(*r.Result, "data.describe.title")
			}))
		},
		func() error { return RegisterPageResult(results, workapp.ActionProviderAuthLogin, authLoginPage) },
		func() error { return RegisterPageResult(results, workapp.ActionProviderAuthStatus, authStatusPage) },
		func() error {
			return RegisterPageResult(results, workapp.ActionProviderAuthLogout, func(r workapp.AuthLogoutReport) Page {
				return actionPage(r.ActionID(), boolStatus("result.removed", r.RemovedLocalSession))
			})
		},
		func() error {
			return RegisterPageResult(results, workapp.ActionWorkItemList, assignedItemsPage)
		},
		func() error {
			return RegisterPageResult(results, workapp.ActionWorkPullRequestList, pullRequestsPage)
		},
		func() error {
			return RegisterChangelogRenderer(results, workapp.ActionWorkChangelog, projectChangelogComplete)
		},
		func() error {
			return RegisterPageResult(results, workapp.ActionWorkContextShow, contextPage)
		},
		func() error {
			return RegisterPageResult(results, workapp.ActionWorkContextAI, func(r workapp.AIContextResult) Page {
				return actionPage(r.ActionID(), Field{Label: "result.project", Value: r.Report.Project}, countField("result.items", len(r.Report.Items)))
			})
		},
		func() error {
			return RegisterPageResult(results, workapp.ActionWorkItemShow, itemShowPage)
		},
		func() error {
			return RegisterPageResult(results, workapp.ActionWorkItemStatePlan, func(r workapp.StatePlanReport) Page {
				return actionPage(r.ActionID(), Field{Label: "result.project", Value: r.Project}, Field{Label: "result.state", Value: r.State}, countField("result.items", len(r.IDs)))
			})
		},
		func() error {
			return RegisterPageResult(results, workapp.ActionWorkItemStateExecute, func(r workapp.StateExecutionReport) Page {
				return actionPage(r.ActionID(), Field{Label: "result.project", Value: r.Plan.Project}, Field{Label: "result.state", Value: r.Plan.State}, countField("result.updated", len(r.Updated)))
			})
		},
		func() error {
			return RegisterPageResult(results, workapp.ActionWorkItemStateSet, func(r workapp.StateSetResult) Page {
				return actionPage(r.ActionID(), Field{Label: "result.project", Value: r.Plan.Project}, Field{Label: "result.state", Value: r.Plan.State}, countField("result.updated", len(r.Execution.Updated)))
			})
		},
		func() error {
			return RegisterPageResult(results, workapp.ActionWorkItemDoingPlan, func(r workapp.DoingPlanReport) Page {
				return actionPage(r.ActionID(), Field{Label: "result.project", Value: r.Project}, countField("result.updates", len(r.Updates)))
			})
		},
		func() error {
			return RegisterPageResult(results, workapp.ActionWorkItemDoingExecute, func(r workapp.DoingExecutionReport) Page {
				return actionPage(r.ActionID(), Field{Label: "result.project", Value: r.Plan.Project}, countField("result.updated", len(r.Updated)))
			})
		},
		func() error {
			return results.Register(ResultWorkItemDoing, func(context RenderContext, payload any) (Output, error) {
				var page Page
				switch result := payload.(type) {
				case workapp.DoingActionResult:
					page = actionPage(ResultWorkItemDoing, Field{Label: "result.project", Value: result.Plan.Project}, countField("result.updated", doingUpdatedCount(result)))
				case workapp.DoingPlanReport:
					page = actionPage(ResultWorkItemDoing, Field{Label: "result.project", Value: result.Project}, countField("result.updates", len(result.Updates)))
				case workapp.DoingExecutionReport:
					page = actionPage(ResultWorkItemDoing, Field{Label: "result.project", Value: result.Plan.Project}, countField("result.updated", len(result.Updated)))
				default:
					return Output{}, PayloadTypeError{Kind: string(ResultWorkItemDoing)}
				}
				return TextOutput(FormatHuman, RenderPage(page, context.Localizer, context.Theme)), nil
			})
		},
		func() error {
			return RegisterPageResult(results, workapp.ActionWorkspaceStart, func(r workapp.StartResult) Page {
				return actionPage(r.ActionID(), Field{Label: "result.root", Value: r.Plan.Root, Style: ValuePath}, countField("result.items", len(r.Plan.WorkItems)), boolStatus("result.executed", r.Execution != nil))
			})
		},
		func() error {
			return RegisterPageResult(results, workapp.ActionWorkspacePullRequestStart, func(r workapp.StartPullRequestResult) Page {
				return actionPage(r.ActionID(), Field{Label: "result.pull-request", Value: strconv.FormatInt(r.Plan.PullRequestID, 10)}, countField("result.repositories", len(r.Plan.Repositories)), boolStatus("result.executed", r.Execution != nil))
			})
		},
		func() error {
			return RegisterPageResult(results, workapp.ActionWorkspaceOpen, func(r workapp.OpenReport) Page {
				return actionPage(r.ActionID(), Field{Label: "result.workspace", Value: r.Workspace, Style: ValuePath})
			})
		},
		func() error {
			return RegisterPageResult(results, workapp.ActionWorkspaceSync, func(r workapp.SyncReport) Page {
				return actionPage(r.ActionID(), Field{Label: "result.workspace", Value: r.Workspace, Style: ValuePath}, countField("result.items", len(r.Snapshots)))
			})
		},
		func() error {
			return RegisterPageResult(results, workapp.ActionWorkspaceContextRefresh, func(r workapp.ContextRefreshReport) Page {
				return actionPage(r.ActionID(), Field{Label: "result.workspace", Value: r.Workspace, Style: ValuePath}, Field{Label: "result.path", Value: r.ContextFile, Style: ValuePath}, countField("result.items", r.ItemCount))
			})
		},
		func() error {
			return RegisterPageResult(results, workapp.ActionWorkItemChildCreate, func(r workapp.ChildReport) Page {
				return actionPage(r.ActionID(), Field{Label: "result.workspace", Value: r.Workspace, Style: ValuePath}, Field{Label: "result.repository", Value: r.Repository}, Field{Label: "result.requested-title", Value: r.RequestedTitle}, Field{Label: "result.created-title", Value: r.Created.Title}, Field{Label: "result.item", Value: r.Created.ID, Style: ValueSuccess})
			})
		},
		func() error {
			return RegisterPageResult(results, workapp.ActionWorkspacePrune, func(r workapp.PruneReport) Page {
				return actionPage(r.ActionID(), boolStatus("result.executed", r.Execution != nil))
			})
		},
		func() error {
			return RegisterPageResult(results, workapp.ActionWorkspaceFinish, func(r workapp.FinishReport) Page {
				return actionPage(r.ActionID(), boolStatus("result.executed", r.Execution != nil))
			})
		},
	}
	for _, register := range registrations {
		if err := register(); err != nil {
			return err
		}
	}
	return nil
}

func actionPage(id ResultKind, fields ...Field) Page {
	return ActionPage(string(id), fields...)
}

func countField(label MessageID, count int) Field {
	return Field{Label: label, Value: strconv.Itoa(count)}
}

func boolStatus(label MessageID, value bool) Field {
	style := ValueWarning
	text := "No"
	if value {
		style = ValueSuccess
		text = "Yes"
	}
	return Field{Label: label, Value: text, Style: style}
}

func statusField(label MessageID, passed bool, success, failure string) Field {
	if passed {
		return Field{Label: label, Value: success, Style: ValueSuccess}
	}
	return Field{Label: label, Value: failure, Style: ValueFailure}
}

func initPage(r config.InitReport) Page {
	status := "Initialized"
	pathLabel := MessageID("result.files-created")
	if r.DryRun {
		status = "Preview"
		pathLabel = "result.files-planned"
	}
	p := actionPage(r.ActionID(),
		Field{Label: "result.root", Value: r.Root, Style: ValuePath},
		Field{Label: "result.profile", Value: r.Profile},
		Field{Label: "result.status", Value: status, Style: ValueSuccess},
		countField(pathLabel, len(r.PlannedPaths)),
		boolStatus("result.saved-root", !r.DryRun && !r.NoSave),
	)
	next := contract.ActionLink{Relation: "doctor", Arguments: []contract.ActionArgument{{Name: "root", Value: r.Root}}}
	if r.DryRun {
		next.Relation = "initialize"
	}
	p.Actions = []contract.ActionLink{next}
	return p
}

func configShowPage(r config.ConfigShow) Page {
	ready := r.WorkflowExists && r.ProjectsExists && r.DatabasesExists
	p := actionPage(r.ActionID(),
		Field{Label: "result.root", Value: r.Root, Style: ValuePath},
		statusField("result.status", ready, "Ready", "Not initialized"),
		Field{Label: "result.mode", Value: string(r.Color)},
		Field{Label: "result.settings", Value: r.SettingsPath, Style: ValuePath},
		Field{Label: "result.workflow", Value: r.WorkflowPath, Style: ValuePath},
		Field{Label: "result.projects", Value: r.ProjectsPath, Style: ValuePath},
		Field{Label: "result.databases", Value: r.DatabasesPath, Style: ValuePath},
	)
	if !ready {
		p.Actions = []contract.ActionLink{{Relation: "initialize", Arguments: []contract.ActionArgument{{Name: "root", Value: r.Root}}}}
	}
	return p
}

func configDoctorPage(r config.ConfigDoctorReport) Page {
	rows := make([][]string, len(r.Checks))
	for i, check := range r.Checks {
		detail := ""
		if check.Message != nil {
			detail = *check.Message
		}
		rows[i] = []string{relativeToRoot(r.Root, check.Path), passFail(check.Passed), detail}
	}
	p := actionPage(r.ActionID(),
		Field{Label: "result.root", Value: r.Root, Style: ValuePath},
		statusField("result.status", r.Passed, "Passed", "Failed"),
	)
	p.Sections = []Section{{Table: &Table{Columns: []MessageID{"result.path", "result.status", "result.detail"}, Rows: rows}}}
	if !r.Passed {
		p.Actions = []contract.ActionLink{{Relation: "initialize", Arguments: []contract.ActionArgument{{Name: "root", Value: r.Root}}}}
	}
	return p
}

func relativeToRoot(root, path string) string {
	relative, err := filepath.Rel(root, path)
	if err == nil && relative != "." && !strings.HasPrefix(relative, "..") {
		return relative
	}
	return path
}

func doctorPage(r doctor.Report) Page {
	rows := make([][]string, len(r.Checks))
	for i, check := range r.Checks {
		rows[i] = []string{HumanizeIdentifier(string(check.Kind)), passFail(check.Passed), doctorRemediation(check)}
	}
	p := actionPage(r.ActionID(),
		Field{Label: "result.root", Value: r.Root, Style: ValuePath},
		statusField("result.status", r.Passed(), "Passed", "Failed"),
		countField("result.passed", r.PassedCount()),
		countField("result.failed", r.FailedCount()),
	)
	p.Sections = []Section{{Table: &Table{Columns: []MessageID{"result.check", "result.status", "result.remediation"}, Rows: rows}}}
	if !r.Passed() {
		p.Actions = []contract.ActionLink{{Relation: "doctor-fix", Arguments: []contract.ActionArgument{{Name: "root", Value: r.Root}}}}
	}
	return p
}

func doctorRemediation(check doctor.Check) string {
	if check.Passed {
		return "—"
	}
	switch check.Remediation.Kind {
	case doctor.RemediationInitRoot:
		return "dw init --root " + strconv.Quote(check.Remediation.Root)
	case doctor.RemediationRunInit:
		return "dw init"
	case doctor.RemediationConfigureDefaultAgent:
		return "dw agent default set " + string(check.Remediation.Agent)
	case doctor.RemediationInstallGit:
		return "Install Git"
	case doctor.RemediationInstallNodePackageManager:
		return "Install pnpm or npm"
	case doctor.RemediationInstallOpenCode:
		return "Install OpenCode"
	default:
		return "—"
	}
}

func passFail(passed bool) string {
	if passed {
		return "Passed"
	}
	return "Failed"
}

func agentDoctorPage(r doctor.AgentReport) Page {
	rows := make([][]string, len(r.Checks))
	for i, check := range r.Checks {
		status := "Missing"
		if check.Available {
			status = "Available"
		}
		rows[i] = []string{string(check.Agent), check.Command, status}
	}
	p := actionPage(r.ActionID(), statusField("result.status", r.Passed(), "Passed", "Failed"))
	p.Sections = []Section{{Table: &Table{Columns: []MessageID{"result.agent", "result.command", "result.status"}, Rows: rows}}}
	return p
}

func secretListPage(r secret.ListReport) Page {
	rows := make([][]string, len(r.Items))
	for i, item := range r.Items {
		status := "Missing"
		if item.Exists {
			status = "Stored"
		}
		rows[i] = []string{string(item.Key), status, strconv.Itoa(len(item.References))}
	}
	p := actionPage(r.ActionID(), Field{Label: "result.root", Value: r.Root, Style: ValuePath}, countField("result.warnings", len(r.Warnings)))
	p.Sections = []Section{{Table: &Table{Columns: []MessageID{"result.key", "result.status", "result.references"}, Rows: rows}}}
	return p
}

func dataSourceListPage(r dataapp.DataSourceListResult) Page {
	rows := make([][]string, len(r.Entries))
	warnings := append([]string(nil), r.Warnings...)
	for i, entry := range r.Entries {
		scope := "Global"
		if entry.Project != nil {
			scope = *entry.Project
		}
		limit := strconv.Itoa(entry.MaxRows)
		if entry.MaxRows == 0 {
			limit = "Unlimited"
		}
		rows[i] = []string{scope, entry.Database, entry.Provider, connectionSource(entry.Source), limit}
		for _, warning := range entry.Warnings {
			warnings = append(warnings, entry.Database+": "+warning)
		}
	}
	p := actionPage(r.ActionID(),
		Field{Label: "result.root", Value: r.Root, Style: ValuePath},
		countField("result.items", len(r.Entries)),
		countField("result.warnings", len(warnings)),
	)
	if len(rows) == 0 {
		p.Summary = append(p.Summary, Field{Label: "result.status", Value: "No data sources configured", Style: ValueWarning})
		p.Actions = []contract.ActionLink{{Relation: "initialize", Arguments: []contract.ActionArgument{{Name: "root", Value: r.Root}}}}
	} else {
		p.Sections = append(p.Sections, Section{Table: &Table{Columns: []MessageID{"result.scope", "result.source", "result.provider", "result.connection", "result.limit"}, Rows: rows}})
	}
	if len(warnings) != 0 {
		p.Sections = append(p.Sections, Section{Title: "result.warnings", Items: warnings})
	}
	return p
}

func connectionSource(source dataapp.ConnectionSource) string {
	switch source.Kind {
	case dataapp.SourceCredential:
		return "Keyring: " + source.Key
	case dataapp.SourceEnvironment:
		return "Environment: " + source.Variable
	case dataapp.SourceInline:
		return "Inline (masked)"
	case dataapp.SourceMultiple:
		return "Multiple"
	default:
		return "Missing"
	}
}

func guardPage(r dataapp.GuardResult) Page {
	p := actionPage(r.ActionID(), boolStatus("result.allowed", r.IsAllowed))
	if r.Reason != nil {
		p.Summary = append(p.Summary, Field{Label: "result.reason", Value: *r.Reason, Style: ValueFailure})
	}
	return p
}

func renderDataQuery(r dataapp.NativeQueryReport, c RenderContext, title MessageID) Output {
	t := data.Table{Columns: make([]data.Column, len(r.Columns)), Rows: make([][]data.Value, len(r.Rows)), Truncated: r.Truncated}
	for i, name := range r.Columns {
		t.Columns[i] = data.Column{Name: name}
	}
	for i, row := range r.Rows {
		t.Rows[i] = make([]data.Value, len(row))
		for j, cell := range row {
			if cell.Valid {
				t.Rows[i][j] = data.StringValue(cell.Value)
			} else {
				t.Rows[i][j] = data.NullValue()
			}
		}
	}
	return RenderDataTable(t, c.Policy, c.Localizer, c.Theme, title)
}

func nativeQueryPage(r dataapp.NativeQueryReport, title MessageID) Page {
	rows := make([][]string, len(r.Rows))
	for index, row := range r.Rows {
		rows[index] = make([]string, len(row))
		for cellIndex, cell := range row {
			if cell.Valid {
				rows[index][cellIndex] = cell.Value
			} else {
				rows[index][cellIndex] = "NULL"
			}
		}
	}
	page := Page{Title: title, Summary: []Field{countField("result.items", len(rows))}}
	if len(r.Columns) != 0 {
		page.Sections = []Section{{Table: &Table{ColumnNames: append([]string(nil), r.Columns...), Rows: rows}}}
	}
	if r.Truncated {
		page.Badge, page.Status = "data.query.truncated.badge", StatusWarning
	}
	return page
}

func assignedItemsPage(r workapp.AssignedReport) Page {
	p := actionPage(r.ActionID(), Field{Label: "result.project", Value: r.Project})
	var rows [][]string
	columns := []MessageID{"result.id", "result.type", "result.state", "result.item-title"}
	if r.GroupByParent {
		columns = append([]MessageID{"result.parent"}, columns...)
		for _, group := range r.Groups {
			for _, item := range group.Items {
				rows = append(rows, append([]string{group.Parent.ID}, itemRow(item)...))
			}
		}
	} else {
		for _, item := range r.Items {
			rows = append(rows, itemRow(item))
		}
	}
	p.Summary = append(p.Summary, countField("result.items", len(rows)))
	if len(rows) == 0 {
		p.Summary = append(p.Summary, Field{Label: "result.status", Value: "No assigned work items found", Style: ValueWarning})
		p.Actions = []contract.ActionLink{{Relation: "refresh-work"}}
		return p
	}
	p.Sections = []Section{{Table: &Table{Columns: columns, Rows: rows}}}
	return p
}

func itemShowPage(r workapp.ItemShowReport) Page {
	p := actionPage(r.ActionID(), Field{Label: "result.project", Value: r.Project}, countField("result.items", len(r.Items)))
	if len(r.Items) == 0 {
		p.Summary = append(p.Summary, Field{Label: "result.status", Value: "No work items found", Style: ValueWarning})
		return p
	}
	rows := make([][]string, len(r.Items))
	for i, item := range r.Items {
		rows[i] = itemRow(item)
	}
	p.Sections = []Section{{Table: &Table{Columns: []MessageID{"result.id", "result.type", "result.state", "result.item-title"}, Rows: rows}}}
	return p
}

func itemRow(item workapp.ItemSnapshot) []string {
	return []string{item.ID, stringValue(item.Type), stringValue(item.State), stringValue(item.Title)}
}

func pullRequestsPage(r workapp.PullRequestsReport) Page {
	p := actionPage(r.ActionID(), Field{Label: "result.project", Value: r.Project}, countField("result.items", len(r.Items)))
	if len(r.Items) == 0 {
		p.Summary = append(p.Summary, Field{Label: "result.status", Value: "No pull requests found", Style: ValueWarning})
		return p
	}
	rows := make([][]string, len(r.Items))
	for i, item := range r.Items {
		status := stringValue(item.Status)
		if item.IsDraft {
			status = strings.TrimSpace(status + " (Draft)")
		}
		branch := stringValue(item.SourceRefName)
		if target := stringValue(item.TargetRefName); target != "" {
			branch += " → " + target
		}
		rows[i] = []string{item.Repository, strconv.FormatInt(item.PullRequestID, 10), status, stringValue(item.Title), branch}
	}
	p.Sections = []Section{{Table: &Table{Columns: []MessageID{"result.repository", "result.pull-request", "result.status", "result.item-title", "result.branch"}, Rows: rows}}}
	return p
}

func contextPage(r workapp.ContextReport) Page {
	p := actionPage(r.ActionID(), Field{Label: "result.project", Value: r.Project}, countField("result.items", len(r.Items)+len(r.Expanded)))
	for _, item := range r.Items {
		section := Section{Fields: []Field{
			{Label: "result.id", Value: item.WorkItem.ID},
			{Label: "result.type", Value: stringValue(item.WorkItem.Type)},
			{Label: "result.state", Value: stringValue(item.WorkItem.State)},
			{Label: "result.item-title", Value: stringValue(item.WorkItem.Title)},
		}}
		if item.Content.Description != nil && strings.TrimSpace(*item.Content.Description) != "" {
			section.Panels = append(section.Panels, Panel{Title: "result.description-content", Body: *item.Content.Description})
		}
		if item.Content.AcceptanceCriteria != nil && strings.TrimSpace(*item.Content.AcceptanceCriteria) != "" {
			section.Panels = append(section.Panels, Panel{Title: "result.acceptance-criteria", Body: *item.Content.AcceptanceCriteria})
		}
		if len(item.Comments) != 0 {
			comments := make([]string, 0, len(item.Comments))
			for _, comment := range item.Comments {
				header := strings.TrimSpace(stringValue(comment.Author) + " " + stringValue(comment.CreatedDate))
				body := strings.TrimSpace(stringValue(comment.Text))
				if header != "" && body != "" {
					comments = append(comments, header+"\n"+body)
				} else if body != "" {
					comments = append(comments, body)
				}
			}
			if len(comments) != 0 {
				section.Panels = append(section.Panels, Panel{Title: "result.comments", Body: strings.Join(comments, "\n\n")})
			}
		}
		p.Sections = append(p.Sections, section)
	}
	for _, expanded := range r.Expanded {
		p.Sections = append(p.Sections, Section{Panels: []Panel{{Body: string(expanded)}}})
	}
	if len(p.Sections) == 0 {
		p.Summary = append(p.Summary, Field{Label: "result.status", Value: "No context found", Style: ValueWarning})
	}
	return p
}

func stringValue(value *string) string {
	if value == nil {
		return ""
	}
	return *value
}

func authLoginPage(r workapp.AuthLoginReport) Page {
	p := actionPage(r.ActionID(), Field{Label: "result.mode", Value: string(r.Mode)}, boolStatus("result.environment-pat", r.UsesEnvironmentPAT))
	if r.Source != nil {
		p.Summary = append(p.Summary, Field{Label: "result.source", Value: *r.Source})
	}
	if r.ExpiresOn != nil {
		p.Summary = append(p.Summary, Field{Label: "result.expires", Value: *r.ExpiresOn})
	}
	return p
}

func authStatusPage(r workapp.AuthStatusReport) Page {
	p := actionPage(r.ActionID(), boolStatus("result.connected", r.Connected))
	if r.Source != nil {
		p.Summary = append(p.Summary, Field{Label: "result.source", Value: *r.Source})
	}
	if r.ExpiresOn != nil {
		p.Summary = append(p.Summary, Field{Label: "result.expires", Value: *r.ExpiresOn})
	}
	return p
}
func projectChangelog(r workapp.ChangelogReport) ChangelogReport {
	out := ChangelogReport{GroupByParent: r.GroupByParent, Table: r.Table, FromGit: r.FromGit, FromPR: r.FromPR}
	switch r.Format {
	case workapp.ChangelogMarkdown:
		out.Format = ChangelogMarkdown
	case workapp.ChangelogHTML:
		out.Format = ChangelogHTML
	}
	for _, s := range r.Sections {
		section := ChangelogSection{SourceEmpty: s.SourceEmpty, ResolvedEmpty: s.ResolvedEmpty}
		if s.Repository != nil {
			section.Repository = *s.Repository
		}
		for _, w := range s.Warnings {
			section.Warnings = append(section.Warnings, ChangelogWarning{Detail: w.Detail})
		}
		for _, x := range s.Items {
			section.Items = append(section.Items, changelogItem(x.ID, x.Type, x.State, x.Title, x.URL))
		}
		for _, g := range s.Groups {
			group := ChangelogGroup{Parent: changelogItem(g.Parent.ID, g.Parent.Type, g.Parent.State, g.Parent.Title, g.Parent.URL)}
			for _, x := range g.Items {
				group.Items = append(group.Items, changelogItem(x.ID, x.Type, x.State, x.Title, x.URL))
			}
			section.Groups = append(section.Groups, group)
		}
		out.Sections = append(out.Sections, section)
	}
	return out
}
func projectChangelogComplete(r workapp.ChangelogReport) ChangelogReport {
	out := projectChangelog(r)
	out.IDsOnly = r.IDsOnly
	out.WorkItemIDs = append([]string(nil), r.WorkItemIDs...)
	return out
}

func changelogItem(id string, kind, state, title, url *string) ChangelogItem {
	item := ChangelogItem{ID: id}
	if kind != nil {
		item.Type = *kind
	}
	if state != nil {
		item.State = *state
	}
	if title != nil {
		item.Title = *title
	}
	if url != nil {
		item.URL = *url
	}
	return item
}

func doingUpdatedCount(result workapp.DoingActionResult) int {
	if result.Execution == nil {
		return 0
	}
	return len(result.Execution.Updated)
}
