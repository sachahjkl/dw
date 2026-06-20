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

. "$PSScriptRoot\Common.WorktreeTools.ps1"

New-SubjectWorktree -Family $Family -SubjectName $SubjectName -BranchName $BranchName -BaseRef $BaseRef -WorkspaceRoot $WorkspaceRoot -Only $Only -FrontOnly:$FrontOnly -BackOnly:$BackOnly
