param(
    [string]$Source,
    [string]$Repository = "sachahjkl/dw",
    [string]$Version = "latest",
    [string]$InstallDir = "$env:LOCALAPPDATA\DevWorkflow\bin",
    [switch]$NoPathUpdate
)

$ErrorActionPreference = "Stop"

function Add-UserPath {
    param([string]$PathToAdd)

    $env:Path = "$PathToAdd;$env:Path"

    $current = [Environment]::GetEnvironmentVariable("Path", "User")
    $parts = @()
    if (-not [string]::IsNullOrWhiteSpace($current)) {
        $parts = $current -split ';' | Where-Object { -not [string]::IsNullOrWhiteSpace($_) }
    }

    if ($parts -contains $PathToAdd) {
        Write-Host "PATH utilisateur deja configure: $PathToAdd"
        return
    }

    $newPath = ($parts + $PathToAdd) -join ';'
    [Environment]::SetEnvironmentVariable("Path", $newPath, "User")
    Write-Host "PATH utilisateur mis a jour. Ouvre un nouveau terminal pour utiliser dw."
    Write-Host "PATH de la session courante mis a jour aussi."
}

New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null

if ([string]::IsNullOrWhiteSpace($Source)) {
    $repoRoot = if ([string]::IsNullOrWhiteSpace($PSScriptRoot)) { $null } else { Resolve-Path (Join-Path $PSScriptRoot "..") -ErrorAction SilentlyContinue }
    $project = if ($repoRoot) { Join-Path $repoRoot "src\Dw.Cli\Dw.Cli.csproj" } else { $null }

    if ($project -and (Test-Path $project)) {
        $publishDir = Join-Path $repoRoot "artifacts\install"

        dotnet publish $project `
            --configuration Release `
            --runtime win-x64 `
            --self-contained false `
            -p:PublishSingleFile=true `
            -p:DebugType=embedded `
            --output $publishDir

        $Source = $publishDir
    }
    else {
        $assetUrl = if ($Version -eq "latest") {
            "https://github.com/$Repository/releases/latest/download/dw-win-x64.zip"
        }
        else {
            "https://github.com/$Repository/releases/download/$Version/dw-win-x64.zip"
        }

        $temp = Join-Path ([System.IO.Path]::GetTempPath()) ("dw-install-" + [Guid]::NewGuid().ToString("N"))
        $zip = Join-Path $temp "dw-win-x64.zip"
        $extract = Join-Path $temp "extract"
        New-Item -ItemType Directory -Force -Path $temp, $extract | Out-Null

        Write-Host "Telechargement $assetUrl..."
        Invoke-WebRequest -Uri $assetUrl -OutFile $zip -Headers @{ "User-Agent" = "dw-installer" }
        Expand-Archive -LiteralPath $zip -DestinationPath $extract -Force
        $Source = $extract
    }
}

if (-not (Test-Path $Source)) {
    throw "Source introuvable: $Source"
}

if ((Get-Item $Source).PSIsContainer) {
    Copy-Item -Path (Join-Path $Source "*") -Destination $InstallDir -Recurse -Force
}
else {
    Copy-Item -Path $Source -Destination (Join-Path $InstallDir "dw.exe") -Force
}

Write-Host "dw installe dans $InstallDir"

if (-not $NoPathUpdate) {
    Add-UserPath -PathToAdd $InstallDir
}

& (Join-Path $InstallDir "dw.exe") version
