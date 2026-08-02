package cockpit

import (
	"fmt"
	"strings"

	"github.com/sachahjkl/dw/internal/action"
)

type ResourceKind string

type Relation string

type InputKind string

const (
	ResourceRoot        ResourceKind = "root"
	ResourceProject     ResourceKind = "project"
	ResourceWorkItem    ResourceKind = "work-item"
	ResourceWorkspace   ResourceKind = "workspace"
	ResourcePullRequest ResourceKind = "pull-request"
	ResourceDataSource  ResourceKind = "data-source"
)

const (
	RelationInitialize        Relation = "initialize"
	RelationDoctor            Relation = "doctor"
	RelationRefresh           Relation = "refresh"
	RelationInspect           Relation = "inspect"
	RelationOpenWorkspace     Relation = "open-workspace"
	RelationAuthenticate      Relation = "authenticate"
	RelationReviewStart       Relation = "review-start"
	RelationStart             Relation = "start"
	RelationShowContext       Relation = "show-context"
	RelationChangeState       Relation = "change-state"
	RelationPreflight         Relation = "preflight"
	RelationSync              Relation = "sync"
	RelationUpdateRepos       Relation = "update-repositories"
	RelationValidateHandoff   Relation = "validate-handoff"
	RelationReviewCommit      Relation = "review-commit"
	RelationReviewFinish      Relation = "review-finish"
	RelationFinish            Relation = "finish"
	RelationReviewRemoval     Relation = "review-removal"
	RelationRemove            Relation = "remove"
	RelationReviewDiff        Relation = "review-diff"
	RelationChangelog         Relation = "changelog"
	RelationCatalog           Relation = "catalog"
	RelationReviewPrune       Relation = "review-prune"
	RelationOpenLink          Relation = "open-link"
	RelationViewConfiguration Relation = "view-configuration"
	RelationValidateConfig    Relation = "validate-configuration"
	RelationRefreshConfig     Relation = "refresh-configuration"
	RelationShowGuide         Relation = "show-guide"
	RelationValidateAgent     Relation = "validate-agent"
	RelationSetAgentOpenCode  Relation = "set-default-agent-opencode"
	RelationSetAgentCursor    Relation = "set-default-agent-cursor"
	RelationSetAgentClaude    Relation = "set-default-agent-claude"
	RelationSetAgentCodex     Relation = "set-default-agent-codex"
	RelationSetAgentCodexCLI  Relation = "set-default-agent-codex-cli"
	RelationSetAgentCopilot   Relation = "set-default-agent-copilot"
	RelationSetColorAuto      Relation = "set-color-auto"
	RelationSetColorAlways    Relation = "set-color-always"
	RelationSetColorNever     Relation = "set-color-never"
)

func (relation Relation) Valid() bool {
	switch relation {
	case RelationInitialize, RelationDoctor, RelationRefresh, RelationInspect, RelationOpenWorkspace,
		RelationAuthenticate, RelationReviewStart, RelationStart, RelationShowContext, RelationChangeState, RelationPreflight,
		RelationSync, RelationUpdateRepos, RelationValidateHandoff, RelationReviewCommit, RelationReviewFinish,
		RelationFinish, RelationReviewRemoval, RelationRemove, RelationReviewDiff, RelationChangelog,
		RelationCatalog, RelationReviewPrune, RelationOpenLink, RelationViewConfiguration, RelationValidateConfig,
		RelationRefreshConfig, RelationShowGuide, RelationValidateAgent, RelationSetAgentOpenCode,
		RelationSetAgentCursor, RelationSetAgentClaude, RelationSetAgentCodex, RelationSetAgentCodexCLI,
		RelationSetAgentCopilot, RelationSetColorAuto, RelationSetColorAlways, RelationSetColorNever:
		return true
	default:
		return false
	}
}

const (
	InputText    InputKind = "text"
	InputBoolean InputKind = "boolean"
	InputSelect  InputKind = "select"
)

type ResourceRef struct {
	Kind    ResourceKind
	Root    string
	Project string
	Key     string
}

func (reference ResourceRef) Validate() error {
	if strings.TrimSpace(reference.Root) == "" || strings.TrimSpace(reference.Key) == "" {
		return fmt.Errorf("cockpit.invalid-resource-reference")
	}
	switch reference.Kind {
	case ResourceRoot, ResourceWorkspace:
		return nil
	case ResourceProject, ResourceWorkItem, ResourcePullRequest, ResourceDataSource:
		if strings.TrimSpace(reference.Project) == "" {
			return fmt.Errorf("cockpit.invalid-resource-reference")
		}
		return nil
	default:
		return fmt.Errorf("cockpit.invalid-resource-kind:%s", reference.Kind)
	}
}

