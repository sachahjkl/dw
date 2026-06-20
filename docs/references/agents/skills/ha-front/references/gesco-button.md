# HA `gesco-button`

- Use: existing HA action button.
- API: `src/app/@shared/components/gesco-button/gesco-button.component.ts`.
- Why: `buttonType`, `iconType`, `iconPosition`, `onlyIcon`, `withoutIcon`, `blockTime`, `readOnlyAppearance`, `isLoading`, `routerLink`.
- Enums: `src/app/@shared/constants.ts`.
- Why: `GescoButtonType`, `GescoIconType`, `GescoIconPosition`, `GescoIconSize`.
- Catalog: `src/app/home/page-style-test/page-style-test.component.html`.
- UI companion: `src/app/home/page-style-test/page-style-test.component.ts`.
- Real use: `src/app/gesco/funeral-customization/funeral-customization.component.html`, `src/app/gesco/folders/folders-search-result/folders-search-result.component.html`.
- Rule: prefer this over ad hoc button or local click-throttle logic.
