package paritytest_test

import (
	"bufio"
	"go/ast"
	"go/parser"
	"go/token"
	"os"
	"path/filepath"
	"sort"
	"strconv"
	"strings"
	"testing"
)

type parsedSource struct {
	path      string
	directory string
	file      *ast.File
}

func TestLiteralMessageIDsExistInEnglishCatalogs(t *testing.T) {
	root := filepath.Join("..", "..")
	known := readTOMLMessageIDs(t, filepath.Join(root, "locales", "active.en.toml"))
	used := make(map[string]string)
	files := token.NewFileSet()
	sources := parseProductionSources(t, files, filepath.Join(root, "internal"))
	constants := make(map[string]map[string]string)
	for _, source := range sources {
		if constants[source.directory] == nil {
			constants[source.directory] = make(map[string]string)
		}
		for name, value := range fileStringConstants(source.file) {
			constants[source.directory][name] = value
		}
	}
	for _, source := range sources {
		packageConstants := constants[source.directory]
		ast.Inspect(source.file, func(node ast.Node) bool {
			switch typed := node.(type) {
			case *ast.CompositeLit:
				collectCatalogEntries(typed, packageConstants, known)
				collectDynamicEventMessage(source.path, typed, files, used)
			case *ast.CallExpr:
				if id, ok := calledMessageID(typed, packageConstants); ok {
					used[id] = files.Position(typed.Pos()).String()
				}
			}
			return true
		})
	}
	for _, id := range missingMessageIDs(known, used) {
		t.Errorf("message ID %q at %s is absent from English catalogs", id, used[id])
	}
}

func TestMessageCoverageDetectsMissingID(t *testing.T) {
	known := map[string]struct{}{"known.message": {}}
	used := map[string]string{"known.message": "known.go:1", "missing.message": "missing.go:2"}
	missing := missingMessageIDs(known, used)
	if len(missing) != 1 || missing[0] != "missing.message" {
		t.Fatalf("missing messages = %#v", missing)
	}
}

func parseProductionSources(t *testing.T, files *token.FileSet, root string) []parsedSource {
	t.Helper()
	var sources []parsedSource
	err := filepath.WalkDir(root, func(path string, entry os.DirEntry, walkErr error) error {
		if walkErr != nil {
			return walkErr
		}
		if entry.IsDir() || !strings.HasSuffix(path, ".go") || strings.HasSuffix(path, "_test.go") {
			return nil
		}
		parsed, err := parser.ParseFile(files, path, nil, 0)
		if err != nil {
			return err
		}
		sources = append(sources, parsedSource{path: path, directory: filepath.Dir(path), file: parsed})
		return nil
	})
	if err != nil {
		t.Fatal(err)
	}
	return sources
}

func collectCatalogEntries(literal *ast.CompositeLit, constants map[string]string, known map[string]struct{}) {
	array, ok := literal.Type.(*ast.ArrayType)
	if !ok || !isL10nEntryType(array.Elt) {
		return
	}
	for _, element := range literal.Elts {
		entry, ok := element.(*ast.CompositeLit)
		if !ok {
			continue
		}
		for _, fieldElement := range entry.Elts {
			field, ok := fieldElement.(*ast.KeyValueExpr)
			if !ok {
				continue
			}
			key, ok := field.Key.(*ast.Ident)
			if !ok || key.Name != "ID" {
				continue
			}
			if id, ok := resolvedString(field.Value, constants); ok {
				known[id] = struct{}{}
			}
		}
	}
}

