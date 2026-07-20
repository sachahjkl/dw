package config

import (
	"os"
	"path/filepath"
	"strings"

	"github.com/sachahjkl/dw/internal/gitrepo"
	"github.com/sachahjkl/dw/internal/l10n"
	"github.com/sachahjkl/dw/internal/wirejson"
)

func InitRoot(request InitRequest) (InitReport, error) {
	root := normalizeRoot(request.Root)
	profile, err := resolveProfile(request.Profile)
	if err != nil {
		return InitReport{}, err
	}
	report := InitReport{
		Root: root, Profile: profile.name, DryRun: request.DryRun,
		NoSave: request.NoSave, PlannedPaths: PlannedPaths(root),
	}
	if request.DryRun {
		return report, nil
	}
	if err = createDirectories(root); err != nil {
		return InitReport{}, err
	}
	if err = writeSchemas(root, false); err != nil {
		return InitReport{}, err
	}
	for _, file := range profileFiles(root, profile) {
		if err = writeFile(file.path, file.content, false); err != nil {
			return InitReport{}, err
		}
	}
	if !request.NoSave {
		settings := UserSettings{Root: &root, Color: nil}
		if err = SaveUserSettings(settings); err != nil {
			return InitReport{}, err
		}
	}
	return report, nil
}

func RefreshRoot(request RefreshRequest) (RefreshReport, error) {
	root := normalizeRoot(request.Root)
	info, err := os.Stat(root)
	if err != nil || !info.IsDir() {
		return RefreshReport{}, localizedError(l10n.M("config.root_not_found", l10n.A("root", root)))
	}
	profile := detectProfile(root)
	if request.Profile != nil && strings.TrimSpace(*request.Profile) != "" {
		profile, err = resolveProfile(*request.Profile)
		if err != nil {
			return RefreshReport{}, err
		}
	}
	if err = createDirectories(root); err != nil {
		return RefreshReport{}, err
	}
	if err = writeSchemas(root, true); err != nil {
		return RefreshReport{}, err
	}
	if err = MigrateProjectURLs(root); err != nil {
		return RefreshReport{}, err
	}
	if err = SyncBareRepositoryRemotes(root); err != nil {
		return RefreshReport{}, err
	}
	for _, file := range agentFiles(root, profile) {
		if err = writeFile(file.path, file.content, true); err != nil {
			return RefreshReport{}, err
		}
	}
	return RefreshReport{Root: root, Profile: profile.name}, nil
}

func PlannedPaths(root string) []string {
	parts := [][]string{
		{}, {"config"}, {"config", "projects.json"}, {"config", "workflow.json"},
		{"config", "databases.json"}, {"config", "opencode", "AGENTS.md"},
		{"config", "opencode", "opencode.jsonc"}, {"config", "claude", "CLAUDE.md"},
		{"config", "cursor", "devworkflow.mdc"}, {"config", "codex", "AGENTS.md"},
		{"config", "codex", "config.toml"}, {"config", "copilot", "copilot-instructions.md"},
		{"projects"}, {"cache"}, {"schemas"}, {"schemas", "projects.schema.json"},
		{"schemas", "workflow.schema.json"}, {"schemas", "databases.schema.json"},
		{"schemas", "release.schema.json"},
	}
	paths := make([]string, len(parts))
	for index, pathParts := range parts {
		paths[index] = joinRoot(root, pathParts...)
	}
	return paths
}

func MigrateProjectURLs(root string) error {
	path := filepath.Join(root, "config", "projects.json")
	data, err := os.ReadFile(path)
	if err != nil {
		return nil
	}
	document, err := wirejson.Parse(data)
	if err != nil {
		return nil
	}
	projects, ok := document.Lookup("projects")
	if !ok || projects.Kind() != wirejson.Object {
		return nil
	}
	changed := false
	projectMembers, _ := effectiveMembers(projects)
	for _, projectMember := range projectMembers {
		project, _ := projects.Lookup(projectMember.Name)
		repositories, exists := project.Lookup("repositories")
		if !exists || repositories.Kind() != wirejson.Object {
			continue
		}
		repositoryMembers, _ := effectiveMembers(repositories)
		for _, repositoryMember := range repositoryMembers {
			repository, _ := repositories.Lookup(repositoryMember.Name)
			if repository.Kind() != wirejson.Object {
				continue
			}
			url, exists := repository.Lookup("url")
			if !exists {
				continue
			}
			httpURL, isString := url.AsString()
			if !isString {
				continue
			}
			members := []wirejson.Member{{Name: "http", Value: wirejson.StringValue(httpURL)}}
			if sshURL, convert := repositorySSHURLForHTTP(httpURL); convert {
				members = append(members, wirejson.Member{Name: "ssh", Value: wirejson.StringValue(sshURL)})
			}
			if err = repository.Set("url", wirejson.ObjectValue(members...)); err != nil {
				return err
			}
			changed = true
		}
	}
	if !changed {
		return nil
	}
	content, err := wirejson.Pretty(document)
	if err != nil {
		return err
	}
	content = append(content, '\n')
	return os.WriteFile(path, content, 0o644)
}

