param(
    [string]$Version = "0.0.0-local",
    [string]$Commit = "dev",
    [string]$Output = "artifacts\win-x64",
    [string]$ReleaseBaseUrl = ""
)

$ErrorActionPreference = "Stop"
$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
$outputPath = Join-Path $repoRoot $Output

dotnet publish (Join-Path $repoRoot "src\Dw.Cli\Dw.Cli.csproj") `
    --configuration Release `
    --runtime win-x64 `
    --self-contained false `
    -p:PublishSingleFile=true `
    -p:DebugType=embedded `
    -p:VersionPrefix=$Version `
    -p:SourceRevisionId=$Commit `
    --output $outputPath

$exe = Join-Path $outputPath "dw.exe"
$hash = (Get-FileHash -Algorithm SHA256 -Path $exe).Hash.ToLowerInvariant()

$manifest = [ordered]@{
    schema = 1
    version = $Version
    commit = $Commit
    channel = "stable"
    assets = @(
        [ordered]@{
            rid = "win-x64"
            fileName = "dw.exe"
            sha256 = $hash
            url = if ([string]::IsNullOrWhiteSpace($ReleaseBaseUrl)) { "" } else { "$ReleaseBaseUrl/dw.exe" }
        }
    )
}

$manifest | ConvertTo-Json -Depth 8 | Set-Content -Path (Join-Path $outputPath "release.json") -Encoding utf8
Write-Host "Published $exe"
Write-Host "SHA256 $hash"
