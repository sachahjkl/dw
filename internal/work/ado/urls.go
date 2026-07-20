package ado

import (
	"fmt"
	"strings"
)

func apiVersion(options Options) string {
	if strings.TrimSpace(options.APIVersion) == "" {
		return DefaultAPIVersion
	}
	return options.APIVersion
}

func organizationRoot(options Options) string { return strings.TrimRight(options.Organization, "/") }

func encodeComponent(value string) string {
	value = strings.ReplaceAll(value, " ", "%20")
	return strings.ReplaceAll(value, "/", "%2F")
}

func ExpandedWorkItemURL(options Options, id string) string {
	return fmt.Sprintf("%s/%s/_apis/wit/workitems/%s?$expand=all&api-version=%s", organizationRoot(options), options.Project, id, apiVersion(options))
}

func WorkItemCommentsURL(options Options, id string, top uint32) string {
	return fmt.Sprintf("%s/%s/_apis/wit/workItems/%s/comments?$top=%d&api-version=%s", organizationRoot(options), options.Project, id, top, apiVersion(options))
}

func WorkItemURL(options Options, id string) string {
	return fmt.Sprintf("%s/%s/_apis/wit/workitems/%s?api-version=%s", organizationRoot(options), options.Project, id, apiVersion(options))
}

func WorkItemsBatchURL(options Options) string {
	return fmt.Sprintf("%s/%s/_apis/wit/workitemsbatch?api-version=%s", organizationRoot(options), options.Project, apiVersion(options))
}

func WIQLURL(options Options, top int) string {
	return fmt.Sprintf("%s/%s/_apis/wit/wiql?$top=%d&api-version=%s", organizationRoot(options), options.Project, top, apiVersion(options))
}

func WorkItemAPIURL(options Options, id string) string {
	return fmt.Sprintf("%s/%s/_apis/wit/workItems/%s", organizationRoot(options), options.Project, id)
}

func WorkItemWebURL(options Options, id string) string {
	return fmt.Sprintf("%s/%s/_workitems/edit/%s", organizationRoot(options), encodeComponent(options.Project), id)
}

func CreateWorkItemURL(options Options, workItemType string) string {
	return fmt.Sprintf("%s/%s/_apis/wit/workitems/$%s?api-version=%s", organizationRoot(options), options.Project, encodeComponent(workItemType), apiVersion(options))
}

func PullRequestsURL(options Options, repository string) string {
	return fmt.Sprintf("%s/%s/_apis/git/repositories/%s/pullrequests?api-version=%s", organizationRoot(options), options.Project, encodeComponent(repository), apiVersion(options))
}

func PullRequestWebURL(options Options, repository string, id int64) string {
	return fmt.Sprintf("%s/%s/_git/%s/pullrequest/%d", organizationRoot(options), encodeComponent(options.Project), encodeComponent(repository), id)
}

func ActivePullRequestsURL(options Options, repository, sourceRef string) string {
	return fmt.Sprintf("%s/%s/_apis/git/repositories/%s/pullrequests?searchCriteria.status=active&searchCriteria.sourceRefName=%s&api-version=%s", organizationRoot(options), options.Project, encodeComponent(repository), encodeComponent(sourceRef), apiVersion(options))
}

func ActivePullRequestsForRepositoryURL(options Options, repository string) string {
	return fmt.Sprintf("%s/%s/_apis/git/repositories/%s/pullrequests?searchCriteria.status=active&api-version=%s", organizationRoot(options), options.Project, encodeComponent(repository), apiVersion(options))
}

func PullRequestWorkItemsURL(options Options, repository string, id int64) string {
	return fmt.Sprintf("%s/%s/_apis/git/repositories/%s/pullRequests/%d/workitems?api-version=%s", organizationRoot(options), options.Project, encodeComponent(repository), id, apiVersion(options))
}

func ConnectionDataURL(options Options) string {
	return fmt.Sprintf("%s/_apis/connectionData?connectOptions=1&lastChangeId=-1&lastChangeId64=-1", organizationRoot(options))
}

func OrganizationName(value string) string {
	trimmed := strings.TrimRight(strings.TrimSpace(value), "/")
	if index := strings.LastIndexByte(trimmed, '/'); index >= 0 && strings.TrimSpace(trimmed[index+1:]) != "" {
		return trimmed[index+1:]
	}
	return trimmed
}
