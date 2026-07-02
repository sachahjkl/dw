# Rust Rewrite Bootstrap

## Decision

Le rewrite Rust se fait dans `rust/` tant que la parité n'est pas prouvée.

## Why

1. garder `src/Dw.Cli` exécutable comme référence
2. éviter une réorganisation massive sans valeur fonctionnelle immédiate
3. conserver les chemins Nix, CI et scripts existants
4. permettre des comparaisons `.NET` vs Rust côte à côte

## Initial Layout

```text
rust/
  Cargo.toml
  README.md
  docs/
  scripts/
  crates/
    dw-cli
    dw-config
    dw-contracts
    dw-ado
    dw-db
    dw-git
    dw-workspace
    dw-ui
```

## Non-Goals Of This Bootstrap

- remplacer le binaire `dw`
- déplacer l'existant `.NET`
- brancher la release CI sur le Rust
- déclarer la parité Phase 0 atteinte

## Next Required Step

Exécuter les spikes Phase 0 sur une machine Windows avec accès réel à Azure DevOps et SQL Server.