func (reference ResourceRef) Equal(other ResourceRef) bool {
	return reference.Kind == other.Kind && reference.Root == other.Root && reference.Project == other.Project && reference.Key == other.Key
}

type InputOption struct {
	Value string
	Label string
}

type OperationInput struct {
	Name     string
	Label    string
	Kind     InputKind
	Required bool
	Options  []InputOption
}

type InputValue struct {
	Name  string
	Value string
}

type RequestBuilder func([]InputValue) (action.Request, error)

func validateInputDefinitions(inputs []OperationInput) error {
	seen := make(map[string]struct{}, len(inputs))
	for _, input := range inputs {
		if strings.TrimSpace(input.Name) == "" || strings.TrimSpace(input.Label) == "" {
			return fmt.Errorf("cockpit.invalid-operation-input")
		}
		if _, duplicate := seen[input.Name]; duplicate {
			return fmt.Errorf("cockpit.duplicate-operation-input:%s", input.Name)
		}
		seen[input.Name] = struct{}{}
		switch input.Kind {
		case InputText, InputBoolean:
			if len(input.Options) != 0 {
				return fmt.Errorf("cockpit.unexpected-operation-options:%s", input.Name)
			}
		case InputSelect:
			if len(input.Options) == 0 {
				return fmt.Errorf("cockpit.operation-options-required:%s", input.Name)
			}
			options := make(map[string]struct{}, len(input.Options))
			for _, option := range input.Options {
				if strings.TrimSpace(option.Value) == "" || strings.TrimSpace(option.Label) == "" {
					return fmt.Errorf("cockpit.invalid-operation-option:%s", input.Name)
				}
				if _, duplicate := options[option.Value]; duplicate {
					return fmt.Errorf("cockpit.duplicate-operation-option:%s", input.Name)
				}
				options[option.Value] = struct{}{}
			}
		default:
			return fmt.Errorf("cockpit.invalid-operation-input-kind:%s", input.Kind)
		}
	}
	return nil
}

func validateInputValues(definitions []OperationInput, values []InputValue) error {
	provided := make(map[string]string, len(values))
	for _, value := range values {
		if _, duplicate := provided[value.Name]; duplicate {
			return fmt.Errorf("cockpit.duplicate-operation-input:%s", value.Name)
		}
		provided[value.Name] = value.Value
	}
	for _, definition := range definitions {
		value, exists := provided[definition.Name]
		if exists {
			delete(provided, definition.Name)
		}
		if definition.Required && (!exists || strings.TrimSpace(value) == "") {
			return fmt.Errorf("cockpit.operation-input-required:%s", definition.Name)
		}
		if !exists || value == "" {
			continue
		}
		switch definition.Kind {
		case InputBoolean:
			if value != "true" && value != "false" {
				return fmt.Errorf("cockpit.invalid-operation-input:%s", definition.Name)
			}
		case InputSelect:
			allowed := false
			for _, option := range definition.Options {
				if option.Value == value {
					allowed = true
					break
				}
			}
			if !allowed {
				return fmt.Errorf("cockpit.invalid-operation-input:%s", definition.Name)
			}
		}
	}
	for name := range provided {
		return fmt.Errorf("cockpit.unknown-operation-input:%s", name)
	}
	return nil
}

func (operation Operation) Validate() error {
	if !operation.Relation.Valid() || operation.Request == nil || strings.TrimSpace(operation.Label) == "" {
		return fmt.Errorf("cockpit.invalid-operation")
	}
	if err := operation.Subject.Validate(); err != nil {
		return err
	}
	if err := validateInputDefinitions(operation.Inputs); err != nil {
		return err
	}
	if len(operation.Inputs) != 0 && operation.Build == nil {
		return fmt.Errorf("cockpit.operation-builder-required:%s", operation.Relation)
	}
	return nil
}

func (operation Operation) BuildRequest(values []InputValue) (action.Request, error) {
	if err := operation.Validate(); err != nil {
		return nil, err
	}
	if err := validateInputValues(operation.Inputs, values); err != nil {
		return nil, err
	}
	if operation.Build != nil {
		request, err := operation.Build(values)
		if err != nil {
			return nil, err
		}
		if request == nil || request.ActionID() != operation.Request.ActionID() {
			return nil, fmt.Errorf("cockpit.invalid-operation-request:%s", operation.Relation)
		}
		return request, nil
	}
	return operation.Request, nil
}
