# HA `OperationResult` dans les services

- Use: service orchestrates multiple steps, must propagate business failure.
- Ref: `[REDACTED]/OnLinePaymentService.cs`.
- `GetAlmaPaymentStatusAsync`: simple external call returning `OperationResult<T>`.
- `ProcessAlmaPaymentResultAsync`: multi-step flow, validations, service calls, error propagation.
- Related codes: `[REDACTED]/ErrorCode.cs`.
- Rule: chain ops with `OperationResult`. Do not invent second result abstraction.
