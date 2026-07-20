package dataapp

import (
	"bytes"
	"context"
	"encoding/json"
	"os"
	"path/filepath"
	"strings"

	"github.com/sachahjkl/dw/internal/contract"
	"github.com/sachahjkl/dw/internal/data"
	"github.com/sachahjkl/dw/internal/l10n"
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
		return l10n.Text("data.collect.status_already_configured")
	}
	return string(status)
}

type DataSourceCollectFinding struct {
	Project       string        `json:"project"`
	Provider      string        `json:"provider"`
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

type DataSourceCollectReport struct {
	Root              string                     `json:"root"`
	SaveRequested     bool                       `json:"saveRequested"`
	ScannedWorkspaces int                        `json:"scannedWorkspaces"`
	ScannedFiles      int                        `json:"scannedFiles"`
	SavedCount        int                        `json:"savedCount"`
	Findings          []DataSourceCollectFinding `json:"findings"`
	Warnings          []string                   `json:"warnings"`
}

type candidate struct {
	finding DataSourceCollectFinding
	value   contract.SecretValue
}

func collectReport(root string, workspaceCount int, discovered data.DiscoveryReport, save bool) (DataSourceCollectReport, []candidate) {
	candidates := make([]candidate, len(discovered.Sources))
	for index, source := range discovered.Sources {
		project := ""
		if configured, found := source.Source.Project.Get(); found {
			project = string(configured)
		}
		status := StatusEligible
		if !source.Eligible {
			status = StatusSkipped
		}
		var findingDetail *string
		if source.Detail != "" {
			findingDetail = detail(source.Detail)
		}
		candidates[index] = candidate{finding: DataSourceCollectFinding{
			Project: project, Provider: string(source.Source.Provider), Repository: source.Repository,
			Application: source.Application, Environment: source.Environment, Name: source.Name,
			Database: string(source.Source.Key), CredentialKey: string(source.CredentialKey), Status: status,
			Detail: findingDetail, ValueMasked: !source.Secret.Empty(), SourcePaths: append([]string(nil), source.SourcePaths...),
		}, value: source.Secret}
	}
	return DataSourceCollectReport{
		Root: root, SaveRequested: save, ScannedWorkspaces: workspaceCount, ScannedFiles: discovered.ScannedFiles,
		Findings: make([]DataSourceCollectFinding, len(candidates)), Warnings: append([]string{}, discovered.Warnings...),
	}, candidates
}

func finishCollectReport(report DataSourceCollectReport, candidates []candidate) DataSourceCollectReport {
	report.SavedCount = 0
	for index := range candidates {
		report.Findings[index] = candidates[index].finding
		if report.Findings[index].Status == StatusSaved {
			report.SavedCount++
		}
	}
	return report
}

func saveCandidates(ctx context.Context, root string, candidates []candidate, store contract.SecretStore) error {
	path := databasesPath(root)
	original, err := os.ReadFile(path)
	if err != nil {
		return localized("data.error.config_read", l10n.A("path", path), l10n.A("error", err))
	}
	var config map[string]any
	if err := json.Unmarshal(original, &config); err != nil {
		return localized("data.error.config_parse", l10n.A("path", path), l10n.A("error", err))
	}
	if config == nil {
		return localized("data.error.config_root_object")
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
		alreadyMatches := exists && strings.TrimSpace(stringValue(existing["provider"])) == item.finding.Provider && stringValue(existing["credentialKey"]) == item.finding.CredentialKey
		if exists && !alreadyMatches {
			item.finding.Status = StatusConflict
			item.finding.Detail = detail(l10n.Text("data.collect.config_conflict"))
			continue
		}
		stored, found, getErr := store.Get(ctx, contract.SecretKey(item.finding.CredentialKey))
		if getErr != nil {
			rollback()
			return localized("data.collect.credential_read", l10n.A("key", item.finding.CredentialKey))
		}
		if found && !secretsEqual(stored, item.value) {
			item.finding.Status = StatusConflict
			item.finding.Detail = detail(l10n.Text("data.collect.credential_conflict"))
			continue
		}
		newSecret := false
		if !found {
			if err := store.Set(ctx, contract.SecretKey(item.finding.CredentialKey), item.value); err != nil {
				rollback()
				return localized("data.collect.credential_store", l10n.A("key", item.finding.CredentialKey))
			}
			newlyStored = append(newlyStored, item.finding.CredentialKey)
			newSecret = true
		}
		if !alreadyMatches {
			if err := insertDatabaseReference(config, item.finding.Project, item.finding.Database, item.finding.Provider, item.finding.CredentialKey); err != nil {
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
			return localized("data.error.config_reread", l10n.A("path", path), l10n.A("error", err))
		}
		if !bytes.Equal(current, original) {
			rollback()
			return localized("data.collect.concurrent_change")
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

func insertDatabaseReference(config map[string]any, project, database, provider, credentialKey string) error {
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
		return localized("data.error.config_project_object", l10n.A("project", project))
	}
	databases, err := objectEntry(projectNode, "databases")
	if err != nil {
		return err
	}
	databases[database] = map[string]any{"provider": provider, "credentialKey": credentialKey, "readonly": true}
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
		return nil, localized("data.error.config_section_object", l10n.A("section", key))
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

func detail(value string) *string  { return &value }
func stringValue(value any) string { text, _ := value.(string); return text }
func secretsEqual(left, right contract.SecretValue) bool {
	return left.Reveal() == right.Reveal()
}
func databasesPath(root string) string { return filepath.Join(root, "config", "databases.json") }
