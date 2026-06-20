# HA formes de contrats API

- Use: choose between old DTO reuse vs dedicated contract.
- Old `Request`: `Ogf.Gesco.FolderManagement.Models/DTO/ReservationConvoiRequest.cs`.
- Simple `Response` record: `Ogf.Gesco.FolderManagement.Models/FakeCreasong/ApiStatusResponseDto.cs`.
- Nearby business enums: `Ogf.Gesco.FolderManagement.Models/StatusKind.cs`, `TransitionKey.cs`, `ServiceOrderType.cs`.
- Rule: new clean slice should be more explicit than `ReservationConvoiRequest`. No new catch-all DTO.
