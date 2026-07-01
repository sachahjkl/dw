# HA `OperationResult` dans les repositories

- Use: repository calls external service, must return `OperationResult<T>`.
- Simple example: `[REDACTED]/StoreLocatorRepository.cs`.
- Why: direct HTTP call, simple null check, `OperationResultFactory.Success` / `Failure`.
- Richer example: `[REDACTED]/ItemRepository.cs`.
- Why: propagate remote `OperationResult`, convert error codes, denser cache/HTTP logic.
- Related codes: `[REDACTED]/ErrorCode.cs`.
- Rule: repository stays readable adapter between external HTTP and local business contract.
