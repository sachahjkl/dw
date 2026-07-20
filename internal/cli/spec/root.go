package spec

import (
	"sort"

	"github.com/sachahjkl/dw/internal/l10n"
)

var rejectedLegacyRoutes = [][]string{
	{"task"},
	{"work", "start-pr"},
	{"work", "add-work-item"},
	{"work", "remove-work-item"},
	{"work", "add-repo"},
	{"work", "repo-latest"},
	{"work", "handoff-validate"},
	{"work", "create-child-task"},
	{"ado", "work-item"},
	{"ado", "set-state"},
	{"ado", "ai-context"},
	{"agent", "set-default"},
	{"config", "set-root"},
	{"config", "set-color"},
}

// EnglishEntries returns the deterministic CLI catalog for composition with l10n.NewEnglish.
func EnglishEntries() []l10n.Entry {
	root := Root(nil)
	ids := make([]string, 0, len(root.english))
	for id := range root.english {
		ids = append(ids, string(id))
	}
	sort.Strings(ids)
	entries := make([]l10n.Entry, 0, len(ids))
	for _, id := range ids {
		entries = append(entries, l10n.Entry{ID: l10n.ID(id), Text: root.english[l10n.ID(id)]})
	}
	return entries
}

// Root constructs the authoritative CLI grammar and binds its localization gateway.
func Root(localizer l10n.Localizer) *Command {
	b := newBuilder(localizer)
	verbose := b.option("root", "verbose", Count, "Increase diagnostic output (-v info, -vv debug).")
	verbose.Short, verbose.Global, verbose.Completion.Hidden = 'v', true, true
	verbose.HelpBefore = "help"
	help := b.option("root", "help", Bool, "Show help.")
	help.Short, help.Global, help.Completion.Hidden = 'h', true, true
	help.Special = SpecialHelp
	version := b.option("root", "version", Bool, "Show version.")
	version.Short, version.Global, version.Completion.Hidden = 'V', true, true
	version.Special = SpecialVersion

	root := b.command("dw", "root", "Dev Workflow", []Argument{verbose, help, version},
		b.command("version", "version", "Show the CLI version.", nil),
		func() *Command {
			c := b.command("guide", "guide", "Explain the getting-started flow.", nil)
			c.Aliases = []string{"get-started"}
			return c
		}(),
		b.command("doctor", "doctor", "Diagnose machine prerequisites and local configuration.", []Argument{
			b.option("doctor", "fix", Bool, "Apply automatic fixes."),
		}),
		b.command("init", "init", "Initialize a local DevWorkflow root.", []Argument{
			defaultString(completion(b.option("init", "profile", String, "Template profile to use."), CompleteProfile, "default"), "default"),
			b.option("init", "root", String, "DevWorkflow root to initialize."),
			b.option("init", "dry-run", Bool, "Preview without writing."),
			b.option("init", "no-save", Bool, "Do not save the default root."),
		}),
		b.command("refresh", "refresh", "Regenerate schemas and agent contexts.", []Argument{
			b.option("refresh", "root", String, "DevWorkflow root to use."),
			defaultString(completion(b.option("refresh", "profile", String, "Template profile to use."), CompleteProfile, "default"), "default"),
		}),
		b.command("tui", "tui", "Open the DevWorkflow TUI dashboard.", []Argument{
			b.option("tui", "root", String, "DevWorkflow root to use."),
		}),
		agentGrammar(b), authGrammar(b), completionGrammar(b), configGrammar(b), adoGrammar(b), dbGrammar(b), secretGrammar(b),
		b.command("upgrade", "upgrade", "Upgrade the dw binary.", []Argument{
			conflict(b.option("upgrade", "check", Bool, "Check without updating."), "rid"),
			conflict(b.option("upgrade", "rid", String, "Artifact runtime identifier."), "check"),
		}),
		workGrammar(b),
	)
	root.CompletionAlphabetical = false
	root.RejectedPaths = rejectedLegacyRoutes
	attach(root, nil, localizer, b.english)
	return root
}

