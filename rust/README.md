# Rust Rewrite Workspace

Ce dossier contient le rewrite Rust incrémental de `dw`.

Règles de base:

- l'implémentation `.NET` en racine reste la source de vérité tant que les gates de parité ne sont pas passés
- le Rust se construit ici, côte à côte, pour permettre des comparaisons `.NET` vs Rust
- aucun déplacement massif de fichiers existants avant le cutover final

## Démarrage

Quand la toolchain Rust est disponible:

```bash
cargo run --manifest-path rust/Cargo.toml -p dw-cli -- version
cargo run --manifest-path rust/Cargo.toml -p dw-cli -- config show --json
cargo run --manifest-path rust/Cargo.toml -p dw-cli -- phase0 status
cargo test --manifest-path rust/Cargo.toml
```

## Structure

```text
rust/
  crates/
    dw-cli
    dw-config
    dw-contracts
    dw-ado
    dw-db
    dw-git
    dw-workspace
    dw-ui
  docs/
  scripts/
```

## Portée actuelle

Cette première mise en place couvre:

- le squelette du workspace Cargo
- les modules alignés sur l'architecture cible
- un CLI minimal de validation de structure
- des premiers tests unitaires sur les composants déterministes
- la documentation et les checklists de spike Phase 0

Elle ne prouve pas encore:

- l'auth ADO réelle sur Windows
- la lecture ADO réelle sur un projet cible
- la connectivité SQL Server réelle avec `tiberius`
- la continuité d'upgrade `.NET -> Rust`
