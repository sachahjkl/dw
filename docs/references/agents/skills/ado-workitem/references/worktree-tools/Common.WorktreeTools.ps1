Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Write-Stage {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Message
    )

    Write-Host "[worktree] $Message"
}

function Format-TerminalPathLink {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Path
    )

    $fullPath = [System.IO.Path]::GetFullPath($Path)
    $uri = ([System.Uri]::new($fullPath)).AbsoluteUri
    $esc = [char]27
    $bel = [char]7

    return "$esc]8;;$uri$bel$fullPath$esc]8;;$bel"
}

function Get-WorktreeSkillRoot {
    return $PSScriptRoot
}

function Resolve-WorkspaceRoot {
    param(
        [string]$StartPath = (Get-Location).Path
    )

    $current = $StartPath

    while ($true) {
        if (Test-Path -LiteralPath (Join-Path $current '.anchors')) {
            return $current
        }

        if (Test-Path -LiteralPath (Join-Path $current 'ws')) {
            return (Join-Path $current 'ws')
        }

        $parent = Split-Path -Parent $current
        if ([string]::IsNullOrWhiteSpace($parent) -or $parent -eq $current) {
            break
        }

        $current = $parent
    }

    throw "Impossible de resoudre le workspace root (dossier ws contenant .anchors) depuis $StartPath."
}

function Get-WorkspaceRoot {
    param(
        [string]$WorkspaceRoot
    )

    if (-not [string]::IsNullOrWhiteSpace($WorkspaceRoot)) {
        $candidate = [System.IO.Path]::GetFullPath($WorkspaceRoot)

        if (Test-Path -LiteralPath (Join-Path $candidate '.anchors')) {
            return $candidate
        }

        $candidateWs = Join-Path $candidate 'ws'
        if (Test-Path -LiteralPath $candidateWs) {
            Write-Stage "WorkspaceRoot legacy detecte, normalisation vers: $(Format-TerminalPathLink -Path $candidateWs)"
            return $candidateWs
        }

        return $candidate
    }

    return (Resolve-WorkspaceRoot)
}

function Get-AnchorRoot {
    param(
        [string]$WorkspaceRoot
    )

    return (Join-Path (Get-WorkspaceRoot -WorkspaceRoot $WorkspaceRoot) '.anchors')
}

function Get-WorkspaceTreesRoot {
    param(
        [string]$WorkspaceRoot
    )

    return (Get-WorkspaceRoot -WorkspaceRoot $WorkspaceRoot)
}

function Ensure-Directory {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Path
    )

    if (-not (Test-Path -LiteralPath $Path)) {
        New-Item -ItemType Directory -Path $Path | Out-Null
    }
}

function Convert-BranchNameToSubjectName {
    param(
        [Parameter(Mandatory = $true)]
        [string]$BranchName
    )

    $normalized = $BranchName.Trim() -replace '\\', '/'

    if ($normalized -match '^(?<type>[^/]+)/(?<slug>.+)$') {
        return "$($Matches['type'])-$($Matches['slug'] -replace '/', '-')"
    }

    return ($normalized -replace '/', '-')
}

function Get-WorktreeToolsConfig {
    $configPath = Join-Path $PSScriptRoot 'worktree-tools.config.json'

    if (-not (Test-Path -LiteralPath $configPath)) {
        throw "Fichier de configuration introuvable: $configPath"
    }

    return (Get-Content -LiteralPath $configPath -Raw | ConvertFrom-Json)
}

function Get-FamilyConfig {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Family,
        [string]$WorkspaceRoot
    )

    $familyKey = $Family.ToLowerInvariant()
    $configData = Get-WorktreeToolsConfig
    $familyData = $configData.families.$familyKey

    if ($null -eq $familyData) {
        throw "Configuration introuvable pour la famille $Family"
    }

    $resolvedWorkspaceRoot = Get-WorkspaceRoot -WorkspaceRoot $WorkspaceRoot
    $anchorRoot = Get-AnchorRoot -WorkspaceRoot $resolvedWorkspaceRoot
    $workspaceTreesRoot = Get-WorkspaceTreesRoot -WorkspaceRoot $resolvedWorkspaceRoot

    $repos = @{}
    foreach ($repoKey in $familyData.PSObject.Properties.Name) {
        if ($repoKey -eq 'workspaceFolder') {
            continue
        }
        $repoData = $familyData.$repoKey
        $repos[$repoKey] = [pscustomobject]@{
            Anchor = Join-Path $anchorRoot $repoData.anchorName
            Remote = $repoData.remote
        }
    }

    return [pscustomobject]@{
        Family = $familyKey
        WorkspaceFolder = $familyData.workspaceFolder
        FrontAnchor = if ($repos.ContainsKey('front')) { $repos['front'].Anchor } else { $null }
        BackAnchor = if ($repos.ContainsKey('back')) { $repos['back'].Anchor } else { $null }
        FrontRemote = if ($repos.ContainsKey('front')) { $repos['front'].Remote } else { $null }
        BackRemote = if ($repos.ContainsKey('back')) { $repos['back'].Remote } else { $null }
        Repos = $repos
        WorkspaceRoot = Join-Path $workspaceTreesRoot $familyData.workspaceFolder
        Root = $resolvedWorkspaceRoot
    }
}

