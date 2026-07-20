package workspace

import (
	"fmt"
	"path/filepath"
	"strings"
	"unicode"
)

func NormalizeSlug(value string) string {
	var output strings.Builder
	previousDash := false
	for _, r := range strings.TrimSpace(value) {
		r = foldRune(r)
		lower := unicode.ToLower(r)
		if lower >= 'a' && lower <= 'z' || lower >= '0' && lower <= '9' {
			output.WriteRune(lower)
			previousDash = false
		} else if !previousDash {
			output.WriteByte('-')
			previousDash = true
		}
	}
	slug := strings.Trim(output.String(), "-")
	if len(slug) > 50 {
		slug = strings.Trim(slug[:50], "-")
	}
	return slug
}
func SlugOrFallback(value, fallback string) string {
	slug := NormalizeSlug(value)
	if slug == "" {
		return NormalizeSlug(fallback)
	}
	return slug
}
func BuildBranchName(kind string, ids []string, slug string) string {
	kind = strings.ToLower(strings.TrimSpace(kind))
	if kind == "" {
		kind = "feat"
	}
	return fmt.Sprintf("%s/%s-%s", kind, strings.Join(distinctCSV(ids), "-"), NormalizeSlug(slug))
}
func BuildSubjectName(kind string, ids []string, slug string) string {
	kind = strings.ToLower(strings.TrimSpace(kind))
	if kind == "" {
		kind = "feat"
	}
	return fmt.Sprintf("%s-%s-%s", kind, strings.Join(distinctCSV(ids), "-"), NormalizeSlug(slug))
}
func foldRune(r rune) rune {
	switch r {
	case 'à', 'á', 'â', 'ã', 'ä', 'å', 'À', 'Á', 'Â', 'Ã', 'Ä', 'Å':
		return 'a'
	case 'ç', 'Ç':
		return 'c'
	case 'è', 'é', 'ê', 'ë', 'È', 'É', 'Ê', 'Ë':
		return 'e'
	case 'ì', 'í', 'î', 'ï', 'Ì', 'Í', 'Î', 'Ï':
		return 'i'
	case 'ñ', 'Ñ':
		return 'n'
	case 'ò', 'ó', 'ô', 'õ', 'ö', 'Ò', 'Ó', 'Ô', 'Õ', 'Ö':
		return 'o'
	case 'ù', 'ú', 'û', 'ü', 'Ù', 'Ú', 'Û', 'Ü':
		return 'u'
	case 'ý', 'ÿ', 'Ý':
		return 'y'
	default:
		return r
	}
}

func PlanMarkdown(manifest Manifest) string {
	ids := make([]string, 0)
	for _, item := range manifest.ParentWorkItems() {
		ids = append(ids, "#"+item.ID)
	}
	repositories := make([]string, 0)
	for _, repo := range manifest.Repositories {
		repositories = append(repositories, "- "+repo+": TODO")
	}
	return fmt.Sprintf("# Plan - Work items %s\n\nProjet: `%s`\n\n## Résumé fonctionnel\n\nTODO\n\n## Repositories impactés\n\n%s\n\n## Analyse code\n\nTODO\n\n## Plan technique\n\nTODO\n\n## Risques\n\nTODO\n\n## Vérification\n\nTODO\n", strings.Join(ids, ", "), manifest.Project, strings.Join(repositories, "\n"))
}
func HandoffMarkdown(manifest Manifest, repository string) string {
	ids := make([]string, 0)
	for _, item := range manifest.ParentWorkItems() {
		ids = append(ids, "`#"+item.ID+"`")
	}
	return fmt.Sprintf("# Handoff %s\n\n## Contexte\n\n- Projet: `%s`\n- Repository: `%s`\n- Branche: `%s`\n- Work items parents: %s\n- Child tasks connus: (aucune)\n\n## Entrées déterministes à relire\n\n1. `task.json`\n2. `plan.md`\n3. `AGENTS.md`\n4. Contexte IA ADO pour chaque work item parent\n5. Rapport de préflight task\n\n## Objectif du lot\n\nDécrire ici, dans `plan.md`, ce qui relève de `%s` et ce qui doit être traité par ce handoff.\n\n## Contraintes\n\n- Préserver les labels métier exacts\n- Tout texte utilisateur/projet en français\n- Traiter les screenshots, mockups et attachments comme sources factuelles\n- Demander à l'utilisateur au lieu de deviner si le contexte manque\n- Vérifier les impacts API et les contrats front/back quand pertinent\n\n## Travail attendu\n\n- Limiter le travail à `%s`\n- Lister clairement les fichiers/zones impactés\n- Signaler les dépendances vers d'autres domaines\n- Mettre à jour la synthèse structurée ci-dessous\n\n## Synthèse structurée attendue\n\nRemplir ce bloc sans changer les labels.\n\n```yaml\nstatus: todo\nrepository: %s\nsummary:\n  done: []\n  decisions: []\n  risks: []\n  blockers: []\n  follow_up: []\nverification:\n  commands: []\n  manual_checks: []\nartifacts:\n  files: []\n  screenshots: []\n  attachments: []\n```\n", repository, manifest.Project, repository, manifest.BranchName, strings.Join(ids, ", "), repository, repository, repository)
}

