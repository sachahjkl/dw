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
	result.ContentFields = mergeContentFields(workflow.ContentFields, project.ContentFields)
	return normalizedOptions(result), nil
}

func normalizedOptions(options Options) Options {
	if strings.TrimSpace(options.APIVersion) == "" {
		options.APIVersion = DefaultAPIVersion
	}
	options.ContentFields = mergeContentFields(defaultContentFields(), options.ContentFields)
	return options
}

func defaultContentFields() ContentFields {
	return ContentFields{
		ContentFieldMapping: ContentFieldMapping{
			Description:        "System.Description",
			AcceptanceCriteria: "Microsoft.VSTS.Common.AcceptanceCriteria",
		},
		WorkItemTypes: map[string]ContentFieldMapping{
			"Bug": {Description: "Microsoft.VSTS.TCM.ReproSteps"},
		},
	}
}

func mergeContentFields(base, override ContentFields) ContentFields {
	result := ContentFields{ContentFieldMapping: base.ContentFieldMapping, WorkItemTypes: make(map[string]ContentFieldMapping)}
	if value := strings.TrimSpace(override.Description); value != "" {
		result.Description = value
	}
	if value := strings.TrimSpace(override.AcceptanceCriteria); value != "" {
		result.AcceptanceCriteria = value
	}
	for name, mapping := range base.WorkItemTypes {
		result.WorkItemTypes[name] = mapping
	}
	for name, mapping := range override.WorkItemTypes {
		matched := name
		for existing := range result.WorkItemTypes {
			if strings.EqualFold(strings.TrimSpace(existing), strings.TrimSpace(name)) {
				matched = existing
				break
			}
		}
		result.WorkItemTypes[matched] = mergeContentFieldMapping(result.WorkItemTypes[matched], mapping)
	}
	return result
}

func mergeContentFieldMapping(base, override ContentFieldMapping) ContentFieldMapping {
	if value := strings.TrimSpace(override.Description); value != "" {
		base.Description = value
	}
	if value := strings.TrimSpace(override.AcceptanceCriteria); value != "" {
		base.AcceptanceCriteria = value
	}
	return base
}

func contentFieldMapping(options Options, itemType string) ContentFieldMapping {
	fields := normalizedOptions(options).ContentFields
	result := fields.ContentFieldMapping
	for name, mapping := range fields.WorkItemTypes {
		if strings.EqualFold(strings.TrimSpace(name), strings.TrimSpace(itemType)) {
			return mergeContentFieldMapping(result, mapping)
		}
	}
	return result
}
