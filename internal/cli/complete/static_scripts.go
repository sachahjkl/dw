package complete

import (
	"sort"
	"strconv"
	"strings"

	"github.com/sachahjkl/dw/internal/cli/spec"
)

type staticRule struct {
	path         string
	children     []string
	options      []string
	positionals  []string
	values       []staticValueRule
	requires     []string
	descriptions map[string]string
}

type staticValueRule struct {
	option string
	values []string
}

func staticRules(root *spec.Command) []staticRule {
	rules := make([]staticRule, 0, 64)
	var walk func(*spec.Command, []string)
	walk = func(command *spec.Command, path []string) {
		rule := staticRule{path: strings.Join(path, " "), descriptions: make(map[string]string)}
		for _, child := range command.Children {
			if child.Hidden {
				continue
			}
			description := command.Text(child.Completion)
			rule.children = append(rule.children, child.Name)
			rule.descriptions[child.Name] = description
			for _, alias := range child.Aliases {
				rule.children = append(rule.children, alias)
				rule.descriptions[alias] = description
			}
		}
		for _, arg := range command.Options(true) {
			if arg.Hidden {
				continue
			}
			descriptionID := arg.Completion.OptionDescription
			if descriptionID == "" {
				descriptionID = arg.Help
			}
			description := command.Text(descriptionID)
			tokens := make([]string, 0, 2)
			if arg.Short != 0 {
				tokens = append(tokens, "-"+string(arg.Short))
			}
			if arg.Long != "" {
				tokens = append(tokens, "--"+arg.Long)
			}
			for _, token := range tokens {
				rule.options = append(rule.options, token)
				rule.descriptions[token] = description
			}
			if arg.Kind != spec.Bool && arg.Kind != spec.Count {
				for _, token := range tokens {
					rule.requires = append(rule.requires, token)
					if len(arg.Completion.Values) != 0 {
						rule.values = append(rule.values, staticValueRule{option: token, values: append([]string(nil), arg.Completion.Values...)})
					}
				}
				for index, value := range arg.Completion.Values {
					valueID := arg.Completion.Description
					if index < len(arg.Completion.ValueDescriptions) {
						valueID = arg.Completion.ValueDescriptions[index]
					}
					rule.descriptions[value] = command.Text(valueID)
				}
			}
		}
		for _, arg := range command.Positionals() {
			for index, value := range arg.Completion.Values {
				rule.positionals = append(rule.positionals, value)
				valueID := arg.Completion.Description
				if index < len(arg.Completion.ValueDescriptions) {
					valueID = arg.Completion.ValueDescriptions[index]
				}
				rule.descriptions[value] = command.Text(valueID)
			}
		}
		rules = append(rules, rule)
		for _, child := range command.Children {
			if child.Hidden {
				continue
			}
			canonicalPath := strings.TrimSpace(strings.Join(path, " ") + " " + child.Name)
			start := len(rules)
			walk(child, append(append([]string(nil), path...), child.Name))
			canonicalRules := append([]staticRule(nil), rules[start:]...)
			for _, alias := range child.Aliases {
				aliasPath := strings.TrimSpace(strings.Join(path, " ") + " " + alias)
				for _, canonical := range canonicalRules {
					clone := canonical
					clone.path = aliasPath + strings.TrimPrefix(canonical.path, canonicalPath)
					rules = append(rules, clone)
				}
			}
		}
	}
	walk(root, nil)
	sort.SliceStable(rules, func(i, j int) bool { return len(rules[i].path) > len(rules[j].path) })
	return rules
}

