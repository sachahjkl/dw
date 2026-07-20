package workspace

import (
	"bytes"
	"encoding/json"
	"errors"
	"io/fs"
	"os"
	"path/filepath"
	"sort"
	"strings"

	"github.com/sachahjkl/dw/internal/l10n"
)

var manifestKeys = []string{"schema", "workItemId", "taskId", "project", "type", "slug", "branchName", "createdAt", "repositories", "status", "workItemType", "workItemTitle", "workItemState", "childTaskIds", "childTasks", "workItems"}

func (m *Manifest) UnmarshalJSON(data []byte) error {
	type plain Manifest
	var decoded plain
	if err := json.Unmarshal(data, &decoded); err != nil {
		return err
	}
	var raw map[string]json.RawMessage
	if err := json.Unmarshal(data, &raw); err != nil {
		return err
	}
	for _, key := range manifestKeys {
		delete(raw, key)
	}
	*m = Manifest(decoded)
	m.Unknown = raw
	return nil
}

func (m Manifest) MarshalJSON() ([]byte, error) {
	values := []any{m.Schema, m.WorkItemID, m.TaskID, m.Project, m.Type, m.Slug, m.BranchName, m.CreatedAt, m.Repositories, m.Status, m.WorkItemType, m.WorkItemTitle, m.WorkItemState, m.ChildTaskIDs, m.ChildTasks, m.WorkItems}
	var b bytes.Buffer
	b.WriteByte('{')
	first := true
	write := func(key string, value []byte) {
		if !first {
			b.WriteByte(',')
		}
		first = false
		encodedKey, _ := json.Marshal(key)
		b.Write(encodedKey)
		b.WriteByte(':')
		b.Write(value)
	}
	for index, key := range manifestKeys {
		encoded, err := json.Marshal(values[index])
		if err != nil {
			return nil, err
		}
		write(key, encoded)
	}
	unknownKeys := make([]string, 0, len(m.Unknown))
	for key := range m.Unknown {
		unknownKeys = append(unknownKeys, key)
	}
	sort.Strings(unknownKeys)
	for _, key := range unknownKeys {
		if len(m.Unknown[key]) == 0 {
			continue
		}
		write(key, m.Unknown[key])
	}
	b.WriteByte('}')
	return b.Bytes(), nil
}

func ReadManifest(path string) (Manifest, error) {
	data, err := os.ReadFile(path)
	if err != nil {
		return Manifest{}, localizedCause("workspace.error.invalid-manifest", ErrInvalidManifest, l10n.A("path", path))
	}
	var manifest Manifest
	if err = json.Unmarshal(data, &manifest); err != nil {
		return Manifest{}, localizedCause("workspace.error.invalid-manifest", errors.Join(ErrInvalidManifest, err), l10n.A("path", path))
	}
	if strings.TrimSpace(manifest.WorkItemID) == "" || strings.TrimSpace(manifest.Project) == "" {
		return Manifest{}, localizedCause("workspace.error.invalid-manifest", ErrInvalidManifest, l10n.A("path", path))
	}
	manifest.Normalize()
	return manifest, nil
}

func (m *Manifest) Normalize() {
	m.Repositories = distinctCSV(m.Repositories)
	if m.Schema == 0 {
		m.Schema = 1
	}
	if strings.TrimSpace(m.Status) == "" {
		m.Status = "created"
	}
	if strings.TrimSpace(m.Type) == "" {
		m.Type = "feat"
	}
	if m.WorkItems != nil {
		m.WorkItems = distinctWorkItems(m.WorkItems)
	}
	if m.ChildTasks != nil {
		m.ChildTasks = distinctChildTasks(m.ChildTasks)
	}
}

func WriteManifest(path string, manifest Manifest) error {
	manifest.Normalize()
	data, err := json.MarshalIndent(manifest, "", "  ")
	if err != nil {
		return localizedCause("workspace.error.invalid-manifest", errors.Join(ErrInvalidManifest, err), l10n.A("path", path))
	}
	data = append(bytes.TrimRight(data, "\n"), '\n')
	return writeFileAtomic(path, data, 0o644)
}

