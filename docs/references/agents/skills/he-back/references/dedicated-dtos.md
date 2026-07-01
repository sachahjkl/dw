# HE DTOs dédiés par endpoint

- Use: create or adjust HE endpoint contracts.
- Ref: `[REDACTED].Models/Dto/SearchDtos.cs`.
- Why: `SearchScope`, dedicated `sealed record` contracts, clean `SearchResultDto` aggregate.
- Related wider enums: `[REDACTED].Models/EnumRefPIvot.cs`.
- Rule: prefer clear `XyzRequest` / `XyzResponse` over old broad DTO. Keep SQL/Dapper details out.
