package console

import (
	"strconv"

	"github.com/sachahjkl/dw/internal/config"
	"github.com/sachahjkl/dw/internal/data"
	"github.com/sachahjkl/dw/internal/dataapp"
	"github.com/sachahjkl/dw/internal/doctor"
	"github.com/sachahjkl/dw/internal/secret"
	"github.com/sachahjkl/dw/internal/workapp"
)

func RegisterCoreRenderers(results *Registry) error {
	registrations := []func() error{
		func() error {
			return RegisterPageResult(results, config.ActionInit, func(r config.InitReport) Page {
				return actionPage(r.ActionID(), Field{Label: "result.root", Value: r.Root, Style: ValuePath}, Field{Label: "result.profile", Value: r.Profile}, countField("result.paths", len(r.PlannedPaths)))
			})
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
			return RegisterPageResult(results, dataapp.ActionDataSourceList, func(r dataapp.DataSourceListResult) Page {
				return actionPage(r.ActionID(), Field{Label: "result.root", Value: r.Root, Style: ValuePath}, countField("result.items", len(r.Entries)), countField("result.warnings", len(r.Warnings)))
			})
		},
		func() error {
			return RegisterPageResult(results, dataapp.ActionDataSourceCollect, func(r dataapp.DataSourceCollectResult) Page {
				return actionPage(r.ActionID(), Field{Label: "result.root", Value: r.Root, Style: ValuePath}, countField("result.workspaces", r.ScannedWorkspaces), countField("result.files", r.ScannedFiles), countField("result.items", len(r.Findings)), countField("result.saved", r.SavedCount))
			})
		},
		func() error { return RegisterPageResult(results, dataapp.ActionDataGuard, guardPage) },
		func() error {
			return RegisterResult(results, dataapp.ActionDataCatalog, func(c RenderContext, r dataapp.CatalogResult) (Output, error) {
				return renderDataQuery(r.NativeQueryReport, c), nil
			})
		},
		func() error {
			return RegisterResult(results, dataapp.ActionDataQuery, func(c RenderContext, r dataapp.DataQueryResult) (Output, error) {
				return renderDataQuery(r.NativeQueryReport, c), nil
			})
		},
		func() error {
			return RegisterResult(results, dataapp.ActionDataRead, func(c RenderContext, r dataapp.DataReadResult) (Output, error) {
				return renderDataQuery(r.NativeQueryReport, c), nil
			})
		},
		func() error {
			return RegisterResult(results, dataapp.ActionDataDescribe, func(c RenderContext, r dataapp.DescribeResult) (Output, error) {
				if r.Result == nil {
					return Output{}, nil
				}
				return renderDataQuery(*r.Result, c), nil
			})
		},
		func() error { return RegisterPageResult(results, workapp.ActionProviderAuthLogin, authLoginPage) },
		func() error { return RegisterPageResult(results, workapp.ActionProviderAuthStatus, authStatusPage) },
		func() error {
			return RegisterPageResult(results, workapp.ActionProviderAuthLogout, func(r workapp.AuthLogoutReport) Page {
				return actionPage(r.ActionID(), boolStatus("result.removed", r.RemovedLocalSession))
			})
		},
		func() error {
			return RegisterPageResult(results, workapp.ActionWorkItemList, func(r workapp.AssignedReport) Page {
				return actionPage(r.ActionID(), Field{Label: "result.project", Value: r.Project}, countField("result.items", len(r.Items)), countField("result.groups", len(r.Groups)))
			})
		},
		func() error {
			return RegisterPageResult(results, workapp.ActionWorkPullRequestList, func(r workapp.PullRequestsReport) Page {
				return actionPage(r.ActionID(), Field{Label: "result.project", Value: r.Project}, countField("result.repositories", len(r.Repositories)), countField("result.items", len(r.Items)))
			})
		},
		func() error {
			return RegisterChangelogRenderer(results, workapp.ActionWorkChangelog, projectChangelogComplete)
		},
		func() error {
			return RegisterPageResult(results, workapp.ActionWorkContextShow, func(r workapp.ContextReport) Page {
				return actionPage(r.ActionID(), Field{Label: "result.project", Value: r.Project}, countField("result.items", len(r.Items)), countField("result.expanded", len(r.Expanded)))
			})
		},
		func() error {
			return RegisterPageResult(results, workapp.ActionWorkContextAI, func(r workapp.AIContextResult) Page {
				return actionPage(r.ActionID(), Field{Label: "result.project", Value: r.Project}, countField("result.items", len(r.Items)))
			})
		},
		func() error {
			return RegisterPageResult(results, workapp.ActionWorkItemShow, func(r workapp.ItemShowReport) Page {
				return actionPage(r.ActionID(), Field{Label: "result.project", Value: r.Project}, countField("result.items", len(r.Items)))
			})
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
				return actionPage(r.ActionID(), Field{Label: "result.project", Value: r.Plan.Project}, Field{Label: "result.state", Value: r.Plan.State}, countField("result.updated", len(r.Updated)))
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
			return RegisterPageResult(results, workapp.ActionWorkItemChildCreate, func(r workapp.ChildReport) Page {
				return actionPage(r.ActionID(), Field{Label: "result.workspace", Value: r.Workspace, Style: ValuePath}, Field{Label: "result.repository", Value: r.Repository}, Field{Label: "result.item", Value: r.Created.ID, Style: ValueSuccess})
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
	return Page{Title: "result.title", Summary: append([]Field{{Label: "result.action", Value: string(id)}}, fields...)}
}
func countField(label MessageID, count int) Field {
	return Field{Label: label, Value: strconv.Itoa(count)}
}
func boolStatus(label MessageID, value bool) Field {
	style := ValueWarning
	if value {
		style = ValueSuccess
	}
	return Field{Label: label, Value: strconv.FormatBool(value), Style: style}
}

func configShowPage(r config.ConfigShow) Page {
	return actionPage(r.ActionID(), Field{Label: "result.root", Value: r.Root, Style: ValuePath}, Field{Label: "result.mode", Value: string(r.Color)}, Field{Label: "result.settings", Value: r.SettingsPath, Style: ValuePath}, Field{Label: "result.workflow", Value: r.WorkflowPath, Style: ValuePath}, Field{Label: "result.projects", Value: r.ProjectsPath, Style: ValuePath}, Field{Label: "result.databases", Value: r.DatabasesPath, Style: ValuePath})
}
func configDoctorPage(r config.ConfigDoctorReport) Page {
	rows := make([][]string, len(r.Checks))
	for i, c := range r.Checks {
		detail := ""
		if c.Message != nil {
			detail = *c.Message
		}
		rows[i] = []string{c.Path, strconv.FormatBool(c.Passed), detail}
	}
	p := actionPage(r.ActionID(), Field{Label: "result.root", Value: r.Root, Style: ValuePath}, boolStatus("result.passed", r.Passed))
	p.Sections = []Section{{Table: &Table{Columns: []MessageID{"result.path", "result.status", "result.detail"}, Rows: rows}}}
	return p
}
func doctorPage(r doctor.Report) Page {
	rows := make([][]string, len(r.Checks))
	for i, c := range r.Checks {
		rows[i] = []string{string(c.Kind), strconv.FormatBool(c.Passed), string(c.Remediation.Kind)}
	}
	p := actionPage(r.ActionID(), Field{Label: "result.root", Value: r.Root, Style: ValuePath}, countField("result.passed", r.PassedCount()), countField("result.failed", r.FailedCount()))
	p.Sections = []Section{{Table: &Table{Columns: []MessageID{"result.check", "result.status", "result.remediation"}, Rows: rows}}}
	return p
}
func agentDoctorPage(r doctor.AgentReport) Page {
	rows := make([][]string, len(r.Checks))
	for i, c := range r.Checks {
		rows[i] = []string{string(c.Agent), c.Command, strconv.FormatBool(c.Available)}
	}
	p := actionPage(r.ActionID())
	p.Sections = []Section{{Table: &Table{Columns: []MessageID{"result.agent", "result.command", "result.status"}, Rows: rows}}}
	return p
}
func secretListPage(r secret.ListReport) Page {
	rows := make([][]string, len(r.Items))
	for i, x := range r.Items {
		rows[i] = []string{string(x.Key), strconv.FormatBool(x.Exists), strconv.Itoa(len(x.References))}
	}
	p := actionPage(r.ActionID(), Field{Label: "result.root", Value: r.Root, Style: ValuePath}, countField("result.warnings", len(r.Warnings)))
	p.Sections = []Section{{Table: &Table{Columns: []MessageID{"result.key", "result.exists", "result.references"}, Rows: rows}}}
	return p
}
func guardPage(r dataapp.GuardResult) Page {
	p := actionPage(r.ActionID(), boolStatus("result.allowed", r.IsAllowed))
	if r.Reason != nil {
		p.Summary = append(p.Summary, Field{Label: "result.reason", Value: *r.Reason, Style: ValueFailure})
	}
	return p
}
func renderDataQuery(r dataapp.NativeQueryReport, c RenderContext) Output {
	t := data.Table{Columns: make([]data.Column, len(r.Columns)), Rows: make([][]data.Value, len(r.Rows)), Truncated: r.Truncated}
	for i, n := range r.Columns {
		t.Columns[i] = data.Column{Name: n}
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
	return RenderQuery(t, c.Policy, c.Localizer, c.Theme)
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