func writeFileAtomic(path string, data []byte, mode fs.FileMode) error {
	if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
		return err
	}
	file, err := os.CreateTemp(filepath.Dir(path), ".dw-*")
	if err != nil {
		return err
	}
	temporary := file.Name()
	clean := func() { _ = os.Remove(temporary) }
	defer clean()
	if err = file.Chmod(mode); err != nil {
		_ = file.Close()
		return err
	}
	if _, err = file.Write(data); err != nil {
		_ = file.Close()
		return err
	}
	if err = file.Sync(); err != nil {
		_ = file.Close()
		return err
	}
	if err = file.Close(); err != nil {
		return err
	}
	if err = os.Rename(temporary, path); err == nil {
		return nil
	}
	backup := path + ".dw-backup"
	if _, statErr := os.Stat(path); statErr == nil {
		_ = os.Remove(backup)
		if renameErr := os.Rename(path, backup); renameErr != nil {
			return err
		}
		if renameErr := os.Rename(temporary, path); renameErr != nil {
			_ = os.Rename(backup, path)
			return renameErr
		}
		_ = os.Remove(backup)
		return nil
	}
	return err
}

func FindWorkspacePath(start string) (string, bool) {
	path, err := filepath.Abs(start)
	if err != nil {
		return "", false
	}
	for {
		if info, err := os.Stat(filepath.Join(path, ManifestFile)); err == nil && !info.IsDir() {
			return path, true
		}
		parent := filepath.Dir(path)
		if parent == path {
			return "", false
		}
		path = parent
	}
}

func Discover(root string) []Summary {
	projects, err := os.ReadDir(filepath.Join(root, "projects"))
	if err != nil {
		return []Summary{}
	}
	result := make([]Summary, 0)
	for _, project := range projects {
		if !project.IsDir() {
			continue
		}
		workspaces, err := os.ReadDir(filepath.Join(root, "projects", project.Name(), "workspaces"))
		if err != nil {
			continue
		}
		for _, entry := range workspaces {
			if !entry.IsDir() {
				continue
			}
			path := filepath.Join(root, "projects", project.Name(), "workspaces", entry.Name())
			manifest, err := ReadManifest(filepath.Join(path, ManifestFile))
			if err == nil {
				result = append(result, Summary{Path: path, Manifest: manifest})
			}
		}
	}
	sort.SliceStable(result, func(i, j int) bool {
		if result[i].Manifest.CreatedAt == result[j].Manifest.CreatedAt {
			return result[i].Path < result[j].Path
		}
		return result[i].Manifest.CreatedAt > result[j].Manifest.CreatedAt
	})
	return result
}

func Filter(workspaces []Summary, project string, requestedIDs []string) []Summary {
	ids := distinctCSV(requestedIDs)
	result := make([]Summary, 0, len(workspaces))
	for _, workspace := range workspaces {
		if project != "" && !equalFold(workspace.Manifest.Project, project) {
			continue
		}
		matches := true
		for _, id := range ids {
			if !workspace.Manifest.MatchesWorkItem(id) {
				matches = false
				break
			}
		}
		if matches {
			result = append(result, workspace)
		}
	}
	return result
}

func List(root, project string, workItemIDs []string) []ListItem {
	workspaces := Filter(Discover(root), project, workItemIDs)
	result := make([]ListItem, 0, len(workspaces))
	for _, workspace := range workspaces {
		manifest := workspace.Manifest
		result = append(result, ListItem{Path: workspace.Path, Project: manifest.Project, WorkItemID: manifest.PrimaryWorkItemID(), WorkItems: manifest.ParentWorkItems(), TaskID: manifest.TaskID, AllKnownWorkItemIDs: manifest.AllKnownWorkItemIDs(), Type: manifest.Type, Slug: manifest.Slug, BranchName: manifest.BranchName, CreatedAt: manifest.CreatedAt, WorkItemType: manifest.WorkItemType, WorkItemTitle: manifest.WorkItemTitle, WorkItemState: manifest.WorkItemState, Repositories: append([]string(nil), manifest.Repositories...)})
	}
	return result
}

