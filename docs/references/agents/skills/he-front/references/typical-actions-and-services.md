# HE actions typiques et services UI

- Use: user action with toast, block UI, resource, nearby UI side effects.
- Central service: `src/app/core/service/taches.service.ts`.
- Why: create/update/delete pattern with `toasSvc`, `extractHttpErrorMessage`, `blockUI`, signaled resources.
- Business form example: `src/app/modules/dossier/informations/popin-reclamation/popin-reclamation.component.ts`.
- Typed dialog infra: `projects/exploitation-core-ui/src/lib/components/ogf-dialog-typed/ogf-dialog-typed.service.ts`.
- Typed popin buttons: `projects/exploitation-core-ui/src/lib/components/ogf-popin-typed/ogf-popin-typed.component.ts`.
