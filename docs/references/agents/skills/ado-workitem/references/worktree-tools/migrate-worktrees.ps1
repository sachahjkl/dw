param(
    [Parameter(Mandatory = $true)]
    [string]$Family,
    [string]$WorkspaceRoot,
    [switch]$IncludeLocked
)

. "$PSScriptRoot\Common.WorktreeTools.ps1"

Move-FamilyWorktreesIntoSubjectFolders -Family $Family -WorkspaceRoot $WorkspaceRoot -IncludeLocked:$IncludeLocked
