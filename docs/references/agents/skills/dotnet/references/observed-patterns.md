# Dotnet observé dans HE/HA

- HE: many ASP.NET Core controllers with `[ApiController]`, `[FromRoute]`, `[FromBody]`, `[ProducesResponseType]`; heavy Dapper/SQL Server; old `class` DTOs + newer `record` DTOs.
- HA: many `Request`/`Response` models; many business services and external integrations; contract style still mixed by module.
- Rule: respect local structure. New clean route/flow should use cleaner dedicated contracts than repo average.