func SyncBareRepositoryRemotes(root string) error {
	projects := LoadProjectsConfig(root)
	for _, project := range projects.Projects {
		for _, repository := range project.Project.Repositories {
			anchor := repository.Key + ".git"
			if repository.Repository.AnchorName != nil && strings.TrimSpace(*repository.Repository.AnchorName) != "" {
				anchor = *repository.Repository.AnchorName
			}
			repositoryPath := filepath.Join(root, "projects", project.Key, "repositories", anchor)
			info, err := os.Stat(repositoryPath)
			if err != nil || !info.IsDir() {
				continue
			}
			httpURL := strings.TrimSpace(repository.Repository.URL.HTTP)
			sshURL := repository.Repository.URL.SSH
			// Refresh historically ignores individual Git failures so one stale or
			// non-Git anchor cannot prevent schema and agent refresh.
			_ = gitrepo.ConfigureRemotes(repositoryPath, httpURL, sshURL)
		}
	}
	return nil
}

type generatedFile struct{ path, content string }

func profileFiles(root string, profile initProfile) []generatedFile {
	return append([]generatedFile{
		{joinRoot(root, "config", "projects.json"), profile.projectsJSON},
		{joinRoot(root, "config", "workflow.json"), profile.workflowJSON},
		{joinRoot(root, "config", "databases.json"), profile.databasesJSON},
	}, agentFiles(root, profile)...)
}

func agentFiles(root string, profile initProfile) []generatedFile {
	return []generatedFile{
		{joinRoot(root, "config", "opencode", "AGENTS.md"), profile.agentsMD},
		{joinRoot(root, "config", "opencode", "opencode.jsonc"), profile.opencodeJSONC},
		{joinRoot(root, "config", "claude", "CLAUDE.md"), profile.agentsMD},
		{joinRoot(root, "config", "cursor", "devworkflow.mdc"), profile.agentsMD},
		{joinRoot(root, "config", "codex", "AGENTS.md"), profile.agentsMD},
		{joinRoot(root, "config", "codex", "config.toml"), workspaceCodexConfig},
		{joinRoot(root, "config", "copilot", "copilot-instructions.md"), profile.agentsMD},
	}
}

func createDirectories(root string) error {
	for _, parts := range [][]string{{}, {"config"}, {"config", "opencode"}, {"config", "claude"}, {"config", "cursor"}, {"config", "codex"}, {"config", "copilot"}, {"projects"}, {"cache"}} {
		if err := os.MkdirAll(joinRoot(root, parts...), 0o755); err != nil {
			return err
		}
	}
	return nil
}

func writeSchemas(root string, overwrite bool) error {
	if err := os.MkdirAll(filepath.Join(root, "schemas"), 0o755); err != nil {
		return err
	}
	for _, name := range []string{"projects.schema.json", "workflow.schema.json", "databases.schema.json", "release.schema.json"} {
		content, err := schemaResources.ReadFile("resources/" + name)
		if err != nil {
			return err
		}
		if err = writeFile(filepath.Join(root, "schemas", name), string(content), overwrite); err != nil {
			return err
		}
	}
	return nil
}

func writeFile(path, content string, overwrite bool) error {
	if !overwrite {
		if _, err := os.Stat(path); err == nil {
			return nil
		}
	}
	if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
		return err
	}
	return os.WriteFile(path, []byte(content), 0o644)
}

func joinRoot(root string, parts ...string) string {
	values := make([]string, 1, len(parts)+1)
	values[0] = root
	values = append(values, parts...)
	return filepath.Join(values...)
}

func normalizeRoot(value string) string {
	if strings.TrimSpace(value) == "" {
		value = DefaultRoot()
	}
	return NormalizePathLossy(value)
}
