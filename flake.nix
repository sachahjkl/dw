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
        dotnet = pkgs.dotnetCorePackages.sdk_8_0;
        dotnetRuntime = pkgs.dotnetCorePackages.runtime_8_0;
        version = builtins.getEnv "DW_VERSION";
        commit = builtins.getEnv "DW_COMMIT";
        versionPrefix =
          if version == "" then "0.0.0-local" else version;
        sourceRevision =
          if commit == "" then "dev" else commit;

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
            dotnet build ./Dw.sln \
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
            dotnet build ./Dw.sln \
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
          version = versionPrefix;
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
      in
      {
        devShells.default = pkgs.mkShell {
          packages = [
            dotnet
            pkgs.git
          ];

          shellHook = ''
            ${dotnetEnv}
            echo "dw dev shell"
            echo "Commands:"
            echo "  nix run .#build"
            echo "  nix run .#check"
            echo "  nix run .#publish-win-x64"
            echo "  nix run .#publish-linux-x64"
            echo ""
            echo "For reproducible release metadata, pass:"
            echo "  DW_VERSION=2026.06.20.1 DW_COMMIT=abc1234 nix run .#publish-win-x64"
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

          dw = {
            type = "app";
            program = "${dwPackage}/bin/dw";
          };

          default = self.apps.${system}.dw;
        };

        packages.default = dwPackage;
      });
}
