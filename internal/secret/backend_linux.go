//go:build linux

package secret

import keyring "github.com/zalando/go-keyring"

var errCredentialNotFound = keyring.ErrNotFound

func setCredential(service, account, value string) error {
	return keyring.Set(service, account, value)
}

func getCredential(service, account string) (string, error) {
	return keyring.Get(service, account)
}

func deleteCredential(service, account string) error {
	return keyring.Delete(service, account)
}