function Ensure-BareAnchor {
    param(
        [Parameter(Mandatory = $true)]
        [string]$AnchorPath,
        [Parameter(Mandatory = $true)]
        [string]$RemoteUrl
    )

    Ensure-Directory -Path (Split-Path -Parent $AnchorPath)

    if (-not (Test-Path -LiteralPath $AnchorPath)) {
        Write-Stage "Anchor absent, clone bare: $(Format-TerminalPathLink -Path $AnchorPath)"
        git clone --bare "$RemoteUrl" "$AnchorPath"
        if (-not $?) {
            throw "Impossible de cloner l'anchor bare $AnchorPath"
        }
    } else {
        Write-Stage "Anchor existant: $(Format-TerminalPathLink -Path $AnchorPath)"
    }
}

function Ensure-FamilyAnchors {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Family,
        [string]$WorkspaceRoot,
        [string[]]$Only
    )

    $config = Get-FamilyConfig -Family $Family -WorkspaceRoot $WorkspaceRoot
    $repoKeys = if ($Only -and $Only.Count -gt 0) { $Only } else { $config.Repos.Keys }

    foreach ($repoKey in $repoKeys) {
        $repo = $config.Repos[$repoKey]
        if ($null -eq $repo) {
            throw "Repo '$repoKey' introuvable dans la configuration de la famille $Family"
        }
        Ensure-BareAnchor -AnchorPath $repo.Anchor -RemoteUrl $repo.Remote
    }
}

function Get-SubjectRoot {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Family,
        [Parameter(Mandatory = $true)]
        [string]$SubjectName,
        [string]$WorkspaceRoot
    )

    $config = Get-FamilyConfig -Family $Family -WorkspaceRoot $WorkspaceRoot
    return (Join-Path $config.WorkspaceRoot $SubjectName)
}

function Invoke-GitAnchor {
    param(
        [Parameter(Mandatory = $true)]
        [string]$AnchorPath,
        [Parameter(Mandatory = $true)]
        [string[]]$Arguments
    )

    & git --git-dir="$AnchorPath" @Arguments
    if (-not $?) {
        throw "Commande git en echec sur l'anchor $AnchorPath : $($Arguments -join ' ')"
    }
}

function New-SubjectWorktree {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Family,
        [Parameter(Mandatory = $true)]
        [string]$SubjectName,
        [string]$BranchName,
        [string]$BaseRef = 'origin/develop',
        [string]$WorkspaceRoot,
        [string[]]$Only,
        [switch]$FrontOnly,
        [switch]$BackOnly
    )

    $effectiveBranchName = if ([string]::IsNullOrWhiteSpace($BranchName)) { $SubjectName } else { $BranchName }

    $selectedOnly = $Only
    if ($FrontOnly.IsPresent) {
        $selectedOnly = @('front')
    }
    if ($BackOnly.IsPresent) {
        $selectedOnly = @('back')
    }

    Write-Stage "Initialisation sujet '$SubjectName' pour famille '$Family' depuis '$BaseRef'"
    if ($effectiveBranchName -ne $SubjectName) {
        Write-Stage "Nom de branche: $effectiveBranchName"
    }
    if ($selectedOnly -and $selectedOnly.Count -gt 0) {
        Write-Stage "Repos selectionnes: $($selectedOnly -join ', ')"
    }
    Write-Stage "Verification des anchors bare"
    Ensure-FamilyAnchors -Family $Family -WorkspaceRoot $WorkspaceRoot -Only $selectedOnly
    $config = Get-FamilyConfig -Family $Family -WorkspaceRoot $WorkspaceRoot
    $subjectRoot = Get-SubjectRoot -Family $Family -SubjectName $SubjectName -WorkspaceRoot $WorkspaceRoot
    Write-Stage "Workspace root: $(Format-TerminalPathLink -Path $config.Root)"
    Write-Stage "Dossier sujet: $(Format-TerminalPathLink -Path $subjectRoot)"
    Ensure-Directory -Path $config.WorkspaceRoot
    Ensure-Directory -Path $subjectRoot

    $repoKeys = if ($selectedOnly -and $selectedOnly.Count -gt 0) { $selectedOnly } else { $config.Repos.Keys }

    foreach ($repoKey in $repoKeys) {
        $repo = $config.Repos[$repoKey]
        if ($null -eq $repo) {
            throw "Repo '$repoKey' introuvable dans la configuration de la famille $Family"
        }

        $repoPath = Join-Path $subjectRoot $repoKey
        Write-Stage "$($repoKey): fetch --all --prune sur $(Format-TerminalPathLink -Path $repo.Anchor)"
        Invoke-GitAnchor -AnchorPath $repo.Anchor -Arguments @('fetch', '--all', '--prune')
        if (-not (Test-Path -LiteralPath $repoPath)) {
            Write-Stage "$($repoKey): creation worktree $(Format-TerminalPathLink -Path $repoPath)"
            Invoke-GitAnchor -AnchorPath $repo.Anchor -Arguments @('worktree', 'add', '-b', $effectiveBranchName, $repoPath, $BaseRef)
        } else {
            Write-Stage "$($repoKey): worktree deja present, creation ignoree: $(Format-TerminalPathLink -Path $repoPath)"
        }
    }

    Write-Stage "Initialisation terminee"
}

