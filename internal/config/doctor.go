package config

import (
	"os"
	"path/filepath"
	"strings"

	"github.com/sachahjkl/dw/internal/l10n"
	"github.com/sachahjkl/dw/internal/wirejson"
)

func Show(explicitRoot string) ConfigShow {
	root := ResolveRoot(explicitRoot)
	settings := LoadUserSettings()
	workflowPath := filepath.Join(root, "config", "workflow.json")
	projectsPath := filepath.Join(root, "config", "projects.json")
	databasesPath := filepath.Join(root, "config", "databases.json")
	return ConfigShow{
		Root: root, Color: NormalizeColorMode(settings.Color), SettingsPath: UserSettingsPath(),
		WorkflowPath: workflowPath, ProjectsPath: projectsPath, DatabasesPath: databasesPath,
		WorkflowExists: pathExists(workflowPath), ProjectsExists: pathExists(projectsPath),
		DatabasesExists: pathExists(databasesPath),
	}
}

func Status(explicitRoot string) RootStatus {
	root := ResolveRoot(explicitRoot)
	required := []string{
		filepath.Join(root, "config", "projects.json"),
		filepath.Join(root, "config", "workflow.json"),
		filepath.Join(root, "config", "databases.json"),
	}
	missing := make([]string, 0)
	for _, path := range required {
		info, err := os.Stat(path)
		if err != nil || !info.Mode().IsRegular() {
			missing = append(missing, path)
		}
	}
	return RootStatus{Root: root, Initialized: len(missing) == 0, MissingPaths: missing}
}

func Doctor(explicitRoot string) ConfigDoctorReport {
	root := ResolveRoot(explicitRoot)
	checks := []ConfigDoctorCheck{
		checkKnownConfig(filepath.Join(root, "config", "projects.json"), []string{"schema", "projects"}),
		checkKnownConfig(filepath.Join(root, "config", "workflow.json"), []string{"schema", "branchPrefixes", "azureDevOps", "auth", "updates"}),
		checkKnownConfig(filepath.Join(root, "config", "databases.json"), []string{"schema", "defaults", "globals", "projects"}),
		checkJSONC(filepath.Join(root, "config", "opencode", "opencode.jsonc")),
		checkExists(filepath.Join(root, "schemas", "projects.schema.json")),
		checkExists(filepath.Join(root, "schemas", "workflow.schema.json")),
		checkExists(filepath.Join(root, "schemas", "databases.schema.json")),
	}
	passed := true
	for _, check := range checks {
		if !check.Passed {
			passed = false
			break
		}
	}
	return ConfigDoctorReport{Root: root, Checks: checks, Passed: passed}
}

func checkKnownConfig(path string, required []string) ConfigDoctorCheck {
	data, err := os.ReadFile(path)
	if err != nil {
		return doctorCheck(path, false, messageText("config.missing_file"))
	}
	value, err := wirejson.Parse(data)
	if err != nil {
		message := err.Error()
		return doctorCheck(path, false, &message)
	}
	if value.Kind() != wirejson.Object {
		return doctorCheck(path, false, messageText("config.root_json_object"))
	}
	missing := make([]string, 0)
	for _, property := range required {
		if _, ok := value.Lookup(property); !ok {
			missing = append(missing, property)
		}
	}
	if len(missing) == 0 {
		return doctorCheck(path, true, nil)
	}
	message := l10n.Render(l10n.M("config.missing_properties", l10n.A("properties", strings.Join(missing, ", "))))
	return doctorCheck(path, false, &message)
}

func checkJSONC(path string) ConfigDoctorCheck {
	data, err := os.ReadFile(path)
	if err != nil {
		return doctorCheck(path, false, messageText("config.missing_file"))
	}
	_, err = wirejson.Parse(stripJSONCComments(data))
	if err == nil {
		return doctorCheck(path, true, nil)
	}
	message := err.Error()
	return doctorCheck(path, false, &message)
}

func checkExists(path string) ConfigDoctorCheck {
	if pathExists(path) {
		return doctorCheck(path, true, nil)
	}
	return doctorCheck(path, false, messageText("config.missing_file"))
}

func doctorCheck(path string, passed bool, message *string) ConfigDoctorCheck {
	return ConfigDoctorCheck{Path: path, Passed: passed, Message: message}
}

func messageText(id l10n.ID) *string { value := l10n.Text(id); return &value }

func pathExists(path string) bool { _, err := os.Stat(path); return err == nil }

func stripJSONCComments(input []byte) []byte {
	output := make([]byte, 0, len(input))
	inString, escaped := false, false
	for index := 0; index < len(input); index++ {
		char := input[index]
		if inString {
			output = append(output, char)
			if escaped {
				escaped = false
			} else if char == '\\' {
				escaped = true
			} else if char == '"' {
				inString = false
			}
			continue
		}
		if char == '"' {
			inString = true
			output = append(output, char)
			continue
		}
		if char == '/' && index+1 < len(input) && input[index+1] == '/' {
			index += 2
			for index < len(input) && input[index] != '\n' {
				index++
			}
			if index < len(input) {
				output = append(output, '\n')
			}
			continue
		}
		output = append(output, char)
	}
	return output
}
