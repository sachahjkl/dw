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
          cargoTestFlags = [ "--workspace" ];

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
            pkgs.clippy
            pkgs.rustc
            pkgs.rustfmt
          ] ++ nativeBuildInputs ++ buildInputs;
          text = command;
        };

        checkScript = cargoScript "dw-check" ''
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
            echo "  cargo run -p dw-cli -- version"
          '';
        };
      });
}
