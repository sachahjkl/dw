# HA contrat `OperationResult`

- Use: module already returns `OperationResult` or `OperationResult<T>`.
- Root contract: `[REDACTED]/OperationResult.cs`.
- Why: `Success`, `Failure`, `Errors`, `Warnings`, `Informations`, `SuccessMessages`, `CopyMessagesTo`.
- Generic variant: `[REDACTED]/OperationResult.TEntity.cs`.
- Factory: `[REDACTED]/OperationResultFactory.cs`.
- Related error codes: `[REDACTED]/ErrorCode.cs`.
- Rule: in existing `OperationResult` flow, start here. Do not hand-build result objects.
