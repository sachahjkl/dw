package secret

import (
	"context"
	"fmt"
	"os"
	"path/filepath"
	"sort"
	"strings"

	"github.com/sachahjkl/dw/internal/contract"
	"github.com/sachahjkl/dw/internal/l10n"
	"github.com/sachahjkl/dw/internal/wirejson"
)

type Storage string

const StorageSystemKeyring Storage = "system-keyring"

type SetReport struct {
	Key         contract.SecretKey `json:"key"`
	Storage     Storage            `json:"storage"`
	ValueMasked bool               `json:"value_masked"`
}

type GetReport struct {
	Key         contract.SecretKey `json:"key"`
	Exists      bool               `json:"exists"`
	ValueMasked bool               `json:"value_masked"`
}

type DeleteReport struct {
	Key              contract.SecretKey `json:"key"`
	DeletedIfPresent bool               `json:"deleted_if_present"`
}

type ListReport struct {
	Root     string     `json:"root"`
	Items    []ListItem `json:"items"`
	Warnings []string   `json:"warnings"`
}

type ListItem struct {
	Key         contract.SecretKey `json:"key"`
	Exists      bool               `json:"exists"`
	ValueMasked bool               `json:"valueMasked"`
	References  []string           `json:"references"`
}

func SetSecret(ctx context.Context, store contract.SecretStore, key contract.SecretKey, value contract.SecretValue) (SetReport, error) {
	if err := store.Set(ctx, key, value); err != nil {
		return SetReport{}, err
	}
	return SetReport{Key: key, Storage: StorageSystemKeyring, ValueMasked: true}, nil
}

func GetSecret(ctx context.Context, store contract.SecretStore, key contract.SecretKey) (GetReport, error) {
	_, exists, err := store.Get(ctx, key)
	if err != nil {
		return GetReport{}, err
	}
	return GetReport{Key: key, Exists: exists, ValueMasked: true}, nil
}

func DeleteSecret(ctx context.Context, store contract.SecretStore, key contract.SecretKey) (DeleteReport, error) {
	if _, err := store.Delete(ctx, key); err != nil {
		return DeleteReport{}, err
	}
	return DeleteReport{Key: key, DeletedIfPresent: true}, nil
}

// Discover reports only keys referenced by configuration. OS keyrings intentionally do not
// provide a portable enumeration API, and enumerating unrelated credentials would be unsafe.
func Discover(ctx context.Context, root string, store contract.SecretStore) (ListReport, error) {
	references := make(map[contract.SecretKey][]string)
	warnings := make([]string, 0)
	for _, name := range [...]string{"databases.json", "projects.json", "workflow.json"} {
		path := filepath.Join(root, "config", name)
		text, err := os.ReadFile(path)
		if err != nil {
			warnings = append(warnings, l10n.Render(l10n.M("secret.config-read-failed", l10n.A("path", path), l10n.A("detail", err))))
			continue
		}
		document, err := wirejson.Parse(text)
		if err != nil {
			warnings = append(warnings, l10n.Render(l10n.M("secret.config-parse-failed", l10n.A("path", path), l10n.A("detail", err))))
			continue
		}
		collectReferences(&document, name, references)
	}

	keys := make([]contract.SecretKey, 0, len(references))
	for key := range references {
		keys = append(keys, key)
	}
	sort.Slice(keys, func(left, right int) bool { return keys[left] < keys[right] })
	items := make([]ListItem, 0, len(keys))
	for _, key := range keys {
		_, exists, err := store.Get(ctx, key)
		if err != nil {
			return ListReport{}, err
		}
		items = append(items, ListItem{
			Key:         key,
			Exists:      exists,
			ValueMasked: true,
			References:  references[key],
		})
	}
	return ListReport{Root: root, Items: items, Warnings: warnings}, nil
}

func collectReferences(value *wirejson.Value, path string, references map[contract.SecretKey][]string) {
	switch value.Kind() {
	case wirejson.Object:
		members, _ := value.Members()
		for index := range members {
			member := &members[index]
			childPath := path + "/" + member.Name
			if member.Name == "credentialKey" || member.Name == "gitCredentialSecret" {
				if secretKey, ok := member.Value.AsString(); ok && strings.TrimSpace(secretKey) != "" {
					typedKey := contract.SecretKey(secretKey)
					references[typedKey] = append(references[typedKey], childPath)
					continue
				}
			}
			collectReferences(&member.Value, childPath, references)
		}
	case wirejson.Array:
		values, _ := value.ArrayValues()
		for index := range values {
			collectReferences(&values[index], fmt.Sprintf("%s/%d", path, index), references)
		}
	}
}
