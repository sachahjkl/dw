# HE `ogf-form-field` et inputs

- Use: existing `ogf-form-input` screen or new modern HE form.
- Container contract: `projects/exploitation-core-ui/src/lib/components/ogf-form-input/form-field/ogf-form-field.component.ts`.
- Why: slots `ogf-label` / control / `ogf-error`, `above` / `inline` / `inline-right`, `required`, ARIA wiring.
- Typed infra: `projects/exploitation-core-ui/src/public-api`.
- Catalog: `src/app/demo/demo-form-input/demo-form-input.component.ts`, `.html`.
- Why: `ogfInput`, `ogf-date-input`, `ogfTimeInput`, `ogfAutocomplete`, `ogf-multiselect-dropdown`, `ogf-radio-group`, `ogfCheckbox`, `ogf-date-select`, `ogf-date-range-select`.
- Dense real screen: `src/app/modules/dossier/informations/popin-reclamation/popin-reclamation.component.html`.
- TS companion: `src/app/modules/dossier/informations/popin-reclamation/popin-reclamation.component.ts`.
- Why: `dropdownSingleSettings`, `dropdownFilterableSettings`, dedicated form value types, computed UI options.
- Rule: if touching `ogf-form-block`, read `legacy-form-blocks.md` instead.
