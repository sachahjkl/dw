# HE `ReferencielService`

- Use: screen depends on HE referentials loaded by pôle logistique.
- Central service: `src/app/core/service/referenciel.service.ts`.
- Why: local cache by pole, `switchPoleLogistique(...)`, `recupererReferentiel(...)`, many `getXxx()` selectors.
- Rule: do not recreate hardcoded reference lists if matching `TypeReferentiel` already exists here.
- Simple modern selector use: `src/app/demo/demo-taches-cs.component.ts`.
- Dropdown mapping example: `src/app/modules/dossier/tld/popup-ajout-trajet-dto/popup-ajout-trajet-dto/popup-ajout-trajet-dto.component.ts`.
- Why: `getTypeEtape()`, `getTypeTacheTLD()`, `getTypeTrajet()` -> `DropdownOption`.
- Rich business form example: `src/app/modules/dossier/informations/popin-reclamation/popin-reclamation.component.ts`.
- Why: `getMetiers()`, `getOrigine()`, `getNature()`, `getStatutReclamation()`, `getTypeGranit()`, `getTransporteur()`, `getFournisseur()` -> UI options.
- Code-source companion: `../he-back/references/pivot-enums-and-referentials.md`.
