namespace Dw.Cli.Templating;

internal static partial class Templates
{
    public static string PlanMd(string workItemId, string project) => $$"""
# Plan - Work item {{workItemId}}

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
