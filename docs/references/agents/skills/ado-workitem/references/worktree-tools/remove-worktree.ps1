param(
    [Parameter(Mandatory = $true)]
    [string]$Family,
    [Parameter(Mandatory = $true)]
    [string]$SubjectName,
    [string]$WorkspaceRoot,
    [switch]$DeleteBranch
)

. "$PSScriptRoot\Common.WorktreeTools.ps1"

Remove-SubjectWorktree -Family $Family -SubjectName $SubjectName -WorkspaceRoot $WorkspaceRoot -DeleteBranch:$DeleteBranch
