package dbcompat

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
	"github.com/sachahjkl/dw/internal/data/sqlserver"
	"github.com/sachahjkl/dw/internal/l10n"
)

const maxAppSettingsBytes int64 = 5 * 1024 * 1024

var skippedDirectories = map[string]struct{}{
	".git": {}, ".idea": {}, ".vs": {}, ".vscode": {}, "bin": {}, "dist": {},
	"node_modules": {}, "obj": {}, "out": {}, "target": {},
}

type ExecutionMode uint8

const (
	Preview ExecutionMode = iota
	Save
)

type Repository struct {
	Name string
	Root string
}

type Workspace struct {
	Path         string
	Project      string
	Repositories []Repository
}

type CollectStatus string

const (
	StatusEligible          CollectStatus = "eligible"
	StatusSaved             CollectStatus = "saved"
	StatusAlreadyConfigured CollectStatus = "already-configured"
	StatusSkipped           CollectStatus = "skipped"
	StatusConflict          CollectStatus = "conflict"
)

func (status CollectStatus) String() string {
	if status == StatusAlreadyConfigured {
		return l10n.Text("db.collect.status_already_configured")
	}
	return string(status)
}

type DatabaseCollectFinding struct {
	Project       string        `json:"project"`
	Repository    string        `json:"repository"`
	Application   string        `json:"application"`
	Environment   string        `json:"environment"`
	Name          string        `json:"name"`
	Database      string        `json:"database"`
	CredentialKey string        `json:"credentialKey"`
	Status        CollectStatus `json:"status"`
	Detail        *string       `json:"detail"`
	ValueMasked   bool          `json:"valueMasked"`
	SourcePaths   []string      `json:"sourcePaths"`
}

type DatabaseCollectReport struct {
	Root              string                   `json:"root"`
	SaveRequested     bool                     `json:"saveRequested"`
	ScannedWorkspaces int                      `json:"scannedWorkspaces"`
	ScannedFiles      int                      `json:"scannedFiles"`
	SavedCount        int                      `json:"savedCount"`
	Findings          []DatabaseCollectFinding `json:"findings"`
	Warnings          []string                 `json:"warnings"`
}

type candidate struct {
	finding DatabaseCollectFinding
	value   sqlserver.Secret
}

// CollectAppSettings discovers connection strings only from caller-resolved repository roots. The
// report contains masked metadata; plaintext remains confined to candidates and the secret store.
func CollectAppSettings(ctx context.Context, root string, workspaces []Workspace, mode ExecutionMode, store sqlserver.SecretStore) (DatabaseCollectReport, error) {
	candidates := make([]candidate, 0)
	identities := make(map[string]int)
	warnings := make([]string, 0)
	scannedFiles := 0
	for _, workspace := range workspaces {
		for _, repository := range workspace.Repositories {
			scanRepository(repository.Root, workspace.Project, repository.Name, &candidates, identities, &scannedFiles, &warnings)
		}
	}
	if mode == Save {
		if store == nil {
			return DatabaseCollectReport{}, localized("db.error.secret_store")
		}
		if err := saveCandidates(ctx, root, candidates, store); err != nil {
			return DatabaseCollectReport{}, err
		}
	}
	findings := make([]DatabaseCollectFinding, len(candidates))
	savedCount := 0
	for index := range candidates {
		findings[index] = candidates[index].finding
		if findings[index].Status == StatusSaved {
			savedCount++
		}
	}
	return DatabaseCollectReport{
		Root: root, SaveRequested: mode == Save, ScannedWorkspaces: len(workspaces), ScannedFiles: scannedFiles,
		SavedCount: savedCount, Findings: findings, Warnings: warnings,
	}, nil
}

func scanRepository(repositoryRoot, project, repository string, candidates *[]candidate, identities map[string]int, scannedFiles *int, warnings *[]string) {
	files := collectAppSettingsFiles(repositoryRoot, warnings)
	for _, file := range files {
		*scannedFiles++
		scanAppSettingsFile(repositoryRoot, file, project, repository, candidates, identities, warnings)
	}
}

