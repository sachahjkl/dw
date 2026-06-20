# Update System

Updates use GitHub Releases as the distribution channel.

Each release should include:

```text
dw-win-x64.exe
release.json
sha256
```

`release.json` is the stable manifest. GitHub is only the transport/CDN.

Implemented first step:

```text
dw update check
```

It reads `updates` from `workflow.json`, calls GitHub Releases, downloads `release.json`, and prints available assets.

## Manifest Shape

```json
{
  "schema": 1,
  "version": "2026.06.20.1",
  "commit": "abc1234",
  "channel": "stable",
  "assets": [
    {
      "rid": "win-x64",
      "fileName": "dw-win-x64.exe",
      "sha256": "",
      "url": "https://github.com/owner/repo/releases/download/v2026.06.20.1/dw-win-x64.exe"
    }
  ]
}
```

The update source URL/channel should be configurable.
