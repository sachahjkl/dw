# HA écran de résultats, tabs et pagination

- Use: résultats + tabs + pagination + états loading/error.
- TS: `src/app/gesco/folders/folders-search-result/folders-search-result.component.ts`.
- Why: `OnPush`, many `input()`, many `computed()`, explicit derived labels/counts/states.
- HTML: `src/app/gesco/folders/folders-search-result/folders-search-result.component.html`.
- Why: `mat-tab-group`, pagination, loading/error branches, `gesco-title`.
- Constants: `src/app/gesco/constants.ts`.
- Why: `GescoRoute` for business navigation.
