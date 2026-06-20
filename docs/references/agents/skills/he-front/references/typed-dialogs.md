# HE popins et dialogs typés

- Use: create, migrate, open HE popin/dialog.
- Minimal typed example: `src/app/shared/commentaire/popin-commentaire.component.ts`.
- Why: `OgfDialogTyped<Data, Result>` + `poppinButton` minimal pattern.
- Rich business example: `src/app/shared/popup-ajout-role/popup-ajout-role.component.ts`.
- Why: explicit `data`/`result`, multiple actions, child popin via `OgfDialogTypedService`.
- Rich form example: `src/app/modules/dossier/informations/popin-reclamation/popin-reclamation.component.ts`.
- Open infra: `projects/exploitation-core-ui/src/lib/components/ogf-dialog-typed/ogf-dialog-typed.service.ts`.
- Render infra + `poppinButton`: `projects/exploitation-core-ui/src/lib/components/ogf-popin-typed/ogf-popin-typed.component.ts`.
- Demo placements/content: `src/app/demo/demo-popinv2/demo-popinv2.component.html`.
- Rule: if zone still uses `MatDialog`, treat these files as migration target. Do not add new untyped popin.
