# HE enums Pivot et référentiels métier

- Use: HE business code already normalized in back.
- Main: `[REDACTED].Models/EnumRefPIvot.cs`.
- Why: `TypeReferentiel`, `CodeMetier`, `CodeRole`, `CodeCategorieTypeTache`, `TypologieReclamation`, `CodeStatutTLD`, many more.
- Rule: check here before adding string code or duplicate local enum.
- DTO companion: `[REDACTED].Models/Dto/SearchDtos.cs`.
