//go:build windows

package secret

import "github.com/danieljoos/wincred"

var errCredentialNotFound = wincred.ErrElementNotFound

// rustCredentialTarget is the windows-native-keyring-store 1.1 default target mapping.
func rustCredentialTarget(service, account string) string {
	return account + "." + service
}

func setCredential(service, account, value string) error {
	credential := wincred.NewGenericCredential(rustCredentialTarget(service, account))
	credential.UserName = account
	credential.CredentialBlob = []byte(value)
	return credential.Write()
}

func getCredential(service, account string) (string, error) {
	credential, err := wincred.GetGenericCredential(rustCredentialTarget(service, account))
	if err != nil {
		return "", err
	}
	return string(credential.CredentialBlob), nil
}

func deleteCredential(service, account string) error {
	credential := wincred.NewGenericCredential(rustCredentialTarget(service, account))
	return credential.Delete()
}