func collectDynamicEventMessage(path string, literal *ast.CompositeLit, files *token.FileSet, used map[string]string) {
	normalized := filepath.ToSlash(path)
	prefix, fieldName, typeName := "", "", ""
	switch {
	case strings.Contains(normalized, "/internal/workapp/") && isNamedType(literal.Type, "Event"):
		prefix, fieldName, typeName = "work.event.", "Kind", "Event"
	case strings.Contains(normalized, "/internal/workspace/") && isNamedType(literal.Type, "ActionEvent"):
		prefix, fieldName, typeName = "work.event.", "Type", "ActionEvent"
	}
	if typeName == "" {
		return
	}
	for _, element := range literal.Elts {
		field, ok := element.(*ast.KeyValueExpr)
		if !ok {
			continue
		}
		key, keyOK := field.Key.(*ast.Ident)
		value, valueOK := stringLiteral(field.Value)
		if keyOK && valueOK && key.Name == fieldName {
			used[prefix+value] = files.Position(field.Pos()).String()
		}
	}
}

func calledMessageID(call *ast.CallExpr, constants map[string]string) (string, bool) {
	if len(call.Args) == 0 {
		return "", false
	}
	if selector, ok := call.Fun.(*ast.SelectorExpr); ok {
		if selector.Sel.Name == "M" {
			if name, packageCall := selector.X.(*ast.Ident); packageCall && name.Name == "l10n" {
				return resolvedString(call.Args[0], constants)
			}
		}
		if selector.Sel.Name == "Text" {
			return resolvedString(call.Args[0], constants)
		}
	}
	if function, ok := call.Fun.(*ast.Ident); ok && function.Name == "localize" && len(call.Args) > 1 {
		return resolvedString(call.Args[1], constants)
	}
	return "", false
}

func isL10nEntryType(expression ast.Expr) bool {
	selector, ok := expression.(*ast.SelectorExpr)
	if !ok || selector.Sel.Name != "Entry" {
		return false
	}
	packageName, ok := selector.X.(*ast.Ident)
	return ok && packageName.Name == "l10n"
}

func missingMessageIDs(known map[string]struct{}, used map[string]string) []string {
	var missing []string
	for id := range used {
		if _, ok := known[id]; !ok {
			missing = append(missing, id)
		}
	}
	sort.Strings(missing)
	return missing
}

func isNamedType(expression ast.Expr, name string) bool {
	identifier, ok := expression.(*ast.Ident)
	return ok && identifier.Name == name
}

func fileStringConstants(file *ast.File) map[string]string {
	result := make(map[string]string)
	for _, declaration := range file.Decls {
		generic, ok := declaration.(*ast.GenDecl)
		if !ok || generic.Tok != token.CONST {
			continue
		}
		for _, specification := range generic.Specs {
			values, ok := specification.(*ast.ValueSpec)
			if !ok {
				continue
			}
			for index, name := range values.Names {
				if index < len(values.Values) {
					if value, literal := stringLiteral(values.Values[index]); literal {
						result[name.Name] = value
					}
				}
			}
		}
	}
	return result
}

func readTOMLMessageIDs(t *testing.T, path string) map[string]struct{} {
	t.Helper()
	file, err := os.Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer file.Close()
	result := make(map[string]struct{})
	scanner := bufio.NewScanner(file)
	for scanner.Scan() {
		line := strings.TrimSpace(scanner.Text())
		if !strings.HasPrefix(line, "\"") {
			continue
		}
		end := strings.Index(line[1:], "\"")
		if end >= 0 {
			result[line[1:1+end]] = struct{}{}
		}
	}
	if err = scanner.Err(); err != nil {
		t.Fatal(err)
	}
	return result
}

func resolvedString(expression ast.Expr, constants map[string]string) (string, bool) {
	if value, ok := stringLiteral(expression); ok {
		return value, true
	}
	identifier, ok := expression.(*ast.Ident)
	if !ok {
		return "", false
	}
	value, ok := constants[identifier.Name]
	return value, ok
}

func stringLiteral(expression ast.Expr) (string, bool) {
	literal, ok := expression.(*ast.BasicLit)
	if !ok || literal.Kind != token.STRING {
		return "", false
	}
	value, err := strconv.Unquote(literal.Value)
	return value, err == nil
}
