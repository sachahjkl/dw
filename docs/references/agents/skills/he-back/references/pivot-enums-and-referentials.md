# HE enums Pivot et référentiels métier

- Use: HE business code already normalized in back.
- Main: `Ogf.Exploitation.CentreServeur.Models/EnumRefPIvot.cs`.
- Why: `TypeReferentiel`, `CodeMetier`, `CodeRole`, `CodeCategorieTypeTache`, `TypologieReclamation`, `CodeStatutTLD`, many more.
- Rule: check here before adding string code or duplicate local enum.
- DTO companion: `Ogf.Exploitation.CentreServeur.Models/Dto/SearchDtos.cs`.