func Current(start string) (CurrentItem, error) {
	path, ok := FindWorkspacePath(start)
	if !ok {
		return CurrentItem{}, ErrNoCurrentWorkspace
	}
	manifest, err := ReadManifest(filepath.Join(path, ManifestFile))
	if err != nil {
		return CurrentItem{}, err
	}
	return CurrentItem{Workspace: path, Project: manifest.Project, PrimaryWorkItemID: manifest.PrimaryWorkItemID(), WorkItems: manifest.ParentWorkItems(), TaskID: manifest.TaskID, ChildTaskIDs: manifest.ChildTaskIDsByRepository(), ChildTasks: manifest.NormalizedChildTasks(), Branch: manifest.BranchName, Repositories: append([]string(nil), manifest.Repositories...)}, nil
}

func Resolve(root, explicit, project string, ids []string, useLatest bool, currentDirectory string) (string, error) {
	if strings.TrimSpace(explicit) != "" {
		return filepath.Clean(explicit), nil
	}
	if !useLatest {
		if path, ok := FindWorkspacePath(currentDirectory); ok {
			return path, nil
		}
		return "", ErrNoCurrentWorkspace
	}
	matches := Filter(Discover(root), project, ids)
	if len(matches) == 0 {
		return "", ErrNoWorkspace
	}
	return matches[0].Path, nil
}

func (m Manifest) ParentWorkItems() []WorkItem {
	items := distinctWorkItems(m.WorkItems)
	if len(items) == 0 {
		return []WorkItem{{ID: m.WorkItemID, Type: cloneString(m.WorkItemType), Title: cloneString(m.WorkItemTitle), State: cloneString(m.WorkItemState)}}
	}
	index := -1
	for i := range items {
		if equalFold(items[i].ID, m.WorkItemID) {
			index = i
			break
		}
	}
	if index < 0 {
		return append([]WorkItem{{ID: m.WorkItemID, Type: cloneString(m.WorkItemType), Title: cloneString(m.WorkItemTitle), State: cloneString(m.WorkItemState)}}, items...)
	}
	if index > 0 {
		primary := items[index]
		copy(items[1:index+1], items[0:index])
		items[0] = primary
	}
	return items
}
func (m Manifest) PrimaryWorkItemID() string {
	items := m.ParentWorkItems()
	if len(items) == 0 {
		return m.WorkItemID
	}
	return items[0].ID
}
func (m Manifest) NormalizedChildTasks() []ChildTask {
	items := distinctChildTasks(m.ChildTasks)
	keys := make([]string, 0, len(m.ChildTaskIDs))
	for key := range m.ChildTaskIDs {
		keys = append(keys, key)
	}
	sort.Strings(keys)
	for _, repository := range keys {
		id := m.ChildTaskIDs[repository]
		if strings.TrimSpace(repository) == "" || strings.TrimSpace(id) == "" {
			continue
		}
		found := false
		for _, item := range items {
			if equalFold(item.ID, id) {
				found = true
				break
			}
		}
		if !found {
			items = append(items, ChildTask{Repository: repository, ID: id})
		}
	}
	return items
}
func (m Manifest) ChildTaskIDsByRepository() map[string]string {
	result := make(map[string]string)
	for _, task := range m.NormalizedChildTasks() {
		found := false
		for key := range result {
			if equalFold(key, task.Repository) {
				found = true
				break
			}
		}
		if !found {
			result[task.Repository] = task.ID
		}
	}
	return result
}
func (m Manifest) AllKnownWorkItemIDs() []string {
	ids := make([]string, 0)
	for _, item := range m.ParentWorkItems() {
		ids = appendDistinct(ids, item.ID)
	}
	if m.TaskID != nil {
		ids = appendDistinct(ids, *m.TaskID)
	}
	for _, task := range m.NormalizedChildTasks() {
		ids = appendDistinct(ids, task.ID)
	}
	return ids
}
func (m Manifest) MatchesWorkItem(id string) bool {
	for _, known := range m.AllKnownWorkItemIDs() {
		if equalFold(known, id) {
			return true
		}
	}
	return false
}

