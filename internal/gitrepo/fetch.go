package gitrepo

import "context"

// FetchAnchor refreshes the remote-tracking branches of a bare anchor.
func (client Client) FetchAnchor(ctx context.Context, anchorPath RepositoryPath) error {
	return client.fetchWithFallback(ctx, anchorPath, nil, nil)
}
