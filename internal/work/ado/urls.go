package ado

import (
	"net/url"
	"strconv"
	"strings"
)

func apiVersion(options Options) string {
	if strings.TrimSpace(options.APIVersion) == "" {
		return DefaultAPIVersion
	}
	return options.APIVersion
}

func adoURL(options Options, segments []string, query adoQuery) string {
	value, err := url.Parse(strings.TrimRight(options.Organization, "/"))
	if err != nil {
		value = &url.URL{Path: strings.TrimRight(options.Organization, "/")}
	}
	path := strings.TrimRight(value.Path, "/")
	escapedPath := strings.TrimRight(value.EscapedPath(), "/")
	for _, segment := range segments {
		path += "/" + segment
		escapedPath += "/" + url.PathEscape(segment)
	}
	value.Path = path
	value.RawPath = escapedPath
	value.RawQuery = query.encode()
	value.Fragment = ""
	return value.String()
}

type adoQuery []string

func (values adoQuery) encode() string {
	var result strings.Builder
	for index := 0; index < len(values); index += 2 {
		if index != 0 {
			result.WriteByte('&')
		}
		result.WriteString(strings.ReplaceAll(url.QueryEscape(values[index]), "%24", "$"))
		result.WriteByte('=')
		result.WriteString(url.QueryEscape(values[index+1]))
	}
	return result.String()
}

func query(values ...string) adoQuery {
	return values
}

func ExpandedWorkItemURL(options Options, id string) string {
	return adoURL(options, []string{options.Project, "_apis", "wit", "workitems", id}, query("$expand", "all", "api-version", apiVersion(options)))
}

func WorkItemCommentsURL(options Options, id string, top int) string {
	return adoURL(options, []string{options.Project, "_apis", "wit", "workItems", id, "comments"}, query("$top", strconv.Itoa(top), "api-version", CommentsAPIVersion))
}

func WorkItemURL(options Options, id string) string {
	return adoURL(options, []string{options.Project, "_apis", "wit", "workitems", id}, query("api-version", apiVersion(options)))
}

func WorkItemsBatchURL(options Options) string {
	return adoURL(options, []string{options.Project, "_apis", "wit", "workitemsbatch"}, query("api-version", apiVersion(options)))
}

func WIQLURL(options Options, top int) string {
	return adoURL(options, []string{options.Project, "_apis", "wit", "wiql"}, query("$top", strconv.Itoa(top), "api-version", apiVersion(options)))
}

func WorkItemAPIURL(options Options, id string) string {
	return adoURL(options, []string{options.Project, "_apis", "wit", "workItems", id}, nil)
}

func WorkItemWebURL(options Options, id string) string {
	return adoURL(options, []string{options.Project, "_workitems", "edit", id}, nil)
}

func CreateWorkItemURL(options Options, workItemType string) string {
	return adoURL(options, []string{options.Project, "_apis", "wit", "workitems", "$" + workItemType}, query("api-version", apiVersion(options)))
}

func PullRequestsURL(options Options, repository string) string {
	return adoURL(options, []string{options.Project, "_apis", "git", "repositories", repository, "pullrequests"}, query("api-version", apiVersion(options)))
}

func PullRequestWebURL(options Options, repository string, id int64) string {
	return adoURL(options, []string{options.Project, "_git", repository, "pullrequest", strconv.FormatInt(id, 10)}, nil)
}

func PullRequestURL(options Options, repository string, id int64) string {
	return adoURL(options, []string{options.Project, "_apis", "git", "repositories", repository, "pullRequests", strconv.FormatInt(id, 10)}, query("api-version", apiVersion(options)))
}

func ActivePullRequestsURL(options Options, repository, sourceRef string) string {
	return adoURL(options, []string{options.Project, "_apis", "git", "repositories", repository, "pullrequests"}, query("searchCriteria.status", "active", "searchCriteria.sourceRefName", sourceRef, "api-version", apiVersion(options)))
}

func ActivePullRequestsForRepositoryURL(options Options, repository string, skip, top int) string {
	return adoURL(options, []string{options.Project, "_apis", "git", "repositories", repository, "pullrequests"}, query("searchCriteria.status", "active", "$skip", strconv.Itoa(skip), "$top", strconv.Itoa(top), "api-version", apiVersion(options)))
}

func PullRequestWorkItemsURL(options Options, repository string, id int64) string {
	return adoURL(options, []string{options.Project, "_apis", "git", "repositories", repository, "pullRequests", strconv.FormatInt(id, 10), "workitems"}, query("api-version", apiVersion(options)))
}

func ConnectionDataURL(options Options) string {
	return adoURL(options, []string{"_apis", "connectionData"}, query("connectOptions", "1", "lastChangeId", "-1", "lastChangeId64", "-1"))
}

func OrganizationName(value string) string {
	trimmed := strings.TrimRight(strings.TrimSpace(value), "/")
	parsed, err := url.Parse(trimmed)
	if err == nil {
		path := strings.Trim(parsed.Path, "/")
		if path != "" {
			parts := strings.Split(path, "/")
			return parts[len(parts)-1]
		}
		if parsed.Host != "" {
			return parsed.Host
		}
	}
	return trimmed
}