func ParseWorkItemIDs(value string) []string {
	items := strings.Split(value, ",")
	result := make([]string, 0, len(items))
	for _, item := range items {
		item = strings.TrimSpace(item)
		if item != "" {
			result = append(result, item)
		}
	}
	sort.Strings(result)
	return distinctExact(result)
}
func IsFinalState(itemType, state *string) bool {
	s := normalizeState(valueOrEmpty(state))
	if s == "" {
		return false
	}
	kind := normalizeState(valueOrEmpty(itemType))
	final := s == "valide" || s == "cloture" || s == "abandonne"
	if kind == "bug" || kind == "activite" {
		return s == "cloture" || s == "abandonne"
	}
	return final
}
func PruneCandidates(root, project string, ids []string) []Summary {
	items := Filter(Discover(root), project, ids)
	result := make([]Summary, 0)
	for _, item := range items {
		final := true
		for _, workItem := range item.Manifest.ParentWorkItems() {
			if !IsFinalState(workItem.Type, workItem.State) {
				final = false
				break
			}
		}
		if final {
			result = append(result, item)
		}
	}
	return result
}

func distinctCSV(values []string) []string {
	result := make([]string, 0, len(values))
	for _, value := range values {
		for _, part := range strings.Split(value, ",") {
			part = strings.TrimSpace(part)
			if part != "" {
				result = appendDistinct(result, part)
			}
		}
	}
	return result
}
func distinctWorkItems(values []WorkItem) []WorkItem {
	result := make([]WorkItem, 0, len(values))
	for _, value := range values {
		if strings.TrimSpace(value.ID) == "" {
			continue
		}
		seen := false
		for _, item := range result {
			if equalFold(item.ID, value.ID) {
				seen = true
				break
			}
		}
		if !seen {
			result = append(result, value)
		}
	}
	return result
}
func distinctChildTasks(values []ChildTask) []ChildTask {
	result := make([]ChildTask, 0, len(values))
	for _, value := range values {
		if strings.TrimSpace(value.ID) == "" || strings.TrimSpace(value.Repository) == "" {
			continue
		}
		seen := false
		for _, item := range result {
			if equalFold(item.ID, value.ID) {
				seen = true
				break
			}
		}
		if !seen {
			result = append(result, value)
		}
	}
	return result
}
func appendDistinct(values []string, value string) []string {
	if strings.TrimSpace(value) == "" {
		return values
	}
	for _, item := range values {
		if equalFold(item, value) {
			return values
		}
	}
	return append(values, value)
}
func distinctExact(values []string) []string {
	result := values[:0]
	for _, v := range values {
		if len(result) == 0 || result[len(result)-1] != v {
			result = append(result, v)
		}
	}
	return result
}
func equalFold(a, b string) bool { return strings.EqualFold(a, b) }
func cloneString(value *string) *string {
	if value == nil {
		return nil
	}
	copy := *value
	return &copy
}
func valueOrEmpty(value *string) string {
	if value == nil {
		return ""
	}
	return *value
}
func normalizeState(value string) string {
	r := strings.NewReplacer("é", "e", "è", "e", "ê", "e", "à", "a", "â", "a", "ô", "o", "û", "u", "ù", "u")
	return r.Replace(strings.ToLower(strings.TrimSpace(value)))
}
