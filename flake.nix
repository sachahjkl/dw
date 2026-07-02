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
        dotnet = pkgs.dotnetCorePackages.sdk_10_0;
        dotnetRuntime = pkgs.dotnetCorePackages.runtime_10_0;
        versionPrefix = pkgs.lib.strings.trim (builtins.readFile ./VERSION);
        sourceRevision =
          if self ? shortRev then self.shortRev
          else if self ? rev then builtins.substring 0 7 self.rev
          else if self ? dirtyShortRev then self.dirtyShortRev
          else "dev";
        packageVersion = "${versionPrefix}+${sourceRevision}";

        dotnetEnv = ''
          export DOTNET_CLI_TELEMETRY_OPTOUT=1
          export DOTNET_NOLOGO=1
          export DOTNET_SKIP_FIRST_TIME_EXPERIENCE=1
        '';

        buildScript = pkgs.writeShellApplication {
          name = "dw-build";
          runtimeInputs = [ dotnet ];
          text = ''
            ${dotnetEnv}
            dotnet build ./Dw.slnx \
              --configuration Release \
              -p:VersionPrefix=${versionPrefix} \
              -p:SourceRevisionId=${sourceRevision}
          '';
        };

        checkScript = pkgs.writeShellApplication {
          name = "dw-check";
          runtimeInputs = [ dotnet ];
          text = ''
            ${dotnetEnv}
            dotnet build ./Dw.slnx \
              --configuration Release \
              -p:VersionPrefix=${versionPrefix} \
              -p:SourceRevisionId=${sourceRevision}

            dotnet run \
              --project ./src/Dw.Cli \
              --configuration Release \
              --no-build \
              -- version
          '';
        };

        dwPackage = pkgs.buildDotnetModule {
          pname = "dw";
          version = packageVersion;
          src = ./.;
          projectFile = "src/Dw.Cli/Dw.Cli.csproj";
          nugetDeps = pkgs.mkNugetDeps {
            name = "dw";
            sourceFile = ./deps.json;
          };
          dotnet-sdk = dotnet;
          dotnet-runtime = dotnetRuntime;
          executables = [ "dw" ];
          selfContainedBuild = false;
          dotnetFlags = [
            "-p:VersionPrefix=${versionPrefix}"
            "-p:SourceRevisionId=${sourceRevision}"
          ];
        };

        publishWinX64Script = pkgs.writeShellApplication {
          name = "dw-publish-win-x64";
          runtimeInputs = [ dotnet ];
          text = ''
            ${dotnetEnv}
            dotnet publish ./src/Dw.Cli/Dw.Cli.csproj \
              --configuration Release \
              --runtime win-x64 \
              --self-contained false \
              -p:PublishSingleFile=true \
              -p:DebugType=embedded \
              -p:VersionPrefix=${versionPrefix} \
              -p:SourceRevisionId=${sourceRevision} \
              --output ./artifacts/win-x64
          '';
        };

        publishLinuxX64Script = pkgs.writeShellApplication {
          name = "dw-publish-linux-x64";
          runtimeInputs = [ dotnet pkgs.bash pkgs.coreutils ];
          text = ''
            ${dotnetEnv}
            VERSION=${versionPrefix} COMMIT=${sourceRevision} bash ./scripts/publish-linux-x64.sh
          '';
        };

        setVersionScript = pkgs.writeShellApplication {
          name = "dw-set-version";
          runtimeInputs = [ pkgs.bash pkgs.coreutils ];
          text = ''
            bash ./scripts/set-version.sh "$@"
          '';
        };
      in
      {
        devShells.default = pkgs.mkShell {
          packages = [
            dotnet
            pkgs.cargo
            pkgs.git
            pkgs.rustc
            pkgs.clippy
            pkgs.rust-analyzer
            pkgs.rustfmt
            pkgs.openssl
            pkgs.pkg-config
          ];

          env = {
            CARGO_TERM_COLOR = "always";
            RUST_BACKTRACE = "1";
          };

          shellHook = ''
            ${dotnetEnv}
            echo "dw dev shell"
            echo "Commands:"
            echo "  nix run .#build"
            echo "  nix run .#check"
            echo "  nix run .#publish-win-x64"
            echo "  nix run .#publish-linux-x64"
            echo "  nix run .#set-version"
            echo "  nix run .#set-version -- 2026.06.20.2"
            echo "  cargo run --manifest-path rust/Cargo.toml -p dw-cli -- version"
            echo "  cargo test --manifest-path rust/Cargo.toml"
            echo "  cargo fmt --all"
            echo "  cargo clippy --workspace --all-targets"
            echo ""
            echo "Rust toolchain:"
            echo "  rustc: $(rustc --version)"
            echo "  cargo: $(cargo --version)"
            echo "  rustfmt: $(rustfmt --version)"
            echo "  clippy: $(cargo clippy --version)"
            echo ""
            echo "For release artifacts with explicit metadata:"
            echo "  nix develop -c env VERSION=2026.06.20.1 COMMIT=abc1234 bash ./scripts/publish-linux-x64.sh"
          '';
        };

        apps = {
          build = {
            type = "app";
            program = "${buildScript}/bin/dw-build";
          };

          check = {
            type = "app";
            program = "${checkScript}/bin/dw-check";
          };

          publish-win-x64 = {
            type = "app";
            program = "${publishWinX64Script}/bin/dw-publish-win-x64";
          };

          publish-linux-x64 = {
            type = "app";
            program = "${publishLinuxX64Script}/bin/dw-publish-linux-x64";
          };

          set-version = {
            type = "app";
            program = "${setVersionScript}/bin/dw-set-version";
          };

          dw = {
            type = "app";
            program = "${dwPackage}/bin/dw";
          };

          default = self.apps.${system}.dw;
        };

        packages.default = dwPackage;
      });
}
