package config

import (
	"errors"
	"os"
	"path/filepath"
	"strings"

	"github.com/sachahjkl/dw/internal/l10n"
	"github.com/sachahjkl/dw/internal/wirejson"
)

func LoadUserSettings() UserSettings {
	settings, err := LoadUserSettingsChecked()
	if err != nil {
		return UserSettings{}
	}
	return settings
}

func LoadUserSettingsChecked() (UserSettings, error) {
	document, err := readOrderedJSON(UserSettingsPath())
	if err != nil {
		return UserSettings{}, err
	}
	settings := UserSettings{document: retained(document)}
	if root, ok := document.Lookup("root"); ok && !root.IsNull() {
		value, valid := root.AsString()
		if !valid {
			return UserSettings{}, errors.New("config.settings-root-not-string")
		}
		settings.Root = &value
	}
	if color, ok := document.Lookup("color"); ok && !color.IsNull() {
		value, valid := color.AsString()
		if !valid {
			return UserSettings{}, errors.New("config.settings-color-not-string")
		}
		mode := ColorMode(value)
		if !mode.Valid() {
			return UserSettings{}, errors.New("config.settings-color-invalid")
		}
		settings.Color = &mode
	}
	return settings, nil
}

func SaveUserSettings(settings UserSettings) error {
	document := wirejson.ObjectValue(
		wirejson.Member{Name: "root", Value: optionalStringValue(settings.Root)},
		wirejson.Member{Name: "color", Value: optionalColorValue(settings.Color)},
	)
	if settings.document != nil && settings.document.Kind() == wirejson.Object {
		document = settings.document.Clone()
		if err := document.Set("root", optionalStringValue(settings.Root)); err != nil {
			return err
		}
		if err := document.Set("color", optionalColorValue(settings.Color)); err != nil {
			return err
		}
	}
	content, err := wirejson.Pretty(document)
	if err != nil {
		return err
	}
	if err = os.MkdirAll(UserConfigDirectory(), 0o755); err != nil {
		return err
	}
	return os.WriteFile(UserSettingsPath(), content, 0o644)
}

func SetUserRoot(root string) (string, error) {
	normalized, err := NormalizePath(root)
	if err != nil {
		return "", err
	}
	settings := LoadUserSettings()
	settings.Root = &normalized
	if err = SaveUserSettings(settings); err != nil {
		return "", err
	}
	return normalized, nil
}

func NormalizeColorMode(mode *ColorMode) ColorMode {
	if mode == nil {
		return ColorAuto
	}
	return *mode
}

func ParseColorMode(value string) (ColorMode, error) {
	trimmed := strings.TrimSpace(value)
	if trimmed == "" {
		return ColorAuto, nil
	}
	for _, allowed := range ColorModeChoices {
		if strings.EqualFold(trimmed, string(allowed)) {
			return allowed, nil
		}
	}
	choices := make([]string, len(ColorModeChoices))
	for index, choice := range ColorModeChoices {
		choices[index] = string(choice)
	}
	return "", localizedError(l10n.M("config.unknown_color", l10n.A("mode", value), l10n.A("choices", strings.Join(choices, ", "))))
}

func SetColorMode(mode ColorMode) (ColorMode, error) {
	normalized, err := ParseColorMode(string(mode))
	if err != nil {
		return "", err
	}
	settings := LoadUserSettings()
	settings.Color = &normalized
	if err = SaveUserSettings(settings); err != nil {
		return "", err
	}
	return normalized, nil
}

func NormalizeDefaultAgent(value string) (Agent, bool) {
	trimmed := strings.TrimSpace(value)
	for _, allowed := range AgentDefaultChoices {
		if strings.EqualFold(trimmed, string(allowed)) {
			return allowed, true
		}
	}
	return "", false
}

func DefaultAgent(root string) Agent {
	workflow := LoadWorkflowConfig(root)
	if workflow.Agent == nil {
		return AgentOpenCode
	}
	value := strings.TrimSpace(workflow.Agent.Default)
	if value == "" {
		return AgentOpenCode
	}
	agent, ok := NormalizeDefaultAgent(value)
	if !ok {
		return AgentOpenCode
	}
	return agent
}

func SetDefaultAgent(root string, agent Agent) (Agent, error) {
	if normalized, ok := NormalizeDefaultAgent(string(agent)); ok {
		agent = normalized
	} else {
		return "", errors.New("config.unknown-agent")
	}
	path := filepath.Join(root, "config", "workflow.json")
	data, err := os.ReadFile(path)
	if err != nil {
		return "", err
	}
	document, parseErr := wirejson.Parse(data)
	if parseErr != nil {
		return "", parseErr
	}
	if document.Kind() != wirejson.Object {
		document = wirejson.ObjectValue()
	}
	agentNode, exists := document.Lookup("agent")
	if !exists {
		value := wirejson.ObjectValue()
		if err = document.Set("agent", value); err != nil {
			return "", err
		}
		agentNode, _ = document.Lookup("agent")
	} else if agentNode.Kind() != wirejson.Object {
		return "", localizedError(l10n.M("config.workflow_agent_object"))
	}
	if err = agentNode.Set("default", wirejson.StringValue(string(agent))); err != nil {
		return "", err
	}
	content, err := wirejson.Pretty(document)
	if err != nil {
		return "", err
	}
	if err = os.WriteFile(path, content, 0o644); err != nil {
		return "", err
	}
	return agent, nil
}

func optionalStringValue(value *string) wirejson.Value {
	if value == nil {
		return wirejson.NullValue()
	}
	return wirejson.StringValue(*value)
}
func optionalColorValue(value *ColorMode) wirejson.Value {
	if value == nil {
		return wirejson.NullValue()
	}
	return wirejson.StringValue(string(*value))
}

func (settings UserSettings) MarshalJSON() ([]byte, error) {
	if settings.document != nil {
		return wirejson.Compact(settings.document.Clone())
	}
	document := wirejson.ObjectValue(
		wirejson.Member{Name: "root", Value: optionalStringValue(settings.Root)},
		wirejson.Member{Name: "color", Value: optionalColorValue(settings.Color)},
	)
	return wirejson.Compact(document)
}
