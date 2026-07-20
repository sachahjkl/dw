package update

import (
	"encoding/json"
	"fmt"
	"net/http"

	"github.com/sachahjkl/dw/internal/config"
)

const (
	DefaultOwner         = "sachahjkl"
	DefaultRepository    = "dw"
	DefaultManifestAsset = "release.json"
	DefaultGitHubAPI     = "https://api.github.com"
)

type Config = config.UpdateOptions

type GitHubRelease struct {
	TagName string
	Assets  []GitHubAsset
}

type GitHubAsset struct {
	Name string
	URL  string
}

type Manifest struct {
	Version string
	Commit  string
	Assets  []Asset
}

type Asset struct {
	RID      string
	FileName string
	SHA256   string
	URL      string
}

type AssetSummary struct {
	RID      string `json:"rid"`
	FileName string `json:"file_name"`
	SHA256   string `json:"sha256"`
}

type CheckReport struct {
	ReleaseTag string         `json:"release_tag"`
	Version    string         `json:"version"`
	Commit     string         `json:"commit"`
	Assets     []AssetSummary `json:"assets"`
}

type InstallReport struct {
	Version                    string `json:"version"`
	Commit                     string `json:"commit"`
	ExecutablePath             string `json:"executable_path"`
	DeferredWindowsReplacement bool   `json:"deferred_windows_replacement"`
}

type Report struct {
	Kind      string
	Check     *CheckReport
	Installed *InstallReport
}

func (report Report) MarshalJSON() ([]byte, error) {
	switch report.Kind {
	case "check":
		if report.Check == nil {
			return nil, fmt.Errorf("update: nil-check-report")
		}
		return json.Marshal(struct {
			Kind       string         `json:"kind"`
			ReleaseTag string         `json:"release_tag"`
			Version    string         `json:"version"`
			Commit     string         `json:"commit"`
			Assets     []AssetSummary `json:"assets"`
		}{"check", report.Check.ReleaseTag, report.Check.Version, report.Check.Commit, report.Check.Assets})
	case "installed":
		if report.Installed == nil {
			return nil, fmt.Errorf("update: nil-install-report")
		}
		return json.Marshal(struct {
			Kind                       string `json:"kind"`
			Version                    string `json:"version"`
			Commit                     string `json:"commit"`
			ExecutablePath             string `json:"executable_path"`
			DeferredWindowsReplacement bool   `json:"deferred_windows_replacement"`
		}{"installed", report.Installed.Version, report.Installed.Commit, report.Installed.ExecutablePath, report.Installed.DeferredWindowsReplacement})
	default:
		return nil, fmt.Errorf("update: invalid-report-kind %q", report.Kind)
	}
}

type Event struct {
	Kind           string `json:"kind"`
	Owner          string `json:"owner,omitempty"`
	Repository     string `json:"repository,omitempty"`
	AssetName      string `json:"asset_name,omitempty"`
	RID            string `json:"rid,omitempty"`
	FileName       string `json:"file_name,omitempty"`
	Received       int64  `json:"received,omitempty"`
	Total          *int64 `json:"total,omitempty"`
	ExpectedSHA256 string `json:"expected_sha256,omitempty"`
	ExecutablePath string `json:"executable_path,omitempty"`
	Version        string `json:"version,omitempty"`
}

func (event Event) MarshalJSON() ([]byte, error) {
	switch event.Kind {
	case "checking-host", "resolving-config":
		return json.Marshal(struct {
			Kind string `json:"kind"`
		}{event.Kind})
	case "fetching-release":
		return json.Marshal(struct {
			Kind       string `json:"kind"`
			Owner      string `json:"owner"`
			Repository string `json:"repository"`
		}{event.Kind, event.Owner, event.Repository})
	case "fetching-manifest":
		return json.Marshal(struct {
			Kind      string `json:"kind"`
			AssetName string `json:"asset_name"`
		}{event.Kind, event.AssetName})
	case "selecting-asset":
		return json.Marshal(struct {
			Kind string `json:"kind"`
			RID  string `json:"rid"`
		}{event.Kind, event.RID})
	case "downloading-asset":
		return json.Marshal(struct {
			Kind     string `json:"kind"`
			FileName string `json:"file_name"`
		}{event.Kind, event.FileName})
	case "downloaded-asset-bytes":
		return json.Marshal(struct {
			Kind     string `json:"kind"`
			FileName string `json:"file_name"`
			Received int64  `json:"received"`
			Total    *int64 `json:"total"`
		}{event.Kind, event.FileName, event.Received, event.Total})
	case "verifying-checksum":
		return json.Marshal(struct {
			Kind           string `json:"kind"`
			FileName       string `json:"file_name"`
			ExpectedSHA256 string `json:"expected_sha256"`
		}{event.Kind, event.FileName, event.ExpectedSHA256})
	case "preparing-executable":
		return json.Marshal(struct {
			Kind     string `json:"kind"`
			FileName string `json:"file_name"`
			RID      string `json:"rid"`
		}{event.Kind, event.FileName, event.RID})
	case "replacing-executable":
		return json.Marshal(struct {
			Kind           string `json:"kind"`
			ExecutablePath string `json:"executable_path"`
		}{event.Kind, event.ExecutablePath})
	case "completed":
		return json.Marshal(struct {
			Kind    string `json:"kind"`
			Version string `json:"version"`
		}{event.Kind, event.Version})
	default:
		return nil, fmt.Errorf("update: invalid-event-kind %q", event.Kind)
	}
}

func (event Event) ActionID() string {
	switch event.Kind {
	case "checking-host":
		return "upgrade.host.check"
	case "resolving-config":
		return "upgrade.config.resolve"
	case "fetching-release":
		return "upgrade.release.fetch"
	case "fetching-manifest":
		return "upgrade.manifest.fetch"
	case "selecting-asset":
		return "upgrade.asset.select"
	case "downloading-asset":
		return "upgrade.asset.download"
	case "downloaded-asset-bytes":
		return "upgrade.asset.download.progress"
	case "verifying-checksum":
		return "upgrade.checksum.verify"
	case "preparing-executable":
		return "upgrade.executable.prepare"
	case "replacing-executable":
		return "upgrade.executable.replace"
	case "completed":
		return "upgrade.complete"
	default:
		return ""
	}
}

type Request struct {
	Check          bool
	RID            string
	Config         Config
	ExecutablePath string
}

type EmitFunc func(Event)

type Service struct {
	HTTPClient *http.Client
	APIBaseURL string
	UserAgent  string
	TempDir    string
}
