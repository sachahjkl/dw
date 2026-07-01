# HA formes de contrats API

- Use: choose between old DTO reuse vs dedicated contract.
- Old `Request`: `[REDACTED].Models/DTO/ReservationConvoiRequest.cs`.
- Simple `Response` record: `[REDACTED].Models/FakeCreasong/ApiStatusResponseDto.cs`.
- Nearby business enums: `[REDACTED].Models/StatusKind.cs`, `TransitionKey.cs`, `ServiceOrderType.cs`.
- Rule: new clean slice should be more explicit than `ReservationConvoiRequest`. No new catch-all DTO.
