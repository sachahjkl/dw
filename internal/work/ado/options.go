package ado

import "strings"

func ResolveOptions(workflow, project *Options) (Options, error) {
	if workflow == nil && project == nil {
		return Options{}, &Error{Kind: ErrorInvalidInput, Detail: "Missing azureDevOps configuration."}
	}
	if workflow == nil {
		return normalizedOptions(*project), nil
	}
	if project == nil {
		return normalizedOptions(*workflow), nil
	}
	result := *project
	if strings.TrimSpace(result.Organization) == "" {
		result.Organization = workflow.Organization
	}
	if strings.TrimSpace(result.Project) == "" {
		result.Project = workflow.Project
	}
	if strings.TrimSpace(result.APIVersion) == "" {
		result.APIVersion = workflow.APIVersion
	}
	return normalizedOptions(result), nil
}

func normalizedOptions(options Options) Options {
	if strings.TrimSpace(options.APIVersion) == "" {
		options.APIVersion = DefaultAPIVersion
	}
	return options
}
