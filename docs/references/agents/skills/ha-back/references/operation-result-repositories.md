# HA `OperationResult` dans les repositories

- Use: repository calls external service, must return `OperationResult<T>`.
- Simple example: `Ogf.HommageAgence.StoreLocator.Repositories/StoreLocatorRepository.cs`.
- Why: direct HTTP call, simple null check, `OperationResultFactory.Success` / `Failure`.
- Richer example: `Ogf.HommageAgence.ItemManagement.Repositories/ItemRepository.cs`.
- Why: propagate remote `OperationResult`, convert error codes, denser cache/HTTP logic.
- Related codes: `Ogf.Gesco.Common/ErrorCode.cs`.
- Rule: repository stays readable adapter between external HTTP and local business contract.