func generateBash(root *spec.Command) string {
	rules := staticRules(root)
	var out strings.Builder
	out.WriteString("_dw_static_children() { case \"$1\" in\n")
	writeShellCases(&out, rules, func(rule staticRule) []string { return rule.children })
	out.WriteString("  *) printf '%s' '';; esac; }\n_dw_static_options() { case \"$1\" in\n")
	writeShellCases(&out, rules, func(rule staticRule) []string { return rule.options })
	out.WriteString("  *) printf '%s' '';; esac; }\n_dw_static_positionals() { case \"$1\" in\n")
	writeShellCases(&out, rules, func(rule staticRule) []string { return rule.positionals })
	out.WriteString("  *) printf '%s' '';; esac; }\n_dw_static_requires() { case \"$1|$2\" in\n")
	for _, rule := range rules {
		for _, option := range rule.requires {
			out.WriteString("  " + shellQuote(rule.path+"|"+option) + ") return 0;;\n")
		}
	}
	out.WriteString("  *) return 1;; esac; }\n_dw_static_values() { case \"$1|$2\" in\n")
	for _, rule := range rules {
		for _, value := range rule.values {
			out.WriteString("  " + shellQuote(rule.path+"|"+value.option) + ") printf '%s' " + shellQuote(strings.Join(value.values, " ")) + ";;\n")
		}
	}
	out.WriteString("  *) printf '%s' '';; esac; }\n")
	out.WriteString(`_dw_static_complete() {
  local path="" expect=0 word option children current previous candidates i
  for ((i=1; i<COMP_CWORD; i++)); do
    word=${COMP_WORDS[i]}
    if ((expect)); then expect=0; continue; fi
    option=${word%%=*}
    if [[ $word == --*=* ]]; then continue; fi
    if _dw_static_requires "$path" "$option"; then expect=1; continue; fi
    if [[ $word == -* ]]; then continue; fi
    children=" $(_dw_static_children "$path") "
    if [[ $children == *" $word "* ]]; then
      if [[ -n $path ]]; then path+=" "; fi
      path+="$word"
    fi
  done
  current=${COMP_WORDS[COMP_CWORD]}
  previous=""; ((COMP_CWORD > 0)) && previous=${COMP_WORDS[COMP_CWORD-1]}
  if _dw_static_requires "$path" "$previous"; then
    candidates=$(_dw_static_values "$path" "$previous")
  elif [[ $current == -* ]]; then
    candidates=$(_dw_static_options "$path")
  else
    candidates="$(_dw_static_children "$path") $(_dw_static_positionals "$path")"
  fi
  if [[ -n $candidates ]]; then
    COMPREPLY=( $(compgen -W "$candidates" -- "$current") )
  else
    COMPREPLY=( $(compgen -f -- "$current") )
  fi
}
complete -F _dw_static_complete dw
`)
	return out.String()
}

func generateZsh(root *spec.Command) string {
	rules := staticRules(root)
	var out strings.Builder
	out.WriteString("#compdef dw\n_dw_static_children() { case \"$1\" in\n")
	writeShellCases(&out, rules, func(rule staticRule) []string { return rule.children })
	out.WriteString("  *) printf '%s' '';; esac; }\n_dw_static_options() { case \"$1\" in\n")
	writeShellCases(&out, rules, func(rule staticRule) []string { return rule.options })
	out.WriteString("  *) printf '%s' '';; esac; }\n_dw_static_positionals() { case \"$1\" in\n")
	writeShellCases(&out, rules, func(rule staticRule) []string { return rule.positionals })
	out.WriteString("  *) printf '%s' '';; esac; }\n_dw_static_requires() { case \"$1|$2\" in\n")
	for _, rule := range rules {
		for _, option := range rule.requires {
			out.WriteString("  " + shellQuote(rule.path+"|"+option) + ") return 0;;\n")
		}
	}
	out.WriteString("  *) return 1;; esac; }\n_dw_static_values() { case \"$1|$2\" in\n")
	for _, rule := range rules {
		for _, value := range rule.values {
			out.WriteString("  " + shellQuote(rule.path+"|"+value.option) + ") printf '%s' " + shellQuote(strings.Join(value.values, " ")) + ";;\n")
		}
	}
	out.WriteString("  *) printf '%s' '';; esac; }\n")
	out.WriteString("_dw_static_description() { case \"$1|$2\" in\n")
	for _, rule := range rules {
		for _, token := range sortedDescriptionTokens(rule) {
			out.WriteString("  " + shellQuote(rule.path+"|"+token) + ") printf '%s' " + shellQuote(rule.descriptions[token]) + ";;\n")
		}
	}
	out.WriteString("  *) printf '%s' '';; esac; }\n")
	out.WriteString(`_dw_static_complete() {
  local path="" expect=0 word option children current previous candidates candidate i
  local -a labels descriptions
  for ((i=2; i<CURRENT; i++)); do
    word=$words[i]
    if ((expect)); then expect=0; continue; fi
    option=${word%%=*}
    if [[ $word == --*=* ]]; then continue; fi
    if _dw_static_requires "$path" "$option"; then expect=1; continue; fi
    if [[ $word == -* ]]; then continue; fi
    children=" $(_dw_static_children "$path") "
    if [[ $children == *" $word "* ]]; then
      [[ -n $path ]] && path+=" "
      path+="$word"
    fi
  done
  current=$words[CURRENT]
  previous=""; ((CURRENT > 2)) && previous=$words[CURRENT-1]
  if _dw_static_requires "$path" "$previous"; then
    candidates=$(_dw_static_values "$path" "$previous")
  elif [[ $current == -* ]]; then
    candidates=$(_dw_static_options "$path")
  else
    candidates="$(_dw_static_children "$path") $(_dw_static_positionals "$path")"
  fi
  if [[ -n $candidates ]]; then
    labels=(${(z)candidates})
    for candidate in $labels; do descriptions+=("$(_dw_static_description "$path" "$candidate")"); done
    compadd -d descriptions -a labels
  else
    _files
  fi
}
compdef _dw_static_complete dw
`)
	return out.String()
}

