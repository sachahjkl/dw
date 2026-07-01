namespace Dw.Cli.Templating;

internal static partial class Templates
{
    public static string PlanMd(IReadOnlyList<WorkspaceWorkItem> workItems, string project) => $$"""
# Plan - Work items {{string.Join(", ", workItems.Select(item => $"#{item.Id}"))}}

Project: `{{project}}`

## Functional Summary

TODO

## Affected Repositories

- front: TODO
- back: TODO

## Code Analysis

TODO

## Technical Plan

TODO

## Risks

TODO

## Verification

TODO
""";
}
