# HA codes d'erreur partagés

- Use: HA repo/service must return failure code.
- Main: `[REDACTED]/ErrorCode.cs`.
- Why: central shared error code list.
- Rule: search here before adding new code. Use with `OperationResultFactory.Failure(...)`, not inline strings.