func authGrammar(b *builder) *Command {
	return b.command("auth", "auth", "Manage Azure DevOps authentication.", nil,
		b.command("login", "auth.login", "Connect Azure DevOps.", []Argument{b.option("auth.login", "root", String, "DevWorkflow root to use for auth configuration.")}),
		b.command("status", "auth.status", "Show Azure DevOps connection status.", []Argument{b.option("auth.status", "root", String, "DevWorkflow root to use for auth configuration.")}),
		b.command("logout", "auth.logout", "Remove the local Azure DevOps session.", []Argument{b.option("auth.logout", "root", String, "DevWorkflow root to use for auth configuration.")}),
	)
}

func completionGrammar(b *builder) *Command {
	complete := b.command("complete", "completion.complete", "Return completion candidates for a shell integration.", []Argument{
		defaultString(completion(choices(b.option("completion.complete", "format", String, "Completion wire format."), "bash", "fish", "json", "zsh"), CompleteFormat, "bash", "fish", "json", "zsh"), "bash"),
		repeat(b.positional("completion.complete", "words", "WORD", Strings, false, "Command words to complete.")),
	})
	complete.Hidden = true
	shells := []string{"bash", "fish", "zsh", "powershell", "elvish"}
	return b.command("completion", "completion", "Install or inspect shell completions.", nil,
		b.command("show", "completion.show", "Show shell completion installation commands.", nil),
		b.command("generate", "completion.generate", "Generate static shell completion.", []Argument{
			completion(choices(b.positional("completion.generate", "shell", "SHELL", String, true, "Shell to generate completion for."), shells...), CompleteShell, shells...),
		}),
		b.command("install", "completion.install", "Print the installed dynamic completion integration.", []Argument{
			completion(choices(b.positional("completion.install", "shell", "SHELL", String, true, "Shell to install completion for."), shells...), CompleteShell, shells...),
		}),
		complete,
	)
}

func configGrammar(b *builder) *Command {
	return b.command("config", "config", "Validate and edit configuration.", nil,
		b.command("show", "config.show", "Show the root, color mode, and configuration paths.", []Argument{
			b.option("config.show", "root", String, "DevWorkflow root to inspect."), b.option("config.show", "json", Bool, "Emit the deterministic JSON report."),
		}),
		b.command("doctor", "config.doctor", "Verify local configuration files and schemas.", []Argument{
			b.option("config.doctor", "root", String, "DevWorkflow root to verify."), b.option("config.doctor", "json", Bool, "Emit the deterministic JSON report."),
		}),
		b.command("root", "config.root", "Manage the configured DevWorkflow root.", nil,
			b.command("set", "config.root.set", "Save the user DevWorkflow root.", []Argument{b.positional("config.root.set", "path", "PATH", String, true, "DevWorkflow root path to save.")}),
		),
		b.command("color", "config.color", "Manage terminal color configuration.", nil,
			b.command("set", "config.color.set", "Configure color mode: auto, always, or never.", []Argument{
				valueDescriptions(b, "config.color.set", completion(b.positional("config.color.set", "mode", "MODE", String, true, "Color mode to save: auto, always, or never."), CompleteColor, "auto", "always", "never"), "Color based on terminal", "Force color", "Disable color"),
			}),
		),
	)
}

func secretGrammar(b *builder) *Command {
	return b.command("secret", "secret", "Store local secrets.", nil,
		b.command("list", "secret.list", "List configured secret keys and whether they exist.", []Argument{
			b.option("secret.list", "root", String, "DevWorkflow root whose configuration should be inspected."), b.option("secret.list", "json", Bool, "Emit deterministic JSON without secret values."),
		}),
		b.command("set", "secret.set", "Save a secret in the system keyring.", []Argument{
			completion(b.positional("secret.set", "key", "KEY", String, true, "Logical secret key, for example a credentialReference."), CompleteSecret),
			conflict(b.option("secret.set", "value", String, "Secret value to save."), "from_env"),
			completion(conflict(b.option("secret.set", "from-env", String, "Environment variable name containing the secret."), "value"), CompleteEnvVariable),
		}),
		b.command("get", "secret.get", "Check whether a secret exists without showing its value.", []Argument{completion(b.positional("secret.get", "key", "KEY", String, true, "Logical secret key to check."), CompleteSecret)}),
		b.command("delete", "secret.delete", "Delete a secret from the system keyring.", []Argument{
			completion(b.positional("secret.delete", "key", "KEY", String, true, "Logical secret key to delete."), CompleteSecret), b.option("secret.delete", "yes", Bool, "Confirm secret deletion in non-interactive mode."),
		}),
	)
}
