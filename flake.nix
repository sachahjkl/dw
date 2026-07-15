{
  description = "dw - Dev Workflow CLI";

  nixConfig = {
    extra-substituters = [
      "https://sachahjkl.cachix.org"
    ];
    extra-trusted-public-keys = [
      "sachahjkl.cachix.org-1:cepX7PCUV88hCchnh9prZM5V72wRkCf6oSJL6JfgWs0="
    ];
  };

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs { inherit system; };
        lib = pkgs.lib;
        versionPrefix = lib.strings.trim (builtins.readFile ./VERSION);
        sourceRevision =
          if self ? shortRev then self.shortRev
          else if self ? rev then builtins.substring 0 7 self.rev
          else if self ? dirtyShortRev then self.dirtyShortRev
          else "dev";
        packageVersion = "${versionPrefix}+${sourceRevision}";

        nativeBuildInputs = [
          pkgs.git
          pkgs.perl
          pkgs.pkg-config
        ];

        buildInputs = lib.optionals pkgs.stdenv.isLinux [
          pkgs.openssl
        ] ++ lib.optionals pkgs.stdenv.isDarwin [
          pkgs.darwin.apple_sdk.frameworks.Security
          pkgs.darwin.apple_sdk.frameworks.SystemConfiguration
        ];

        dwPackage = pkgs.rustPlatform.buildRustPackage {
          pname = "dw";
          version = packageVersion;
          src = ./.;

          cargoLock.lockFile = ./Cargo.lock;
          inherit nativeBuildInputs buildInputs;

          cargoBuildFlags = [ "-p" "dw-cli" ];
          # CI runs the full workspace check app; the package build only produces the release binary.
          doCheck = false;

          DW_COMMIT = sourceRevision;

          postInstall = ''
            if [ -x "$out/bin/dw-cli" ]; then
              mv "$out/bin/dw-cli" "$out/bin/dw"
            fi
          '';
        };

        cargoScript = name: command: pkgs.writeShellApplication {
          inherit name;
          runtimeInputs = [
            pkgs.cargo
            pkgs.cmake
            pkgs.clippy
            pkgs.rustc
            pkgs.rustfmt
            pkgs.stdenv.cc
          ] ++ nativeBuildInputs ++ buildInputs;
          text = command;
        };

        checkScript = cargoScript "dw-check" ''
          ${architectureCheckScript}/bin/dw-architecture-check
          cargo fmt --all -- --check
          cargo test --workspace --locked
          cargo clippy --workspace --all-targets --locked -- -D warnings
        '';

        fmtScript = cargoScript "dw-fmt" ''
          cargo fmt --all
        '';

        testScript = cargoScript "dw-test" ''
          cargo test --workspace --locked "$@"
        '';

        clippyScript = cargoScript "dw-clippy" ''
          cargo clippy --workspace --all-targets --locked -- -D warnings "$@"
        '';

        architectureCheckScript = pkgs.writeShellApplication {
          name = "dw-architecture-check";
          runtimeInputs = [ pkgs.ripgrep ];
          text = ''
            set -euo pipefail
            fail_if_matches() {
              local label="$1"
              local pattern="$2"
              shift 2
              if rg -n "$pattern" "$@"; then
                echo "Architecture check failed: $label" >&2
                exit 1
              fi
            }

            fail_if_matches \
              "TUI/UI must not embed CLI command hints or shell relaunches" \
              "dw-cli-adapter|dw_cli_adapter|current_exe|run_current_dw|LegacyShellAction|Action interne non portée|CommandAction|CompletionShow|QuickOptionAction::Completion|Confirmation CLI|AnsiRender|Non-TTY|dw task |dw ado |dw db |dw auth |dw config " \
              crates/dw-tui/src crates/dw-tui-adapter/src crates/dw-ui/src

            fail_if_matches \
              "Core crates must not carry CLI JSON flags or dw command hints" \
              "\\bjson: bool\\b|pub json: bool|json: _|dw task |dw ado |dw db |dw auth |dw config " \
              crates/dw-ado/src crates/dw-ado-commands/src crates/dw-config/src crates/dw-agent/src crates/dw-doctor/src crates/dw-secret/src crates/dw-db/src crates/dw-task/src crates/dw-workspace/src crates/dw-upgrade/src

            fail_if_matches \
              "Core requests must use ExecutionMode instead of execute bool flags" \
              "\\bexecute: bool\\b|pub execute: bool" \
              crates/dw-ado/src crates/dw-ado-commands/src crates/dw-config/src crates/dw-agent/src crates/dw-doctor/src crates/dw-secret/src crates/dw-db/src crates/dw-task/src crates/dw-workspace/src crates/dw-upgrade/src

            fail_if_matches \
              "TUI tests should use action wording, not legacy command wording" \
              "fn .*_command\\(" \
              crates/dw-tui/src/actions.rs crates/dw-tui/src/form.rs crates/dw-tui/src/ui.rs

            echo "Architecture check passed."
          '';
        };
      in
      {
        packages.default = dwPackage;
        packages.dw = dwPackage;

        checks.default = dwPackage;

        apps = {
          dw = {
            type = "app";
            program = "${dwPackage}/bin/dw";
          };

          check = {
            type = "app";
            program = "${checkScript}/bin/dw-check";
          };

          fmt = {
            type = "app";
            program = "${fmtScript}/bin/dw-fmt";
          };

          test = {
            type = "app";
            program = "${testScript}/bin/dw-test";
          };

          clippy = {
            type = "app";
            program = "${clippyScript}/bin/dw-clippy";
          };

          architecture-check = {
            type = "app";
            program = "${architectureCheckScript}/bin/dw-architecture-check";
          };

          default = self.apps.${system}.dw;
        };

        devShells.default = pkgs.mkShell {
          packages = [
            pkgs.cargo
            pkgs.clippy
            pkgs.git
            pkgs.rust-analyzer
            pkgs.rustc
            pkgs.rustfmt
          ] ++ nativeBuildInputs ++ buildInputs;

          env = {
            CARGO_TERM_COLOR = "always";
            RUST_BACKTRACE = "1";
            DW_COMMIT = sourceRevision;
          };

          shellHook = ''
            echo "dw dev shell"
            echo "Commands:"
            echo "  nix run .#dw -- version"
            echo "  nix run .#check"
            echo "  nix run .#fmt"
            echo "  nix run .#test"
            echo "  nix run .#clippy"
            echo "  nix run .#architecture-check"
            echo "  cargo run -p dw-cli -- version"
          '';
        };
      });
}
