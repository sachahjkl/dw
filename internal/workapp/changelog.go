package workapp

import (
	"context"
	"sort"

	"github.com/sachahjkl/dw/internal/l10n"
	"github.com/sachahjkl/dw/internal/work"
)

func (s *Service) Changelog(ctx context.Context, request ChangelogRequest, sink EventSink) (ChangelogReport, error) {
	if request.Table && request.Format == ChangelogRaw {
		return ChangelogReport{}, problem(msgChangelogTableFormat, "table output is only available with markdown or html format")
	}
	if request.IDsOnly && request.Table {
		return ChangelogReport{}, problem(msgChangelogIDsTable, "IDs-only output and table output cannot be combined")
	}
	if request.Project == "" {
		return ChangelogReport{}, projectRequired("work changelog")
	}
	provider, err := s.provider(s.providerName(request.Provider, request.Root, request.Project))
	if err != nil {
		return ChangelogReport{}, err
	}
	report := ChangelogReport{Root: request.Root, Project: request.Project, FromPR: request.Source == ChangelogPullRequests, FromGit: request.Source == ChangelogGitRange, GroupByParent: request.GroupByParent, Format: request.Format, Table: request.Table, IDsOnly: request.IDsOnly, WorkItemIDs: []string{}, Sections: []ChangelogSection{}, Events: []Event{}}
	if err := collectEvent(ctx, &report.Events, sink, Event{Kind: "authenticating", Project: stringPtr(request.Project)}); err != nil {
		return ChangelogReport{}, err
	}
	switch request.Source {
	case ChangelogWorkItems:
		ids := append([]string(nil), request.WorkItemIDs...)
		report.Sections = []ChangelogSection{newChangelogSection(nil, nil, ids, nil, len(ids) == 0)}
	case ChangelogPullRequests:
		if len(request.Repositories) == 0 {
			return ChangelogReport{}, repositoriesRequired("work changelog --from-pr", "requires configured work repositories")
		}
		reader, requireErr := work.Require[work.PullRequestReader](provider, work.CapabilityPullRequestReader)
		if requireErr != nil {
			return ChangelogReport{}, requireErr
		}
		if err := collectEvent(ctx, &report.Events, sink, Event{Kind: "resolving-pull-request-work-items", Repositories: append([]string(nil), request.Repositories...)}); err != nil {
			return ChangelogReport{}, err
		}
		ids := make([]string, 0)
		for _, repository := range request.Repositories {
			for _, pullRequestID := range request.PullRequestIDs {
				providerID, idErr := formatPullRequestID(pullRequestID)
				if idErr != nil {
					return ChangelogReport{}, idErr
				}
				resolved, readErr := reader.PullRequestWorkItemIDs(ctx, projectRef(request.Root, request.Project), work.RepositoryName(repository), providerID)
				if readErr != nil {
					return ChangelogReport{}, readErr
				}
				for _, id := range resolved {
					ids = appendDistinct(ids, string(id))
				}
			}
		}
		report.Sections = []ChangelogSection{newChangelogSection(nil, nil, ids, nil, len(ids) == 0)}
	case ChangelogGitRange:
		if s.GitChangelog == nil {
			return ChangelogReport{}, capabilityUnavailable("work git changelog")
		}
		if err := collectEvent(ctx, &report.Events, sink, Event{Kind: "extracting-git-work-items", GitTo: request.GitTo}); err != nil {
			return ChangelogReport{}, err
		}
		resolved, resolveErr := s.GitChangelog.ResolveGitRange(ctx, request)
		if resolveErr != nil {
			return ChangelogReport{}, resolveErr
		}
		for _, section := range resolved {
			repository, path := section.Repository, section.Path
			report.Sections = append(report.Sections, newChangelogSection(&repository, &path, section.WorkItemIDs, section.Warnings, section.SourceEmpty))
		}
	default:
		return ChangelogReport{}, problem(msgChangelogSource, "unsupported changelog source %q", l10n.A("source", request.Source))
	}
	for _, section := range report.Sections {
		for _, id := range section.WorkItemIDs {
			report.WorkItemIDs = appendDistinct(report.WorkItemIDs, id)
		}
	}
	if len(report.WorkItemIDs) == 0 || request.IDsOnly {
		return report, nil
	}
	reader, err := work.Require[work.ItemReader](provider, work.CapabilityItemReader)
	if err != nil {
		return ChangelogReport{}, err
	}
	if err := collectEvent(ctx, &report.Events, sink, Event{Kind: "loading-changelog-items", IDs: append([]string(nil), report.WorkItemIDs...)}); err != nil {
		return ChangelogReport{}, err
	}
	items, err := reader.ReadItems(ctx, projectRef(request.Root, request.Project), itemIDs(report.WorkItemIDs), work.ReadOptions{})
	if err != nil {
		return ChangelogReport{}, err
	}
	for index := range report.Sections {
		section := &report.Sections[index]
		for _, item := range items {
			if containsString(section.WorkItemIDs, string(item.ID)) {
				section.Items = append(section.Items, itemToSnapshot(item))
			}
		}
		sort.SliceStable(section.Items, func(i, j int) bool { return section.Items[i].ID < section.Items[j].ID })
		section.ResolvedEmpty = len(section.WorkItemIDs) > 0 && len(section.Items) == 0
	}
	if request.GroupByParent {
		if err := collectEvent(ctx, &report.Events, sink, Event{Kind: "grouping-assigned-work-items", Project: stringPtr(request.Project)}); err != nil {
			return ChangelogReport{}, err
		}
		for index := range report.Sections {
			section := &report.Sections[index]
			if len(section.Items) == 0 {
				continue
			}
			normalized := make([]work.Item, 0, len(section.Items))
			for _, item := range items {
				if containsString(section.WorkItemIDs, string(item.ID)) {
					normalized = append(normalized, item)
				}
			}
			groups, groupErr := s.groupItems(ctx, provider, request.Root, request.Project, normalized)
			if groupErr != nil {
				section.Warnings = append(section.Warnings, ChangelogWarning{Detail: "Could not group work items by parent: " + groupErr.Error()})
				continue
			}
			section.Groups = groups
		}
	}
	return report, nil
}

func newChangelogSection(repository, path *string, ids []string, warnings []ChangelogWarning, sourceEmpty bool) ChangelogSection {
	return ChangelogSection{Repository: repository, RepositoryPath: path, WorkItemIDs: append([]string(nil), ids...), Items: []ItemSnapshot{}, Groups: []ItemGroup{}, SourceEmpty: sourceEmpty, ResolvedEmpty: false, Warnings: append([]ChangelogWarning(nil), warnings...)}
}
func containsString(values []string, value string) bool {
	for _, candidate := range values {
		if candidate == value {
			return true
		}
	}
	return false
}
