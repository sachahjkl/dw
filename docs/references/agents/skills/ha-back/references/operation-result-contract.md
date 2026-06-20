# HA contrat `OperationResult`

- Use: module already returns `OperationResult` or `OperationResult<T>`.
- Root contract: `Ogf.Gesco.Common.Contracts/OperationResult.cs`.
- Why: `Success`, `Failure`, `Errors`, `Warnings`, `Informations`, `SuccessMessages`, `CopyMessagesTo`.
- Generic variant: `Ogf.Gesco.Common.Contracts/OperationResult.TEntity.cs`.
- Factory: `Ogf.Gesco.Common/OperationResultFactory.cs`.
- Related error codes: `Ogf.Gesco.Common/ErrorCode.cs`.
- Rule: in existing `OperationResult` flow, start here. Do not hand-build result objects.
