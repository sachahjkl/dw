package paritytest_test

import (
	"encoding/json"
	"os"
	"path/filepath"
	"reflect"
	"testing"

	"github.com/sachahjkl/dw/internal/config"
	"github.com/sachahjkl/dw/internal/wirejson"
)

func TestConfigInitCreatesExactPlannedFilesystem(t *testing.T) {
	root := filepath.Join(t.TempDir(), "root")
	report, err := config.InitRoot(config.InitRequest{Root: root, Profile: "default", NoSave: true})
	if err != nil {
		t.Fatal(err)
	}
	if report.Root != root || report.Profile != "default" || !report.NoSave || report.DryRun {
		t.Fatalf("init report = %#v", report)
	}
	if got, want := report.PlannedPaths, config.PlannedPaths(root); !reflect.DeepEqual(got, want) {
		t.Fatalf("planned paths = %#v, want %#v", got, want)
	}
	for _, path := range report.PlannedPaths {
		if _, err := os.Stat(path); err != nil {
			t.Errorf("planned path does not exist %q: %v", path, err)
		}
	}
	fixture, err := os.ReadFile(filepath.Join("..", "..", "testdata", "contract", "config-init-paths.json"))
	if err != nil {
		t.Fatal(err)
	}
	var requiredPaths []string
	if err := json.Unmarshal(fixture, &requiredPaths); err != nil {
		t.Fatal(err)
	}
	for _, relative := range requiredPaths {
		path := filepath.Join(root, filepath.FromSlash(relative))
		info, err := os.Stat(path)
		if err != nil {
			t.Errorf("inspect %s: %v", relative, err)
			continue
		}
		if info.Mode().IsRegular() {
			contents, err := os.ReadFile(path)
			if err != nil {
				t.Errorf("read %s: %v", relative, err)
				continue
			}
			if len(contents) == 0 {
				t.Errorf("generated file %s is empty", relative)
			}
		}
	}
}

func TestConfigInitDryRunDoesNotTouchFilesystem(t *testing.T) {
	root := filepath.Join(t.TempDir(), "not-created")
	report, err := config.InitRoot(config.InitRequest{Root: root, Profile: "default", NoSave: true, DryRun: true})
	if err != nil {
		t.Fatal(err)
	}
	if !report.DryRun || len(report.PlannedPaths) == 0 {
		t.Fatalf("dry-run report = %#v", report)
	}
	if _, err := os.Stat(root); !os.IsNotExist(err) {
		t.Fatalf("dry run created root: %v", err)
	}
}

func TestConfigRefreshMigratesLegacyURLWithoutLosingOrderOrExtensions(t *testing.T) {
	root := filepath.Join(t.TempDir(), "root")
	if _, err := config.InitRoot(config.InitRequest{Root: root, Profile: "default", NoSave: true}); err != nil {
		t.Fatal(err)
	}
	projectsPath := filepath.Join(root, "config", "projects.json")
	legacy := []byte(`{
  "$schema": "../../schemas/projects.schema.json",
  "extension": {"order":["z","a"],"enabled":true},
  "projects": {
    "acme": {
      "displayName": "Acme",
      "workProvider": "github",
      "providers": {
        "github": {"endpoint":"https://api.github.example","extension":{"order":["z","a"]}}
      },
      "repositories": {
        "front": {
		  "url": "https://github.com/torvalds/linux.git",
          "defaultBranch": "main",
          "providerRepository": "platform/front",
          "extension": "keep"
        }
      }
    }
  }
}
`)
	if err := os.WriteFile(projectsPath, legacy, 0o644); err != nil {
		t.Fatal(err)
	}
	profile := "default"
	report, err := config.RefreshRoot(config.RefreshRequest{Root: root, Profile: &profile})
	if err != nil {
		t.Fatal(err)
	}
	if report.Root != root || report.Profile != profile {
		t.Fatalf("refresh report = %#v", report)
	}
	contents, err := os.ReadFile(projectsPath)
	if err != nil {
		t.Fatal(err)
	}
	document, err := wirejson.Parse(contents)
	if err != nil {
		t.Fatal(err)
	}
	members, _ := document.Members()
	gotRootOrder := make([]string, len(members))
	for index := range members {
		gotRootOrder[index] = members[index].Name
	}
	if want := []string{"$schema", "extension", "projects"}; !reflect.DeepEqual(gotRootOrder, want) {
		t.Fatalf("root member order = %#v, want %#v", gotRootOrder, want)
	}
	extension, ok := document.Lookup("extension")
	if !ok {
		t.Fatal("unknown root extension was lost")
	}
	order, ok := extension.Lookup("order")
	if !ok || order.Kind() != wirejson.Array {
		t.Fatal("unknown ordered extension was changed")
	}
	projects, ok := document.Lookup("projects")
	if !ok {
		t.Fatal("projects object is missing")
	}
	acme, ok := projects.Lookup("acme")
	if !ok {
		t.Fatal("acme project is missing")
	}
	workProvider, ok := acme.Lookup("workProvider")
	if !ok {
		t.Fatal("project work provider is missing")
	}
	if value, isString := workProvider.AsString(); !isString || value != "github" {
		t.Fatalf("project work provider = %q, string=%v", value, isString)
	}
	providers, ok := acme.Lookup("providers")
	if !ok {
		t.Fatal("project provider extensions are missing")
	}
	github, ok := providers.Lookup("github")
	if !ok {
		t.Fatal("github provider extension is missing")
	}
	providerExtension, ok := github.Lookup("extension")
	if !ok || providerExtension.Kind() != wirejson.Object {
		t.Fatal("provider extension data was changed")
	}
	repositories, ok := acme.Lookup("repositories")
	if !ok {
		t.Fatal("repositories object is missing")
	}
	front, ok := repositories.Lookup("front")
	if !ok {
		t.Fatal("front repository is missing")
	}
	url, ok := front.Lookup("url")
	if !ok || url.Kind() != wirejson.Object {
		t.Fatalf("legacy URL was not migrated: %#v", url)
	}
	httpURL, ok := url.Lookup("http")
	if !ok {
		t.Fatal("migrated HTTP URL is missing")
	}
	if value, isString := httpURL.AsString(); !isString || value != "https://github.com/torvalds/linux.git" {
		t.Fatalf("migrated HTTP URL = %q, string=%v", value, isString)
	}
	sshURL, ok := url.Lookup("ssh")
	if !ok {
		t.Fatal("migrated SSH URL is missing")
	}
	if value, isString := sshURL.AsString(); !isString || value != "git@github.com:torvalds/linux.git" {
		t.Fatalf("migrated SSH URL = %q, string=%v", value, isString)
	}
	providerRepository, ok := front.Lookup("providerRepository")
	if !ok {
		t.Fatal("provider repository mapping is missing")
	}
	if value, isString := providerRepository.AsString(); !isString || value != "platform/front" {
		t.Fatalf("provider repository = %q, string=%v", value, isString)
	}
	kept, ok := front.Lookup("extension")
	if !ok {
		t.Fatal("repository extension was lost")
	}
	if value, isString := kept.AsString(); !isString || value != "keep" {
		t.Fatalf("repository extension changed: %q", value)
	}
}
