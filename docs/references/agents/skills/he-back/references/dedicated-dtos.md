# HE DTOs dédiés par endpoint

- Use: create or adjust HE endpoint contracts.
- Ref: `Ogf.Exploitation.CentreServeur.Models/Dto/SearchDtos.cs`.
- Why: `SearchScope`, dedicated `sealed record` contracts, clean `SearchResultDto` aggregate.
- Related wider enums: `Ogf.Exploitation.CentreServeur.Models/EnumRefPIvot.cs`.
- Rule: prefer clear `XyzRequest` / `XyzResponse` over old broad DTO. Keep SQL/Dapper details out.
