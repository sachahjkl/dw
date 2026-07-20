// Package doctor evaluates machine readiness without changing state unless fix is requested.
package doctor

import (
	"bytes"
	"context"
	"os"

	"github.com/sachahjkl/dw/internal/contract"
)

type Status string

const (
	StatusHealthy    Status = "healthy"
	StatusNeedsFixes Status = "needs-fixes"
)

type CheckKind string

const (
	CheckDevWorkflowRoot    CheckKind = "dev-workflow-root"
	CheckUserConfiguration  CheckKind = "user-configuration"
	CheckDefaultAgent       CheckKind = "default-agent"
	CheckGit                CheckKind = "git"
	CheckNodePackageManager CheckKind = "node-package-manager"
	CheckOpenCode           CheckKind = "open-code"
)

type DetailKind string

const (
	DetailPath                  DetailKind = "path"
	DetailAgent                 DetailKind = "agent"
	DetailProcessOutput         DetailKind = "process-output"
	DetailPackageManagerVersion DetailKind = "package-manager-version"
)

type RemediationKind string

const (
	RemediationInitRoot                  RemediationKind = "init-root"
	RemediationRunInit                   RemediationKind = "run-init"
	RemediationConfigureDefaultAgent     RemediationKind = "configure-default-agent"
	RemediationInstallGit                RemediationKind = "install-git"
	RemediationInstallNodePackageManager RemediationKind = "install-node-package-manager"
	RemediationInstallOpenCode           RemediationKind = "install-open-code"
)

type PackageManager string

const (
	PackageManagerPnpm PackageManager = "pnpm"
	PackageManagerNpm  PackageManager = "npm"
)

type Report struct {
	Root   string  `json:"root"`
	Checks []Check `json:"checks"`
}

func (report Report) PassedCount() int {
	passed := 0
	for _, check := range report.Checks {
		if check.Passed {
			passed++
		}
	}
	return passed
}

func (report Report) FailedCount() int { return len(report.Checks) - report.PassedCount() }
func (report Report) Passed() bool     { return report.FailedCount() == 0 }
func (report Report) Status() Status {
	if report.Passed() {
		return StatusHealthy
	}
	return StatusNeedsFixes
}

// ExitCode makes diagnostic failure explicit without turning individual failed checks into errors.
func (report Report) ExitCode() int {
	if report.Passed() {
		return 0
	}
	return 1
}

type Check struct {
	Kind        CheckKind    `json:"kind"`
	Passed      bool         `json:"passed"`
	Detail      *CheckDetail `json:"detail"`
	Remediation Remediation  `json:"remediation"`
}

type CheckDetail struct {
	Kind    DetailKind     `json:"kind"`
	Path    string         `json:"path,omitempty"`
	Agent   contract.Agent `json:"agent,omitempty"`
	Line    string         `json:"line,omitempty"`
	Manager PackageManager `json:"manager,omitempty"`
	Version string         `json:"version,omitempty"`
}

type Remediation struct {
	Kind  RemediationKind `json:"kind"`
	Root  string          `json:"root,omitempty"`
	Agent contract.Agent  `json:"agent,omitempty"`
}

// Config is the configuration boundary needed by machine diagnostics.
type Config interface {
	ResolveRoot() string
	UserSettingsPath() string
	DefaultAgent(root string) contract.Agent
	InitRoot(context.Context, InitRequest) error
}

type InitRequest struct {
	Root    string
	Profile string
	NoSave  bool
	DryRun  bool
}

type CommandOutput struct {
	Stdout   []byte
	Stderr   []byte
	ExitCode int
}

// Process is the direct-execution boundary. Implementations must never invoke a shell.
type Process interface {
	Output(context.Context, string, ...string) (CommandOutput, error)
}

type Service struct {
	config  Config
	process Process
}

func New(config Config, process Process) *Service {
	return &Service{config: config, process: process}
}

