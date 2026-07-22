param(
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
        Write-Host "User PATH already configured: $PathToAdd"
        return
    }

    $newPath = ($parts + $PathToAdd) -join ';'
    [Environment]::SetEnvironmentVariable("Path", $newPath, "User")
    Write-Host "User PATH updated. Open a new terminal to use dw."
    Write-Host "Current session PATH updated too."
}

New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null

$assetUrl = if ($Version -eq "latest") {
    "https://github.com/$Repository/releases/latest/download/dw-win-x64.zip"
}
else {
    $releaseTag = if ($Version.StartsWith("v", [System.StringComparison]::OrdinalIgnoreCase)) {
        "v" + $Version.Substring(1)
    }
    else {
        "v$Version"
    }
    "https://github.com/$Repository/releases/download/$releaseTag/dw-win-x64.zip"
}

$temp = Join-Path ([System.IO.Path]::GetTempPath()) ("dw-install-" + [Guid]::NewGuid().ToString("N"))
$zip = Join-Path $temp "dw-win-x64.zip"
$extract = Join-Path $temp "extract"
New-Item -ItemType Directory -Force -Path $temp, $extract | Out-Null

try {
    Write-Host "Downloading $assetUrl..."
    Invoke-WebRequest -Uri $assetUrl -OutFile $zip -Headers @{ "User-Agent" = "dw-installer" }
    Expand-Archive -LiteralPath $zip -DestinationPath $extract -Force
    Copy-Item -Path (Join-Path $extract "dw.exe") -Destination (Join-Path $InstallDir "dw.exe") -Force
}
finally {
    Remove-Item -Recurse -Force $temp -ErrorAction SilentlyContinue
}

Write-Host "dw installed in $InstallDir"

if (-not $NoPathUpdate) {
    Add-UserPath -PathToAdd $InstallDir
}

& (Join-Path $InstallDir "dw.exe") version
