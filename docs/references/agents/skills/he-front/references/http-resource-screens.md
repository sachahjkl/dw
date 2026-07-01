# HE écrans `createHttpResource`

- Use: screen loads remote data with signals, not scattered subscriptions.
- Business screen: `[REDACTED]`.
- Why: `createHttpResource`, `reload()`, `loading` / `value` / `error`, `effect()` sync.
- Demo screen: `[REDACTED]`.
- Why: user-driven filters, explicit loading/error branches, resource projection in template.
- Companion service: `[REDACTED]`.
- Why: key `signal` + `createHttpResource` + `setXFilters()` + action methods with toast / block UI.
- Rule: if zone already uses `createHttpResource`, stay in that pattern.