// Run performs every compatibility check. Failed checks are report data; only inability to apply
// an explicitly requested fix is returned as an error.
func (service *Service) Run(ctx context.Context, fix bool) (Report, error) {
	root := service.config.ResolveRoot()
	rootPassed := isDirectory(root)
	checks := []Check{
		{
			Kind:        CheckDevWorkflowRoot,
			Passed:      rootPassed,
			Detail:      &CheckDetail{Kind: DetailPath, Path: root},
			Remediation: Remediation{Kind: RemediationInitRoot, Root: root},
		},
		{
			Kind:        CheckUserConfiguration,
			Passed:      isFile(service.config.UserSettingsPath()),
			Detail:      &CheckDetail{Kind: DetailPath, Path: service.config.UserSettingsPath()},
			Remediation: Remediation{Kind: RemediationRunInit},
		},
		service.defaultAgentCheck(root),
		service.commandCheck(ctx, "git", CheckGit, RemediationInstallGit),
		service.nodePackageManagerCheck(ctx),
		service.commandCheck(ctx, "opencode", CheckOpenCode, RemediationInstallOpenCode),
	}

	if fix && !rootPassed {
		err := service.config.InitRoot(ctx, InitRequest{
			Root: root, Profile: "default", NoSave: false, DryRun: false,
		})
		if err != nil {
			return Report{}, err
		}
		checks[0].Passed = true
	}
	return Report{Root: root, Checks: checks}, nil
}

func (service *Service) defaultAgentCheck(root string) Check {
	agent := service.config.DefaultAgent(root)
	return Check{
		Kind:        CheckDefaultAgent,
		Passed:      true,
		Detail:      &CheckDetail{Kind: DetailAgent, Agent: agent},
		Remediation: Remediation{Kind: RemediationConfigureDefaultAgent, Agent: contract.AgentOpenCode},
	}
}

func (service *Service) commandCheck(ctx context.Context, name string, kind CheckKind, remediation RemediationKind) Check {
	output, err := service.process.Output(ctx, name, "--version")
	if err != nil || output.ExitCode != 0 {
		return failedCheck(kind, remediation)
	}
	line := firstNonEmptyLine(output)
	if line == "" {
		line = name
	}
	return Check{
		Kind:        kind,
		Passed:      true,
		Detail:      &CheckDetail{Kind: DetailProcessOutput, Line: line},
		Remediation: Remediation{Kind: remediation},
	}
}

func (service *Service) nodePackageManagerCheck(ctx context.Context) Check {
	for _, manager := range [...]PackageManager{PackageManagerPnpm, PackageManagerNpm} {
		output, err := service.process.Output(ctx, string(manager), "--version")
		if err != nil || output.ExitCode != 0 {
			continue
		}
		version := firstNonEmptyLine(output)
		if version == "" {
			version = string(manager)
		}
		return Check{
			Kind:        CheckNodePackageManager,
			Passed:      true,
			Detail:      &CheckDetail{Kind: DetailPackageManagerVersion, Manager: manager, Version: version},
			Remediation: Remediation{Kind: RemediationInstallNodePackageManager},
		}
	}
	return failedCheck(CheckNodePackageManager, RemediationInstallNodePackageManager)
}

func failedCheck(kind CheckKind, remediation RemediationKind) Check {
	return Check{Kind: kind, Remediation: Remediation{Kind: remediation}}
}

func firstNonEmptyLine(output CommandOutput) string {
	if line := firstLine(output.Stdout); line != "" {
		return line
	}
	return firstLine(output.Stderr)
}

func firstLine(output []byte) string {
	for len(output) != 0 {
		line := output
		if index := bytes.IndexByte(output, '\n'); index >= 0 {
			line, output = output[:index], output[index+1:]
		} else {
			output = nil
		}
		line = bytes.TrimSpace(line)
		if len(line) != 0 {
			return string(line)
		}
	}
	return ""
}

func isDirectory(path string) bool {
	info, err := os.Stat(path)
	return err == nil && info.IsDir()
}

func isFile(path string) bool {
	info, err := os.Stat(path)
	return err == nil && info.Mode().IsRegular()
}
