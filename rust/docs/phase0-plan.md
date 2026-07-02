# Phase 0 Rust Feasibility

## Scope

Phase 0 se limite aux coutures les plus risquées:

1. auth Azure DevOps sur Windows
2. lecture work item expanded + comments + relations
3. SQL Server readonly
4. continuité de livraison et upgrade `.NET -> Rust`
5. séparation interactive vs non interactive

## Current Parity Status

- `.NET` reste l'implémentation de référence
- le workspace Rust est initialisé mais non branché à la livraison
- aucun résultat de service réel n'est encore validé depuis ce repo

## Implementation Changes In This Step

1. création d'un workspace `rust/`
2. création des crates alignées sur l'architecture cible
3. ajout d'un CLI minimal `dw-cli`
4. ajout des contrats et modèles de base pour les sorties structurées
5. ajout d'une garde SQL readonly côté Rust
6. ajout d'un squelette ADO pour auth env/PAT et URIs d'endpoints
7. ajout d'un premier slice Phase 1 pour `config show` et `task handoff-validate`

## Validation Evidence

- lecture du code `.NET` de référence pour `UpgradeCommand`, `AzureDevOpsClient`, `AzureDevOpsTokenProvider`, `SqlServerQueryService`, `SqlReadOnlyGuard`
- lecture des docs d'architecture `004-azure-devops.md`, `005-database.md`, `006-update-system.md`
- absence de `Cargo.toml` avant cette étape, confirmant qu'aucune base Rust n'existait encore
- impossibilité de compiler localement ici car `cargo` n'est pas installé dans cet environnement

## Known Gaps

1. pas de validation Windows réelle ici
2. pas de validation ADO réelle ici
3. pas de validation SQL Server réelle ici
4. pas de packaging Rust ni d'intégration CI activée
5. pas encore de harness side-by-side `.NET` vs Rust

## Go / No-Go

- Go provisoire pour continuer Phase 0 en environnement adapté
- No-Go pour passer en Phase 1 tant que les spikes Windows/ADO/SQL réels ne sont pas exécutés

## Immediate Commands To Run On A Windows Machine

```powershell
cargo run --manifest-path rust/Cargo.toml -p dw-cli -- phase0 status
cargo run --manifest-path rust/Cargo.toml -p dw-cli -- ado auth-env
cargo run --manifest-path rust/Cargo.toml -p dw-cli -- ado expanded-work-item --organization https://dev.azure.com/<org> --project <project> --work-item 12345
cargo run --manifest-path rust/Cargo.toml -p dw-cli -- db guard --sql "select top 5 * from dbo.Table"
```
