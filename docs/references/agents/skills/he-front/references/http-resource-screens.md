# HE écrans `createHttpResource`

- Use: screen loads remote data with signals, not scattered subscriptions.
- Business screen: `src/app/shared/popin-vue-simulateur/popin-vue-simulateur.component.ts`.
- Why: `createHttpResource`, `reload()`, `loading` / `value` / `error`, `effect()` sync.
- Demo screen: `src/app/demo/demo-taches-cs.component.ts`.
- Why: user-driven filters, explicit loading/error branches, resource projection in template.
- Companion service: `src/app/core/service/taches.service.ts`.
- Why: key `signal` + `createHttpResource` + `setXFilters()` + action methods with toast / block UI.
- Rule: if zone already uses `createHttpResource`, stay in that pattern.
