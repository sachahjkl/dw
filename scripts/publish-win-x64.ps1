param(
    [string]$Version = "0.0.0-local",
    [string]$Commit = "dev",
    [string]$Output = "artifacts\win-x64",
    [string]$ReleaseBaseUrl = ""
)

$ErrorActionPreference = "Stop"
$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
if ([System.IO.Path]::IsPathRooted($Output)) {
    $outputPath = $Output
}
else {
    $outputPath = Join-Path $repoRoot $Output
}
$archiveName = "dw-win-x64.zip"
$archivePath = Join-Path $outputPath $archiveName

New-Item -ItemType Directory -Force -Path $outputPath | Out-Null

Push-Location $repoRoot
try {
    $env:DW_COMMIT = $Commit
    cargo build --locked --release -p dw-cli
    if ($LASTEXITCODE -ne 0) {
        throw "cargo build failed with exit code $LASTEXITCODE"
    }
}
finally {
    Pop-Location
}

Copy-Item -Force (Join-Path $repoRoot "target\release\dw-cli.exe") (Join-Path $outputPath "dw.exe")
Compress-Archive -Path (Join-Path $outputPath "dw.exe") -DestinationPath $archivePath -Force

$hash = (Get-FileHash -Algorithm SHA256 -Path $archivePath).Hash.ToLowerInvariant()
$url = if ([string]::IsNullOrWhiteSpace($ReleaseBaseUrl)) { "" } else { "$ReleaseBaseUrl/$archiveName" }

$manifest = [ordered]@{
    schema = 1
    version = $Version
    commit = $Commit
    channel = "stable"
    assets = @(
        [ordered]@{
            rid = "win-x64"
            fileName = $archiveName
            sha256 = $hash
            url = $url
        }
    )
}

$manifest | ConvertTo-Json -Depth 8 | Set-Content -Path (Join-Path $outputPath "release.json") -Encoding utf8
Write-Host "Published $archivePath"
Write-Host "SHA256 $hash"
