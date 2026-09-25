# Upgrade and Release System

GitHub Releases is the distribution channel for the standalone Linux x64 and Windows x64 executables. macOS is not supported.

The Go 1.26 release pipeline builds with `CGO_ENABLED=0`, smoke-tests each executable, and packages:

```text
dw-linux-x64.tar.gz   contains dw
dw-win-x64.zip        contains dw.exe
release.json          combined asset manifest
release.json.minisig  minisign signature of release.json
```

The platform release scripts calculate each archive's SHA256 and emit an intermediate manifest. When a release is enabled, GitHub Actions validates the Nix package, combines both platform artifacts into the stable `release.json`, signs it with minisign, tags the version, and publishes the archives, manifest and signature. GitHub is only the transport/CDN.

Implemented commands and options remain:

```text
dw upgrade --check
dw upgrade
```

The updater reads `updates` from `workflow.json`, calls GitHub Releases, downloads `release.json` and `release.json.minisig`, verifies the signature (including the trusted comment) against the public keys embedded in `internal/update/signature.go`, validates the selected archive's SHA256, and replaces the current executable for release-binary installs. An unsigned or badly signed release is rejected. Nix-managed installations continue to use Nix upgrade commands.

## Signing Key

The CI reads the unencrypted minisign secret key from the `MINISIGN_SECRET_KEY` GitHub secret. To rotate it, add the new public key to `TrustedPublicKeys`, ship a release signed with the old key, then switch the secret and remove the old public key in a later release.

## Manifest Shape

```json
{
  "schema": 1,
  "version": "2026.06.20.1",
  "commit": "abc1234",
  "channel": "stable",
  "assets": [
    {
      "rid": "linux-x64",
      "fileName": "dw-linux-x64.tar.gz",
      "sha256": "<archive-sha256>",
      "url": "https://github.com/owner/repo/releases/download/v2026.06.20.1/dw-linux-x64.tar.gz"
    },
    {
      "rid": "win-x64",
      "fileName": "dw-win-x64.zip",
      "sha256": "<archive-sha256>",
      "url": "https://github.com/owner/repo/releases/download/v2026.06.20.1/dw-win-x64.zip"
    }
  ]
}
```

The update source URL and channel are configurable.