func AgentFiles(manifest Manifest) []GeneratedFile {
	content := agentsMarkdown(manifest)
	return []GeneratedFile{{"AGENTS.md", content}, {"CLAUDE.md", content}, {filepath.Join(".claude", "CLAUDE.md"), content}, {filepath.Join(".cursor", "rules", "devworkflow.mdc"), "---\nalwaysApply: true\n---\n\n" + content}, {filepath.Join(".codex", "config.toml"), "# Configuration Codex locale au projet.\n# Les instructions d'exécution principales sont chargées depuis AGENTS.md dans ce workspace.\n"}, {filepath.Join(".github", "copilot-instructions.md"), content}}
}

type GeneratedFile struct {
	RelativePath string `json:"relativePath"`
	Content      string `json:"content"`
}

func agentsMarkdown(manifest Manifest) string {
	lines := make([]string, 0)
	for _, item := range manifest.ParentWorkItems() {
		suffix := ""
		if item.Type != nil || item.Title != nil {
			kind := "?"
			if item.Type != nil {
				kind = *item.Type
			}
			title := ""
			if item.Title != nil {
				title = *item.Title
			}
			suffix = strings.TrimRight(" ["+kind+"] "+title, " ")
		}
		lines = append(lines, "  - `#"+item.ID+"`"+suffix)
	}
	return fmt.Sprintf(`# Workspace DevWorkflow

Ce workspace est géré par DevWorkflow.

Contexte:

- Project: %s
- Work items:
%s

Actions DevWorkflow:

- ADO: assigned, item show, context ai et state set.
- Work: current, open, sync et preflight.
- Contenu: work item doing/add/remove, work repo add/latest et work PR start.
- Child tasks et handoffs: work task child create et work handoff validate.
- Cycle de vie: work commit, finish, teardown et prune.
- Base de données: schema, describe et query.

Règles:

1. Identifier le workspace courant avec l'action work current avant d'agir.
2. Lire chaque work item avec l'action ADO item show avant de coder.
3. Lire le contexte IA avec l'action ADO context ai avant d'agir sur le contexte ADO.
4. Utiliser les actions DB schema, describe et query quand le contexte base de données peut clarifier le changement.
5. Lire task.json, plan.md et tous les handoffs avant de modifier le code.
6. Exécuter l'action work preflight avant de coder.
7. Préserver les contrats machine et les labels métier exacts.
8. Limiter chaque changement au repository concerné.
9. Mettre à jour le handoff du repository après chaque lot cohérent.
10. Renseigner le bloc YAML structuré sans changer ses labels.
11. Vérifier les changements avec les commandes prévues dans le handoff.
12. Exécuter l'action work handoff validate avant de terminer.
13. Utiliser l'action work commit pour les commits intermédiaires.
14. Utiliser l'action work finish pour vérifier, pousser et ouvrir les pull requests.
15. Utiliser les actions work teardown ou prune pour le nettoyage.
`, "`"+manifest.Project+"`", strings.Join(lines, "\n"))
}
func WriteGeneratedFiles(workspace string, manifest Manifest) error {
	for _, file := range AgentFiles(manifest) {
		if err := writeFileAtomic(filepath.Join(workspace, file.RelativePath), []byte(file.Content), 0o644); err != nil {
			return err
		}
	}
	return nil
}

func RefreshGeneratedAgentFiles(root string) error {
	for _, workspace := range Discover(root) {
		if err := WriteGeneratedFiles(workspace.Path, workspace.Manifest); err != nil {
			return err
		}
	}
	return nil
}
