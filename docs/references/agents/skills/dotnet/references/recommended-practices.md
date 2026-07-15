# Dotnet recommandé long terme

- Prefer simple `sealed record` transport contracts when module style allows.
- Prefer dedicated `XyzRequest` / `XyzResponse` per endpoint.
- Reuse contract only if business meaning same.
- Avoid kitchen-sink DTOs.
- Avoid mixing persistence, domain, API model in one type.
