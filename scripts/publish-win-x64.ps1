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
$binaryPath = Join-Path $outputPath "dw.exe"


New-Item -ItemType Directory -Force -Path $outputPath | Out-Null

$ldflags = "-s -w -X github.com/sachahjkl/dw/internal/buildinfo.Version=$Version -X github.com/sachahjkl/dw/internal/buildinfo.Commit=$Commit"
Push-Location $repoRoot
try {
    $env:CGO_ENABLED = "0"
    $env:GOOS = "windows"
    $env:GOARCH = "amd64"
    $env:GOTOOLCHAIN = "local"
    go build -trimpath -buildvcs=false -tags timetzdata -ldflags $ldflags -o $binaryPath ./cmd/dw
    if ($LASTEXITCODE -ne 0) {
        throw "go build failed with exit code $LASTEXITCODE"
    }
}
finally {
    Pop-Location
}

& $binaryPath version | Out-Null
if ($LASTEXITCODE -ne 0) {
    throw "dw version failed with exit code $LASTEXITCODE"
}

Compress-Archive -Path $binaryPath -DestinationPath $archivePath -Force

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
