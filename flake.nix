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
          vendorHash = "sha256-AiiP8kVBHE3aHGaSl5Zcx9zJ5yhQScXBOkaGC3qrM5E=";
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
          nativeBuildInputs = [ pkgs.chromium pkgs.fontconfig pkgs.dejavu_fonts pkgs.git ];
          env = commonArgs.env // {
            DW_CHROMIUM = "${pkgs.chromium}/bin/chromium";
            FONTCONFIG_FILE = pkgs.makeFontsConf {
              fontDirectories = [ pkgs.dejavu_fonts ];
            };
          };
          checkPhase = ''
            cp internal/web/components_templ.go "$TMPDIR/components_templ.go"
            go tool templ generate
            cmp "$TMPDIR/components_templ.go" internal/web/components_templ.go
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
            export XDG_CACHE_HOME="$TMPDIR/cache"
            mkdir -p "$XDG_CACHE_HOME"
            go vet -tags=timetzdata ./...
            go tool staticcheck ./...
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

        testScript = pkgs.writeShellApplication {
          name = "dw-test";
          runtimeInputs = [ go pkgs.git ];
          text = ''
            export CGO_ENABLED=0
            export GOTOOLCHAIN=local
            export GOFLAGS="-tags=timetzdata"
            go test "$@" ./...
          '';
        };

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

            fail_if_matches \
              "Domain layers must not import web presentation or lifecycle packages" \
              'github.com/sachahjkl/dw/internal/(web|tui|webservice)' \
              internal/action internal/cockpit internal/contract internal/data internal/dataapp internal/execution internal/l10n internal/providerapp internal/wirejson internal/work internal/workapp internal/workspace

            echo "Architecture check passed."
          '';
        };

        bumpCommitPushScript = pkgs.writeShellApplication {
          name = "dw-bump-commit-push";
          runtimeInputs = [ pkgs.coreutils pkgs.findutils pkgs.git ];
          text = ''
            set -euo pipefail

            if (( $# > 1 )); then
              echo 'Usage: nix run .#bump-commit-push -- ["commit message"]' >&2
              exit 2
            fi

            repo="$(git rev-parse --show-toplevel)"
            cd "$repo"

            if [[ -n "$(git status --porcelain)" ]]; then
              echo "Cannot release from a dirty worktree." >&2
              exit 1
            fi

            branch="$(git symbolic-ref --quiet --short HEAD)" || {
              echo "Cannot bump from a detached HEAD." >&2
              exit 1
            }
            git remote get-url origin >/dev/null

            version_file="$repo/VERSION"
            if [[ ! -f "$version_file" ]]; then
              mapfile -t version_files < <(find "$repo" -path "$repo/.git" -prune -o -type f -name VERSION -print)
              if (( ''${#version_files[@]} != 1 )); then
                echo "Expected exactly one VERSION file; found ''${#version_files[@]}." >&2
                printf '  %s\n' "''${version_files[@]}" >&2
                exit 1
              fi
              version_file="''${version_files[0]}"
            fi

            current="$(tr -d '\r\n' < "$version_file")"
            if [[ ! "$current" =~ ^[0-9]{4}\.[0-9]{2}\.[0-9]{2}\.[0-9]+$ ]]; then
              echo "Invalid version '$current' in $version_file; expected YYYY.MM.DD.N." >&2
              exit 1
            fi

            today="$(date +%Y.%m.%d)"
            current_date="''${current%.*}"
            current_build="''${current##*.}"
            if [[ "$current_date" == "$today" ]]; then
              next="$today.$((10#$current_build + 1))"
            else
              next="$today.1"
            fi

            printf '%s\n' "$next" > "$version_file"
            git add -- "$version_file"
            message="''${1:-chore: release $next}"
            git commit -m "$message"
            git push origin "HEAD:$branch"
            printf 'Released %s on %s.\n' "$next" "$branch"
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

          bump-commit-push = {
            type = "app";
            program = "${bumpCommitPushScript}/bin/dw-bump-commit-push";
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
            echo "  nix run .#bump-commit-push -- \"commit message\""
            echo "  go run ./cmd/dw version"
          '';
        };
      });
}
