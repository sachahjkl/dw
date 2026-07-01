# HE services Dapper lisibles

- Use: service-layer logic with Dapper + SQL Server.
- Ref: `[REDACTED].Services/Services/GlobalSearchService.cs`.
- Why: input validation, connection open, `CommandDefinition`, Dapper projection to explicit DTOs.
- Methods: `SearchDossiersAsync`, `SearchTachesAsync`, `SearchRessourcesHumainesAsync`, `SearchRessourcesMateriellesAsync`.
- Rule: keep SQL projection here, not in public API contract.