func generateFish(root *spec.Command) string {
	rules := staticRules(root)
	var out strings.Builder
	out.WriteString("function __dw_static_rule\n  set -l kind $argv[1]; set -l path $argv[2]\n  switch \"$kind|$path\"\n")
	for _, rule := range rules {
		kinds := []struct {
			name   string
			values []string
		}{{"children", rule.children}, {"options", rule.options}, {"positionals", rule.positionals}, {"requires", rule.requires}}
		for _, kind := range kinds {
			out.WriteString("    case " + fishQuote(kind.name+"|"+rule.path) + "\n      echo " + fishQuote(strings.Join(kind.values, " ")) + "\n")
		}
	}
	out.WriteString("  end\nend\nfunction __dw_static_values\n  switch \"$argv[1]|$argv[2]\"\n")
	for _, rule := range rules {
		for _, value := range rule.values {
			out.WriteString("    case " + fishQuote(rule.path+"|"+value.option) + "\n      echo " + fishQuote(strings.Join(value.values, " ")) + "\n")
		}
	}
	out.WriteString("  end\nend\nfunction __dw_static_description\n  switch \"$argv[1]|$argv[2]\"\n")
	for _, rule := range rules {
		for _, token := range sortedDescriptionTokens(rule) {
			out.WriteString("    case " + fishQuote(rule.path+"|"+token) + "\n      echo " + fishQuote(rule.descriptions[token]) + "\n")
		}
	}
	out.WriteString("  end\nend\nfunction __dw_static_complete\n  set -l words (commandline -opc); set -e words[1]; set -l path ''; set -l expect 0; set -l previous ''\n  for word in $words\n    if test $expect -eq 1; set expect 0; set previous $word; continue; end\n    set -l requires (string split ' ' (__dw_static_rule requires \"$path\"))\n    if contains -- $word $requires; set expect 1; set previous $word; continue; end\n    if string match -q -- '-*' $word; set previous $word; continue; end\n    set -l children (string split ' ' (__dw_static_rule children \"$path\"))\n    if contains -- $word $children; set path (string trim \"$path $word\"); end\n    set previous $word\n  end\n  set -l current (commandline -ct); set -l candidates ''\n  set -l value_candidates (__dw_static_values \"$path\" \"$previous\")\n  if test -n \"$value_candidates\"; set candidates $value_candidates\n  else if string match -q -- '-*' $current; set candidates (__dw_static_rule options \"$path\")\n  else; set candidates (__dw_static_rule children \"$path\")' '(__dw_static_rule positionals \"$path\"); end\n  if test -z (string trim \"$candidates\"); __fish_complete_path; return; end\n  for candidate in (string split ' ' (string trim $candidates))\n    set -l description (__dw_static_description \"$path\" \"$candidate\")\n    if test -n \"$description\"; printf '%s\\t%s\\n' $candidate $description; else; echo $candidate; end\n  end\nend\ncomplete -c dw -f -a '(__dw_static_complete)'\n")
	return out.String()
}

func generatePowerShell(root *spec.Command) string {
	rules := staticRules(root)
	var out strings.Builder
	out.WriteString("$__dwChildren = @{\n")
	for _, rule := range rules {
		out.WriteString("  " + psQuote(rule.path) + " = " + psArray(rule.children) + "\n")
	}
	out.WriteString("}\n$__dwOptions = @{\n")
	for _, rule := range rules {
		out.WriteString("  " + psQuote(rule.path) + " = " + psArray(rule.options) + "\n")
	}
	out.WriteString("}\n$__dwPositionals = @{\n")
	for _, rule := range rules {
		out.WriteString("  " + psQuote(rule.path) + " = " + psArray(rule.positionals) + "\n")
	}
	out.WriteString("}\n$__dwValues = @{\n")
	for _, rule := range rules {
		for _, value := range rule.values {
			out.WriteString("  " + psQuote(rule.path+"|"+value.option) + " = " + psArray(value.values) + "\n")
		}
	}
	out.WriteString("}\n$__dwDescriptions = @{\n")
	for _, rule := range rules {
		for _, token := range sortedDescriptionTokens(rule) {
			out.WriteString("  " + psQuote(rule.path+"|"+token) + " = " + psQuote(rule.descriptions[token]) + "\n")
		}
	}
	out.WriteString("}\nRegister-ArgumentCompleter -Native -CommandName dw -ScriptBlock { param($wordToComplete, $commandAst, $cursorPosition)\n  $path = ''; $elements = @($commandAst.CommandElements | Select-Object -Skip 1 | ForEach-Object { $_.Extent.Text }); $prior = $elements\n  if ($prior.Count -and $prior[-1] -eq $wordToComplete) { if ($prior.Count -eq 1) { $prior = @() } else { $prior = @($prior[0..($prior.Count-2)]) } }\n  foreach ($word in $prior) { if ($word.StartsWith('-')) { continue }; if ($__dwChildren[$path] -contains $word) { $path = ($path+' '+$word).Trim() } }\n  $previous = if ($prior.Count) { $prior[-1] } else { '' }; $key = $path+'|'+$previous\n  if ($__dwValues.ContainsKey($key)) { $candidates = $__dwValues[$key] } elseif ($wordToComplete.StartsWith('-')) { $candidates = $__dwOptions[$path] } else { $candidates = @($__dwChildren[$path]) + @($__dwPositionals[$path]) }\n  if (-not $candidates) { $candidates = @(Get-ChildItem -Name -Path ($wordToComplete+'*')) }\n  foreach ($candidate in $candidates) { if ($candidate -like \"$wordToComplete*\") { $description = $__dwDescriptions[$path+'|'+$candidate]; [System.Management.Automation.CompletionResult]::new($candidate,$candidate,'ParameterValue',$description) } }\n}\n")
	return out.String()
}

