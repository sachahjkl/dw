package sqlserver

import (
	"bytes"
	"context"
	"encoding/json"
	"io/fs"
	"os"
	"path/filepath"
	"sort"
	"strings"
	"unicode"

	"github.com/sachahjkl/dw/internal/contract"
	"github.com/sachahjkl/dw/internal/data"
	"github.com/sachahjkl/dw/internal/l10n"
)

const maxAppSettingsBytes int64 = 5 * 1024 * 1024

var skippedDiscoveryDirectories = map[string]struct{}{
	".git": {}, ".idea": {}, ".vs": {}, ".vscode": {}, "bin": {}, "dist": {},
	"node_modules": {}, "obj": {}, "out": {}, "target": {},
}

type discoveryCandidate struct {
	source data.DiscoveredSource
}

// Discover scans caller-resolved repository roots for ASP.NET connection strings.
// Plaintext stays in opaque SecretValue fields and is never projected by reports.
func (provider *Provider) Discover(_ context.Context, request data.DiscoveryRequest) (data.DiscoveryReport, error) {
	candidates := make([]discoveryCandidate, 0)
	identities := make(map[string]int)
	report := data.DiscoveryReport{Sources: []data.DiscoveredSource{}, Warnings: []string{}}
	for _, workspace := range request.Workspaces {
		project, _ := workspace.Project.Get()
		for _, repository := range workspace.Repositories {
			provider.scanRepository(repository.Root, string(project), repository.Name, &candidates, identities, &report)
		}
	}
	report.Sources = make([]data.DiscoveredSource, len(candidates))
	for index := range candidates {
		report.Sources[index] = candidates[index].source
	}
	return report, nil
}

func (provider *Provider) scanRepository(repositoryRoot, project, repository string, candidates *[]discoveryCandidate, identities map[string]int, report *data.DiscoveryReport) {
	files := collectAppSettingsFiles(repositoryRoot, &report.Warnings)
	for _, file := range files {
		report.ScannedFiles++
		provider.scanAppSettingsFile(repositoryRoot, file, project, repository, candidates, identities, &report.Warnings)
	}
}

func collectAppSettingsFiles(repositoryRoot string, warnings *[]string) []string {
	info, err := os.Stat(repositoryRoot)
	if err != nil || !info.IsDir() {
		if err == nil {
			err = l10nError("data.collect.not_directory")
		}
		*warnings = append(*warnings, l10n.Render(l10n.M("data.collect.scan_error", l10n.A("path", repositoryRoot), l10n.A("error", err))))
		return nil
	}
	files := make([]string, 0)
	err = filepath.WalkDir(repositoryRoot, func(path string, entry fs.DirEntry, walkErr error) error {
		if walkErr != nil {
			if path == repositoryRoot {
				return walkErr
			}
			return nil
		}
		if path == repositoryRoot {
			return nil
		}
		if entry.Type()&os.ModeSymlink != 0 {
			if entry.IsDir() {
				return filepath.SkipDir
			}
			return nil
		}
		if entry.IsDir() {
			if _, skipped := skippedDiscoveryDirectories[strings.ToLower(entry.Name())]; skipped {
				return filepath.SkipDir
			}
			return nil
		}
		if entry.Type().IsRegular() && isAppSettingsFile(entry.Name()) {
			files = append(files, path)
		}
		return nil
	})
	if err != nil {
		*warnings = append(*warnings, l10n.Render(l10n.M("data.collect.scan_error", l10n.A("path", repositoryRoot), l10n.A("error", err))))
		return nil
	}
	sort.Strings(files)
	return files
}