function Remove-SubjectWorktree {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Family,
        [Parameter(Mandatory = $true)]
        [string]$SubjectName,
        [string]$WorkspaceRoot,
        [switch]$DeleteBranch
    )

    $config = Get-FamilyConfig -Family $Family -WorkspaceRoot $WorkspaceRoot
    $subjectRoot = Get-SubjectRoot -Family $Family -SubjectName $SubjectName -WorkspaceRoot $WorkspaceRoot

    Write-Stage "Suppression sujet '$SubjectName' pour famille '$Family'"
    Write-Stage "Dossier sujet: $(Format-TerminalPathLink -Path $subjectRoot)"

    foreach ($repoKey in $config.Repos.Keys) {
        $repo = $config.Repos[$repoKey]
        $repoPath = Join-Path $subjectRoot $repoKey

        if (Test-Path -LiteralPath $repoPath) {
            Write-Stage "$($repoKey): suppression worktree $(Format-TerminalPathLink -Path $repoPath)"
            Invoke-GitAnchor -AnchorPath $repo.Anchor -Arguments @('worktree', 'remove', $repoPath)
            if ($DeleteBranch.IsPresent) {
                Write-Stage "$($repoKey): suppression branche locale $SubjectName"
                Invoke-GitAnchor -AnchorPath $repo.Anchor -Arguments @('branch', '-D', $SubjectName)
            }
        } else {
            Write-Stage "$($repoKey): aucun worktree a supprimer"
        }
    }

    if ((Test-Path -LiteralPath $subjectRoot) -and -not (Get-ChildItem -LiteralPath $subjectRoot -Force | Select-Object -First 1)) {
        Write-Stage "Suppression dossier sujet vide: $(Format-TerminalPathLink -Path $subjectRoot)"
        Remove-Item -LiteralPath $subjectRoot
    }

    Write-Stage "Suppression terminee"
}

function Get-WorktreeList {
    param(
        [Parameter(Mandatory = $true)]
        [string]$RepoPath
    )

    $output = & git -C "$RepoPath" worktree list --porcelain
    if (-not $?) {
        throw "Impossible de lire les worktrees depuis $RepoPath"
    }

    $items = @()
    $current = $null

    foreach ($line in $output) {
        if ($line -like 'worktree *') {
            if ($null -ne $current) {
                $items += [pscustomobject]$current
            }

            $current = @{
                Path = $line.Substring(9)
                Branch = $null
                Locked = $false
                LockReason = $null
            }
            continue
        }

        if ($null -eq $current) {
            continue
        }

        if ($line -like 'branch *') {
            $current.Branch = $line.Substring(7) -replace '^refs/heads/', ''
            continue
        }

        if ($line -like 'locked*') {
            $current.Locked = $true
            $current.LockReason = $line
        }
    }

    if ($null -ne $current) {
        $items += [pscustomobject]$current
    }

    return $items
}

function Move-TrackedWorktree {
    param(
        [Parameter(Mandatory = $true)]
        [string]$RepoPath,
        [Parameter(Mandatory = $true)]
        [string]$OldPath,
        [Parameter(Mandatory = $true)]
        [string]$NewPath
    )

    Ensure-Directory -Path (Split-Path -Parent $NewPath)
    & git -C "$RepoPath" worktree move "$OldPath" "$NewPath"
    if (-not $?) {
        throw "Impossible de deplacer le worktree $OldPath vers $NewPath"
    }
}

function Move-FamilyWorktreesIntoSubjectFolders {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Family,
        [string]$WorkspaceRoot,
        [switch]$IncludeLocked
    )

    $config = Get-FamilyConfig -Family $Family -WorkspaceRoot $WorkspaceRoot
    Write-Stage "Migration famille '$Family' vers les dossiers sujet"
    Ensure-Directory -Path $config.WorkspaceRoot

    throw 'La migration des anciens roots legacy n''est plus supportee par ce script. Utiliser uniquement les worktrees sous ws/ et les anchors bare sous ws/.anchors.'
}
