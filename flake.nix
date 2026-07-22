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
    flake-utils.lib.eachSystem [ "x86_64-linux" ] (system:
      let
        pkgs = import nixpkgs { inherit system; };
        lib = pkgs.lib;
        go = pkgs.go_1_26;
        buildGoModule = pkgs.buildGoModule.override { inherit go; };
        src = lib.fileset.toSource {
          root = ./.;
          fileset = lib.fileset.unions (
            [
              ./cmd
              ./internal
              ./locales
              ./schemas
              ./scripts
              ./testdata
              ./go.mod
              ./LICENSE
              ./VERSION
            ] ++ lib.optionals (builtins.pathExists ./go.sum) [ ./go.sum ]
          );
        };
        versionPrefix = lib.strings.trim (builtins.readFile ./VERSION);
        sourceRevision =
          if self ? shortRev then self.shortRev
          else if self ? rev then builtins.substring 0 7 self.rev
          else if self ? dirtyShortRev then self.dirtyShortRev
          else "dev";
        packageVersion = "${versionPrefix}+${sourceRevision}";
        ldflags = [
          "-s"
          "-w"
          "-X github.com/sachahjkl/dw/internal/buildinfo.Version=${versionPrefix}"
          "-X github.com/sachahjkl/dw/internal/buildinfo.Commit=${sourceRevision}"
        ];

        commonArgs = {
          pname = "dw";
          version = packageVersion;
          inherit src ldflags;
          tags = [ "timetzdata" ];
          subPackages = [ "cmd/dw" ];
          vendorHash = "sha256-/ms4tQysN66o2qeVvLC5gXHVmfZeBgRO5oQ2aTndqeY=";
          env.CGO_ENABLED = "0";
        };

        dwPackage = buildGoModule (commonArgs // {
          doCheck = false;
        });

        formatCheck = pkgs.runCommand "dw-format-check" {
          nativeBuildInputs = [ go ];
        } ''
          cp -R ${src} source
          chmod -R u+w source
          cd source
          unformatted="$(find . -type f -name '*.go' -exec gofmt -l {} +)"
          if [ -n "$unformatted" ]; then
            printf 'Unformatted Go files:\n%s\n' "$unformatted" >&2
            exit 1
          fi
          touch $out
        '';

        testCheck = buildGoModule (commonArgs // {
          pname = "dw-tests";
          doCheck = true;
          checkPhase = ''
            runHook preCheck
            go test -tags=timetzdata ./...
            runHook postCheck
          '';
          installPhase = "touch $out";
        });

        staticAnalysisCheck = buildGoModule (commonArgs // {
          pname = "dw-static-analysis";
          doCheck = true;
          checkPhase = ''
            runHook preCheck
            go vet -tags=timetzdata ./...
            runHook postCheck
          '';
          installPhase = "touch $out";
        });

        goScript = name: command: pkgs.writeShellApplication {
          inherit name;
          runtimeInputs = [ go ];
          text = ''
            export CGO_ENABLED=0
            export GOTOOLCHAIN=local
            export GOFLAGS="-tags=timetzdata"
            ${command}
          '';
        };

        fmtScript = goScript "dw-fmt" ''
          go fmt ./...
        '';

        testScript = goScript "dw-test" ''
          go test "$@" ./...
        '';

        staticAnalysisScript = goScript "dw-static-analysis" ''
          go vet "$@" ./...
        '';

        architectureScript = pkgs.writeShellApplication {
          name = "dw-architecture";
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
              "TUI must not import CLI parsing" \
              'github.com/sachahjkl/dw/internal/cli' \
              internal/tui

            fail_if_matches \
              "Core contracts must not depend on presentation or composition" \
              'github.com/sachahjkl/dw/internal/(bootstrap|cli|console|tui|provider)|"os/exec"' \
              internal/action/*.go internal/contract/*.go internal/data/*.go internal/l10n/*.go internal/wirejson/*.go internal/work/*.go

            fail_if_matches \
              "Application and core layers must not import concrete providers" \
              'github.com/sachahjkl/dw/internal/(data|work)/[^"]+' \
              internal/dataapp internal/providerapp internal/workapp internal/workspace

            echo "Architecture check passed."
          '';
        };

        architectureCheck = pkgs.runCommand "dw-architecture-check" {
          nativeBuildInputs = [ architectureScript ];
        } ''
          cd ${src}
          dw-architecture
          touch $out
        '';

        checkScript = pkgs.writeShellApplication {
          name = "dw-check";
          text = ''
            test -e ${architectureCheck}
            test -e ${formatCheck}
            test -e ${testCheck}
            test -e ${staticAnalysisCheck}
            echo "Go checks passed."
          '';
        };
      in
      {
        packages.default = dwPackage;
        packages.dw = dwPackage;

        checks = {
          default = dwPackage;
          formatting = formatCheck;
          tests = testCheck;
          static-analysis = staticAnalysisCheck;
          architecture = architectureCheck;
        };

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

          static-analysis = {
            type = "app";
            program = "${staticAnalysisScript}/bin/dw-static-analysis";
          };

          architecture = {
            type = "app";
            program = "${architectureScript}/bin/dw-architecture";
          };

          default = self.apps.${system}.dw;
        };

        devShells.default = pkgs.mkShell {
          packages = [ go pkgs.git pkgs.gopls ];

          env = {
            CGO_ENABLED = "0";
            GOTOOLCHAIN = "local";
            GOFLAGS = "-tags=timetzdata";
          };

          shellHook = ''
            echo "dw dev shell (Go ${go.version})"
            echo "Commands:"
            echo "  nix run .#dw -- version"
            echo "  nix run .#check"
            echo "  nix run .#fmt"
            echo "  nix run .#test"
            echo "  nix run .#static-analysis"
            echo "  nix run .#architecture"
            echo "  go run ./cmd/dw version"
          '';
        };
      });
}
