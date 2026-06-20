# HA modèles de résultat/erreur API front

- Use: HA API returns `success`, `errors`, `warnings`, maybe `data`.
- Root contract: `src/app/@shared/base-services/interfaces/errors-result.model.ts`.
- Why: `OperationResultModel<T>`.
- Single-result contract: `src/app/@shared/base-services/interfaces/unique-result.model.ts`.
- Why: `UniqueResultModel<TModel>`.
- HTTP service: `src/app/support/@services/support-service.service.ts`.
- Why: shows where API returns `UniqueResultModel<T>` vs `OperationResultModel`.
- Explicit success/error screen: `src/app/support/support-page/sinex-batch-update/sinex-batch-update.component.ts`.
- BasePage helper screen: `src/app/support/support-page/support-page.component.ts`.
- UI message companion: `src/app/home/page-style-test/page-style-test.component.ts`.
