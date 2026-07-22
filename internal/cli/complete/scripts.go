package complete

import (
	"fmt"
	"strings"

	"github.com/sachahjkl/dw/internal/cli/spec"
)

type Shell string

const (
	Bash       Shell = "bash"
	Fish       Shell = "fish"
	Zsh        Shell = "zsh"
	PowerShell Shell = "powershell"
	Elvish     Shell = "elvish"
)

func ParseShell(value string) (Shell, error) {
	shell := Shell(strings.ToLower(value))
	switch shell {
	case Bash, Fish, Zsh, PowerShell, Elvish:
		return shell, nil
	default:
		return "", fmt.Errorf("cli.completion.invalid-shell:%s", value)
	}
}

func Install(shell Shell) (string, error) {
	switch shell {
	case Bash:
		return `_dw_complete() {
  local -a words
  words=("${COMP_WORDS[@]:1:COMP_CWORD}")
  COMPREPLY=()
  while IFS= read -r line; do
    COMPREPLY+=("$line")
  done < <(dw completion complete --format bash -- "${words[@]}")
}
complete -F _dw_complete dw
`, nil
	case Zsh:
		return `#compdef dw
_dw_complete() {
  local -a rows labels descriptions
  rows=("${(@f)$(dw completion complete --format zsh -- $words[2,-1])}")
  local row label description
  for row in $rows; do
    label=${row%%$'\t'*}
    if [[ "$row" == *$'\t'* ]]; then
      description=${row#*$'\t'}
    else
      description=""
    fi
    labels+=("$label")
    descriptions+=("$description")
  done
  compadd -d descriptions -a labels
}
compdef _dw_complete dw
`, nil
	case Fish:
		return "complete -c dw -f -a '(dw completion complete --format fish -- (commandline -opc)[2..-1])'\n", nil
	case PowerShell:
		return "Register-ArgumentCompleter -Native -CommandName dw -ScriptBlock { param($wordToComplete, $commandAst, $cursorPosition) dw completion complete --format json -- @($commandAst.CommandElements | Select-Object -Skip 1 | ForEach-Object { $_.Extent.Text }) | ConvertFrom-Json | ForEach-Object { [System.Management.Automation.CompletionResult]::new($_.label, $_.label, 'ParameterValue', $_.description) } }\n", nil
	case Elvish:
		return "set edit:completion:arg-completer[dw] = {|@args|\n  dw completion complete --format json -- $args[1..] | from-json | each {|item|\n    edit:complex-candidate $item[label] &display=$item[label]'  '$item[description] &code-suffix=' '\n  }\n}\n", nil
	default:
		return "", fmt.Errorf("cli.completion.invalid-shell:%s", shell)
	}
}

// Generate returns a self-contained static catalog; unlike Install it never invokes dw at completion time.
func Generate(root *spec.Command, shell Shell) (string, error) {
	switch shell {
	case Bash:
		return generateBash(root), nil
	case Zsh:
		return generateZsh(root), nil
	case Fish:
		return generateFish(root), nil
	case PowerShell:
		return generatePowerShell(root), nil
	case Elvish:
		return generateElvish(root), nil
	default:
		return "", fmt.Errorf("cli.completion.invalid-shell:%s", shell)
	}
}

func Show(root *spec.Command) string {
	return strings.Join([]string{
		root.Text(spec.MsgCompletionTitle),
		root.Text(spec.MsgCompletionIntro),
		"",
		"  bash       dw completion install bash >> ~/.bashrc",
		"  zsh        dw completion install zsh >> ~/.zshrc",
		"  fish       dw completion install fish > ~/.config/fish/completions/dw.fish",
		"  powershell dw completion install powershell >> $PROFILE",
		"  elvish     dw completion install elvish >> ~/.elvish/rc.elv",
	}, "\n") + "\n"
}
