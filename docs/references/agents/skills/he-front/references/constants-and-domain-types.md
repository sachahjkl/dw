# HE constantes, types métier et enums front

- Use: business code, tabs, TLD popup, location referential, constant-driven logic.
- Typed tabs: `src/app/shared/types/TabKeys.ts`.
- Why: `as const` + derived type + `isTabKey` guard.
- Location referential + enums: `src/app/shared/referentiel-lieux-wrapper/rl.model.ts`.
- Why: `TypeEtablissement`, `Religion`, `SearchMode`, labels.
- Shared constants: `src/app/shared/constantes.ts`.
- Why: hour bounds, drag-drop tolerance, reused business ID lists.
- Strongly typed TLD editor state: `src/app/modules/dossier/tld/popup-ajout-trajet-dto/popup-ajout-trajet-dto.types.ts`.
- Local business code example: `src/app/modules/dossier/tld/popup-ajout-trajet-dto/popup-ajout-trajet-dto/popup-ajout-trajet-dto.component.ts`.