func (provider *Provider) scanAppSettingsFile(repositoryRoot, path, project, repository string, candidates *[]discoveryCandidate, identities map[string]int, warnings *[]string) {
	info, err := os.Stat(path)
	if err == nil && info.Size() > maxAppSettingsBytes {
		*warnings = append(*warnings, l10n.Render(l10n.M("data.collect.oversized", l10n.A("path", path))))
		return
	}
	content, err := os.ReadFile(path)
	if err != nil {
		*warnings = append(*warnings, l10n.Render(l10n.M("data.collect.read_error", l10n.A("path", path), l10n.A("error", err))))
		return
	}
	content = bytes.TrimPrefix(content, []byte{0xef, 0xbb, 0xbf})
	var root map[string]json.RawMessage
	if err := json.Unmarshal(content, &root); err != nil {
		*warnings = append(*warnings, l10n.Render(l10n.M("data.collect.parse_error", l10n.A("path", path), l10n.A("error", err))))
		return
	}
	var connectionStringsRaw json.RawMessage
	for key, value := range root {
		if strings.EqualFold(key, "ConnectionStrings") {
			connectionStringsRaw = value
			break
		}
	}
	if len(connectionStringsRaw) == 0 {
		return
	}
	var connectionStrings map[string]json.RawMessage
	if json.Unmarshal(connectionStringsRaw, &connectionStrings) != nil {
		return
	}
	application := applicationName(repositoryRoot, path)
	environment := appSettingsEnvironment(path)
	for _, name := range sortedDiscoveryKeys(connectionStrings) {
		var connectionString string
		if json.Unmarshal(connectionStrings[name], &connectionString) != nil || strings.TrimSpace(connectionString) == "" {
			continue
		}
		eligible := isSQLServerConnectionString(connectionString)
		database := generatedDatabaseKey(repository, application, environment, name)
		credentialKey := generatedCredentialKey(project, repository, application, environment, name)
		identity := strings.ToLower(strings.Join([]string{project, repository, application, environment, name}, "|"))
		sourcePath := path
		if index, found := identities[identity]; found {
			existing := &(*candidates)[index].source
			if EqualSecrets(existing.Secret, contract.NewSecretValue(connectionString)) {
				if !containsDiscoveryPath(existing.SourcePaths, sourcePath) {
					existing.SourcePaths = append(existing.SourcePaths, sourcePath)
				}
			} else {
				existing.Eligible = false
				existing.Detail = l10n.Text("data.collect.different_values")
				if !containsDiscoveryPath(existing.SourcePaths, sourcePath) {
					existing.SourcePaths = append(existing.SourcePaths, sourcePath)
				}
			}
			continue
		}
		identities[identity] = len(*candidates)
		projectOption := contract.None[contract.ProjectKey]()
		if strings.TrimSpace(project) != "" {
			projectOption = contract.Some(contract.ProjectKey(project))
		}
		detail := ""
		if !eligible {
			detail = l10n.Text("data.collect.not_sqlserver")
		}
		*candidates = append(*candidates, discoveryCandidate{source: data.DiscoveredSource{
			Source:     data.Source{Key: data.SourceKey(database), Provider: provider.Name(), Project: projectOption, DisplayName: name},
			Repository: repository, Application: application, Environment: environment, Name: name,
			CredentialKey: contract.SecretKey(credentialKey), Secret: contract.NewSecretValue(connectionString),
			Eligible: eligible, Detail: detail, SourcePaths: []string{sourcePath},
		}})
	}
}

func isAppSettingsFile(name string) bool {
	lowered := strings.ToLower(name)
	return lowered == "appsettings.json" || strings.HasPrefix(lowered, "appsettings.") && strings.HasSuffix(lowered, ".json")
}

func appSettingsEnvironment(path string) string {
	name := strings.TrimSuffix(filepath.Base(path), filepath.Ext(path))
	if environment, found := strings.CutPrefix(name, "appsettings."); found && strings.TrimSpace(environment) != "" {
		return environment
	}
	return "default"
}

func applicationName(repositoryRoot, path string) string {
	parent := filepath.Dir(path)
	relative, err := filepath.Rel(repositoryRoot, parent)
	if err != nil || relative == "." || relative == "" {
		return "root"
	}
	return sanitizeDiscoverySegment(strings.ReplaceAll(relative, string(filepath.Separator), "-"))
}

func isSQLServerConnectionString(value string) bool {
	normalized := strings.ToLower(strings.TrimSpace(value))
	if normalized == "" || strings.Contains(normalized, "${") || strings.Contains(normalized, "#{") || strings.Contains(normalized, "{{") || strings.HasPrefix(normalized, "http://") || strings.HasPrefix(normalized, "https://") {
		return false
	}
	hasServer := containsAnyDiscovery(normalized, "server=", "data source=", "address=", "addr=", "network address=")
	hasDatabase := containsAnyDiscovery(normalized, "database=", "initial catalog=")
	return hasServer && hasDatabase && strings.Contains(normalized, ";")
}

func generatedDatabaseKey(repository, application, environment, name string) string {
	return joinDiscoverySegments("-", "collected", repository, application, environment, name)
}

func generatedCredentialKey(project, repository, application, environment, name string) string {
	return joinDiscoverySegments(".", "db", "collected", "v1", project, repository, application, environment, name)
}

func joinDiscoverySegments(separator string, values ...string) string {
	for index := range values {
		values[index] = sanitizeDiscoverySegment(values[index])
	}
	return strings.Join(values, separator)
}

func sanitizeDiscoverySegment(value string) string {
	var output strings.Builder
	previousDash := false
	for _, character := range value {
		character = unicode.ToLower(character)
		if character <= unicode.MaxASCII && (character >= 'a' && character <= 'z' || character >= '0' && character <= '9') {
			output.WriteRune(character)
			previousDash = false
		} else if !previousDash && output.Len() > 0 {
			output.WriteByte('-')
			previousDash = true
		}
	}
	result := strings.Trim(output.String(), "-")
	if result == "" {
		return "default"
	}
	runes := []rune(result)
	if len(runes) > 48 {
		return string(runes[:48])
	}
	return result
}

func sortedDiscoveryKeys(values map[string]json.RawMessage) []string {
	keys := make([]string, 0, len(values))
	for key := range values {
		keys = append(keys, key)
	}
	sort.Strings(keys)
	return keys
}

func containsDiscoveryPath(values []string, target string) bool {
	for _, value := range values {
		if value == target {
			return true
		}
	}
	return false
}

func containsAnyDiscovery(value string, needles ...string) bool {
	for _, needle := range needles {
		if strings.Contains(value, needle) {
			return true
		}
	}
	return false
}

type localizedDiscoveryError string

func (err localizedDiscoveryError) Error() string { return string(err) }
func l10nError(id l10n.ID) error                  { return localizedDiscoveryError(l10n.Text(id)) }
