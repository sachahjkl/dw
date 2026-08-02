package update

import (
	"bytes"
	"encoding/json"
	"fmt"
	"io"
	"net/http"

	"github.com/sachahjkl/dw/internal/action"
	"github.com/sachahjkl/dw/internal/config"
)

const (
	DefaultOwner          = "sachahjkl"
	DefaultRepository     = "dw"
	DefaultManifestAsset  = "release.json"
	DefaultGitHubAPI      = "https://api.github.com"
	maxGitHubResponseSize = int64(2 << 20)
	maxManifestSize       = int64(1 << 20)
	maxAssetSize          = int64(256 << 20)
	maxArchiveSize        = int64(128 << 20)
	maxArchiveEntries     = 1024
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
	Kind      string         `json:"kind"`
	Check     *CheckReport   `json:"check,omitempty"`
	Installed *InstallReport `json:"installed,omitempty"`
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

func (report *Report) UnmarshalJSON(data []byte) error {
	var value struct {
		Kind                       string         `json:"kind"`
		ReleaseTag                 string         `json:"release_tag"`
		Version                    string         `json:"version"`
		Commit                     string         `json:"commit"`
		Assets                     []AssetSummary `json:"assets"`
		ExecutablePath             string         `json:"executable_path"`
		DeferredWindowsReplacement bool           `json:"deferred_windows_replacement"`
	}
	decoder := json.NewDecoder(bytes.NewReader(data))
	decoder.DisallowUnknownFields()
	if err := decoder.Decode(&value); err != nil {
		return err
	}
	if err := decoder.Decode(&struct{}{}); err != io.EOF {
		return fmt.Errorf("update: trailing-report-json")
	}
	switch value.Kind {
	case "check":
		*report = Report{Kind: value.Kind, Check: &CheckReport{ReleaseTag: value.ReleaseTag, Version: value.Version, Commit: value.Commit, Assets: value.Assets}}
	case "installed":
		*report = Report{Kind: value.Kind, Installed: &InstallReport{Version: value.Version, Commit: value.Commit, ExecutablePath: value.ExecutablePath, DeferredWindowsReplacement: value.DeferredWindowsReplacement}}
	default:
		return fmt.Errorf("update: invalid-report-kind %q", value.Kind)
	}
	return nil
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

func (Event) EventDataType() action.EventDataType { return "update.event" }
func (Event) EventDataSchema() uint16             { return 1 }

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
	Check          bool   `json:"check"`
	RID            string `json:"rid,omitempty"`
	Config         Config `json:"config"`
	ExecutablePath string `json:"executable_path,omitempty"`
}

type EmitFunc func(Event)

type Service struct {
	HTTPClient *http.Client
	APIBaseURL string
	UserAgent  string
	TempDir    string
}