func generateElvish(root *spec.Command) string {
	rules := staticRules(root)
	ordered := append([]staticRule(nil), rules...)
	sort.SliceStable(ordered, func(i, j int) bool { return len(ordered[i].path) < len(ordered[j].path) })
	var out strings.Builder
	out.WriteString("use str;\nset edit:completion:arg-completer[dw] = {|@args|\n  var path = ''\n  for word $args {\n")
	for _, rule := range ordered {
		for _, child := range rule.children {
			childPath := strings.TrimSpace(rule.path + " " + child)
			out.WriteString("    if (and (== $path " + elvQuote(rule.path) + ") (== $word " + elvQuote(child) + ")) { set path = " + elvQuote(childPath) + " }\n")
		}
	}
	out.WriteString("  }\n  var candidates = []\n  var current = ''\n  if (> (count $args) 0) { set current = $args[-1] }\n")
	for _, rule := range rules {
		plain := append(append([]string{}, rule.children...), rule.positionals...)
		out.WriteString("  if (== $path " + elvQuote(rule.path) + ") { set candidates = [" + elvWords(plain) + "] }\n")
		out.WriteString("  if (and (== $path " + elvQuote(rule.path) + ") (str:has-prefix $current '-')) { set candidates = [" + elvWords(rule.options) + "] }\n")
	}
	out.WriteString("  if (> (count $args) 0) {\n    var previous = $args[-1]\n")
	for _, rule := range rules {
		for _, value := range rule.values {
			out.WriteString("    if (and (== $path " + elvQuote(rule.path) + ") (== $previous " + elvQuote(value.option) + ")) { set candidates = [" + elvWords(value.values) + "] }\n")
		}
	}
	out.WriteString("  }\n  if (> (count $candidates) 0) {\n    each {|candidate|\n      var description = ''\n")
	for _, rule := range rules {
		for _, token := range sortedDescriptionTokens(rule) {
			out.WriteString("      if (and (== $path " + elvQuote(rule.path) + ") (== $candidate " + elvQuote(token) + ")) { set description = " + elvQuote(rule.descriptions[token]) + " }\n")
		}
	}
	out.WriteString("      if (> (count $description) 0) { edit:complex-candidate $candidate &display=$candidate'  '$description &code-suffix=' ' } else { put $candidate }\n    } $candidates\n  } else { edit:complete-filename $@args }\n}\n")
	return out.String()
}

func writeShellCases(out *strings.Builder, rules []staticRule, values func(staticRule) []string) {
	for _, rule := range rules {
		out.WriteString("  " + shellQuote(rule.path) + ") printf '%s' " + shellQuote(strings.Join(values(rule), " ")) + ";;\n")
	}
}

func sortedDescriptionTokens(rule staticRule) []string {
	tokens := make([]string, 0, len(rule.descriptions))
	for token := range rule.descriptions {
		tokens = append(tokens, token)
	}
	sort.Strings(tokens)
	return tokens
}

func shellQuote(value string) string { return "'" + strings.ReplaceAll(value, "'", "'\\''") + "'" }
func fishQuote(value string) string  { return "'" + strings.ReplaceAll(value, "'", "\\'") + "'" }
func psQuote(value string) string    { return "'" + strings.ReplaceAll(value, "'", "''") + "'" }
func psArray(values []string) string {
	parts := make([]string, len(values))
	for i, value := range values {
		parts[i] = psQuote(value)
	}
	return "@(" + strings.Join(parts, ",") + ")"
}
func elvQuote(value string) string { return strconv.Quote(value) }
func elvWords(values []string) string {
	parts := make([]string, len(values))
	for i, value := range values {
		parts[i] = elvQuote(value)
	}
	return strings.Join(parts, " ")
}
