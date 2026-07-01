# HE `ReferencielService`

- Use: screen depends on HE referentials loaded by pôle logistique.
- Central service: `[REDACTED]`.
- Why: local cache by pole, `switchPoleLogistique(...)`, `recupererReferentiel(...)`, many `getXxx()` selectors.
- Rule: do not recreate hardcoded reference lists if matching `TypeReferentiel` already exists here.
- Simple modern selector use: `[REDACTED]`.
- Dropdown mapping example: `[REDACTED]`.
- Why: `getTypeEtape()`, `getTypeTacheTLD()`, `getTypeTrajet()` -> `DropdownOption`.
- Rich business form example: `[REDACTED]`.
- Why: `getMetiers()`, `getOrigine()`, `getNature()`, `getStatutReclamation()`, `getTypeGranit()`, `getTransporteur()`, `getFournisseur()` -> UI options.
- Code-source companion: `../he-back/references/pivot-enums-and-referentials.md`.
