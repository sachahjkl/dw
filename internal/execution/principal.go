package execution

import (
	"fmt"
	"os/user"
	"runtime"
)

func CurrentPrincipal() (PrincipalID, error) {
	current, err := user.Current()
	if err != nil {
		return "", fmt.Errorf("execution.current-principal:%w", err)
	}
	if current.Uid == "" {
		return "", fmt.Errorf("execution.current-principal-missing-id")
	}
	prefix := "unix:"
	if runtime.GOOS == "windows" {
		prefix = "windows:"
	}
	return PrincipalID(prefix + current.Uid), nil
}
