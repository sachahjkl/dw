# HE controllers ASP.NET fins

- Use: add or modify HE ASP.NET endpoint.
- Ref: `Ogf.Exploitation.CentreServeur.Api/Controllers/SearchController.cs`.
- Why: clear route, `[ApiController]`, `[ProducesResponseType]`, direct service delegation.
- Rule: controller should not hold SQL or heavy business orchestration.