func collectAppSettingsFiles(repositoryRoot string, warnings *[]string) []string {
	info, err := os.Stat(repositoryRoot)
	if err != nil || !info.IsDir() {
		if err == nil {
			err = localized("db.collect.not_directory")
		}
		*warnings = append(*warnings, l10n.Render(l10n.M("db.collect.scan_error", l10n.A("path", repositoryRoot), l10n.A("error", err))))
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
			if _, skipped := skippedDirectories[strings.ToLower(entry.Name())]; skipped {
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
		*warnings = append(*warnings, l10n.Render(l10n.M("db.collect.scan_error", l10n.A("path", repositoryRoot), l10n.A("error", err))))
		return nil
	}
	sort.Strings(files)
	return files
}

func scanAppSettingsFile(repositoryRoot, path, project, repository string, candidates *[]candidate, identities map[string]int, warnings *[]string) {
	info, err := os.Stat(path)
	if err == nil && info.Size() > maxAppSettingsBytes {
		*warnings = append(*warnings, l10n.Render(l10n.M("db.collect.oversized", l10n.A("path", path))))
		return
	}
	content, err := os.ReadFile(path)
	if err != nil {
		*warnings = append(*warnings, l10n.Render(l10n.M("db.collect.read_error", l10n.A("path", path), l10n.A("error", err))))
		return
	}
	content = bytes.TrimPrefix(content, []byte{0xef, 0xbb, 0xbf})
	var root map[string]json.RawMessage
	if err := json.Unmarshal(content, &root); err != nil {
		*warnings = append(*warnings, l10n.Render(l10n.M("db.collect.parse_error", l10n.A("path", path), l10n.A("error", err))))
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
	for _, name := range sortedKeys(connectionStrings) {
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
			existing := &(*candidates)[index]
			if secretsEqual(existing.value, sqlserver.NewSecret(connectionString)) {
				if !contains(existing.finding.SourcePaths, sourcePath) {
					existing.finding.SourcePaths = append(existing.finding.SourcePaths, sourcePath)
				}
			} else {
				existing.finding.Status = StatusConflict
				existing.finding.Detail = detail(l10n.Text("db.collect.different_values"))
				if !contains(existing.finding.SourcePaths, sourcePath) {
					existing.finding.SourcePaths = append(existing.finding.SourcePaths, sourcePath)
				}
			}
			continue
		}
		identities[identity] = len(*candidates)
		status := StatusEligible
		var findingDetail *string
		if !eligible {
			status = StatusSkipped
			findingDetail = detail(l10n.Text("db.collect.not_sqlserver"))
		}
		*candidates = append(*candidates, candidate{finding: DatabaseCollectFinding{
			Project: project, Repository: repository, Application: application, Environment: environment,
			Name: name, Database: database, CredentialKey: credentialKey, Status: status,
			Detail: findingDetail, ValueMasked: true, SourcePaths: []string{sourcePath},
		}, value: sqlserver.NewSecret(connectionString)})
	}
}

func saveCandidates(ctx context.Context, root string, candidates []candidate, store sqlserver.SecretStore) error {
	path := databasesPath(root)
	original, err := os.ReadFile(path)
	if err != nil {
		return localized("db.error.config_read", l10n.A("path", path), l10n.A("error", err))
	}
	var config map[string]any
	if err := json.Unmarshal(original, &config); err != nil {
		return localized("db.error.config_parse", l10n.A("path", path), l10n.A("error", err))
	}
	if config == nil {
		return localized("db.error.config_root_object")
	}
	newlyStored := make([]string, 0)
	configChanged := false
	rollback := func() {
		for _, key := range newlyStored {
			_, _ = store.Delete(ctx, contract.SecretKey(key))
		}
	}
	for index := range candidates {
		item := &candidates[index]
		if item.finding.Status != StatusEligible {
			continue
		}
		existing, exists := configuredDatabase(config, item.finding.Project, item.finding.Database)
		alreadyMatches := exists && stringValue(existing["provider"]) != "" && sqlserver.IsProviderName(stringValue(existing["provider"])) && stringValue(existing["credentialKey"]) == item.finding.CredentialKey
		if exists && !alreadyMatches {
			item.finding.Status = StatusConflict
			item.finding.Detail = detail(l10n.Text("db.collect.config_conflict"))
			continue
		}
		stored, found, getErr := store.Get(ctx, contract.SecretKey(item.finding.CredentialKey))
		if getErr != nil {
			rollback()
			return localized("db.collect.credential_read", l10n.A("key", item.finding.CredentialKey))
		}
		if found && !secretsEqual(stored, item.value) {
			item.finding.Status = StatusConflict
			item.finding.Detail = detail(l10n.Text("db.collect.credential_conflict"))
			continue
		}
		newSecret := false
		if !found {
			if err := store.Set(ctx, contract.SecretKey(item.finding.CredentialKey), item.value); err != nil {
				rollback()
				return localized("db.collect.credential_store", l10n.A("key", item.finding.CredentialKey))
			}
			newlyStored = append(newlyStored, item.finding.CredentialKey)
			newSecret = true
		}
		if !alreadyMatches {
			if err := insertDatabaseReference(config, item.finding.Project, item.finding.Database, item.finding.CredentialKey); err != nil {
				rollback()
				return err
			}
			configChanged = true
		}
		if alreadyMatches && !newSecret {
			item.finding.Status = StatusAlreadyConfigured
		} else {
			item.finding.Status = StatusSaved
		}
		item.finding.Detail = nil
	}
	if configChanged {
		current, err := os.ReadFile(path)
		if err != nil {
			rollback()
			return localized("db.error.config_reread", l10n.A("path", path), l10n.A("error", err))
		}
		if !bytes.Equal(current, original) {
			rollback()
			return localized("db.collect.concurrent_change")
		}
		encoded, err := json.MarshalIndent(config, "", "  ")
		if err != nil {
			rollback()
			return err
		}
		encoded = append(encoded, '\n')
		if err := atomicWriteFile(path, encoded); err != nil {
			rollback()
			return err
		}
	}
	return nil
}

func configuredDatabase(config map[string]any, project, database string) (map[string]any, bool) {
	projects, ok := config["projects"].(map[string]any)
	if !ok {
		return nil, false
	}
	projectNode, ok := projects[project].(map[string]any)
	if !ok {
		return nil, false
	}
	databases, ok := projectNode["databases"].(map[string]any)
	if !ok {
		return nil, false
	}
	value, ok := databases[database].(map[string]any)
	return value, ok
}

func insertDatabaseReference(config map[string]any, project, database, credentialKey string) error {
	projects, err := objectEntry(config, "projects")
	if err != nil {
		return err
	}
	projectValue, exists := projects[project]
	if !exists {
		projectValue = map[string]any{"databases": map[string]any{}}
		projects[project] = projectValue
	}
	projectNode, ok := projectValue.(map[string]any)
	if !ok {
		return localized("db.error.config_project_object", l10n.A("project", project))
	}
	databases, err := objectEntry(projectNode, "databases")
	if err != nil {
		return err
	}
	databases[database] = map[string]any{"provider": sqlserver.LegacyProviderName, "credentialKey": credentialKey, "readonly": true}
	return nil
}

func objectEntry(object map[string]any, key string) (map[string]any, error) {
	value, exists := object[key]
	if !exists {
		created := make(map[string]any)
		object[key] = created
		return created, nil
	}
	entry, ok := value.(map[string]any)
	if !ok {
		return nil, localized("db.error.config_section_object", l10n.A("section", key))
	}
	return entry, nil
}

func atomicWriteFile(path string, content []byte) error {
	parent := filepath.Dir(path)
	temporary, err := os.CreateTemp(parent, ".databases-*.json")
	if err != nil {
		return err
	}
	temporaryPath := temporary.Name()
	keep := false
	defer func() {
		_ = temporary.Close()
		if !keep {
			_ = os.Remove(temporaryPath)
		}
	}()
	if info, statErr := os.Stat(path); statErr == nil {
		_ = temporary.Chmod(info.Mode().Perm())
	}
	if _, err := temporary.Write(content); err != nil {
		return err
	}
	if err := temporary.Sync(); err != nil {
		return err
	}
	if err := temporary.Close(); err != nil {
		return err
	}
	if err := replaceFileAtomic(temporaryPath, path); err != nil {
		return err
	}
	keep = true
	return nil
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
	return sanitizeSegment(strings.ReplaceAll(relative, string(filepath.Separator), "-"))
}

func isSQLServerConnectionString(value string) bool {
	normalized := strings.ToLower(strings.TrimSpace(value))
	if normalized == "" || strings.Contains(normalized, "${") || strings.Contains(normalized, "#{") || strings.Contains(normalized, "{{") || strings.HasPrefix(normalized, "http://") || strings.HasPrefix(normalized, "https://") {
		return false
	}
	hasServer := containsAny(normalized, "server=", "data source=", "address=", "addr=", "network address=")
	hasDatabase := containsAny(normalized, "database=", "initial catalog=")
	return hasServer && hasDatabase && strings.Contains(normalized, ";")
}

func generatedDatabaseKey(repository, application, environment, name string) string {
	return joinSanitized("-", "collected", repository, application, environment, name)
}

func generatedCredentialKey(project, repository, application, environment, name string) string {
	return joinSanitized(".", "db", "collected", "v1", project, repository, application, environment, name)
}

func joinSanitized(separator string, values ...string) string {
	for index := range values {
		values[index] = sanitizeSegment(values[index])
	}
	return strings.Join(values, separator)
}

func sanitizeSegment(value string) string {
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

func detail(value string) *string  { return &value }
func stringValue(value any) string { text, _ := value.(string); return text }
func contains(values []string, target string) bool {
	for _, value := range values {
		if value == target {
			return true
		}
	}
	return false
}
func containsAny(value string, needles ...string) bool {
	for _, needle := range needles {
		if strings.Contains(value, needle) {
			return true
		}
	}
	return false
}

// Secret equality is intentionally delegated to the opaque type's stable masked JSON only inside
// this package via EqualSecrets; no plaintext is projected.
func secretsEqual(left, right sqlserver.Secret) bool { return sqlserver.EqualSecrets(left, right) }

func databasesPath(root string) string { return filepath.Join(root, "config", "databases.json") }
