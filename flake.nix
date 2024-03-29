{
  inputs = {
    flake-utils.url = "github:numtide/flake-utils";
    naersk.url = "github:nix-community/naersk";
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
  };

  outputs = { self, flake-utils, naersk, nixpkgs }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = (import nixpkgs) {
          inherit system;
        };

        lib = pkgs.lib;
        guiInputs = with pkgs; with pkgs.xorg; [ libX11 libXcursor libXrandr libXi vulkan-loader libxkbcommon wayland ];
        commonBuildInputs = with pkgs; [ pkg-config freetype systemd fontconfig  bluez ];

        naersk' = pkgs.callPackage naersk {};
        
        d30-cli-full = naersk'.buildPackage rec {
          pname = "d30-cli";
          src = ./.;
          nativeBuildInputs = with pkgs; [ pkg-config cmake makeWrapper ];
          buildInputs = commonBuildInputs ++ guiInputs;
          postInstall = ''
            wrapProgram "$out/bin/${pname}" \
              --prefix LD_LIBRARY_PATH : "${lib.makeLibraryPath (buildInputs ++ guiInputs)}"
          '';
        };
        
        d30-cli = naersk'.buildPackage rec {
          pname = "d30-cli";
          src = ./.;
          nativeBuildInputs = with pkgs; [ pkg-config cmake makeWrapper ];
          buildInputs = commonBuildInputs;
          cargoBuildOptions = opts: opts ++ [ "--package" pname ];
        };
        
        d30-cli-preview = naersk'.buildPackage rec {
          pname = "d30-cli-preview";
          src = ./.;
          nativeBuildInputs = with pkgs; [ pkg-config cmake makeWrapper ];
          buildInputs = commonBuildInputs ++ guiInputs;
          cargoBuildOptions = opts: opts ++ [ "--package" pname ];
          postInstall = ''
            wrapProgram "$out/bin/${pname}" \
              --prefix LD_LIBRARY_PATH : "${lib.makeLibraryPath (buildInputs ++ guiInputs)}"
          '';
        };

      in {
        defaultPackage = d30-cli-full;
        inherit d30-cli-full;
        inherit d30-cli;
        inherit d30-cli-preview;

        devShell = pkgs.mkShell {
          LD_LIBRARY_PATH = lib.makeLibraryPath (commonBuildInputs ++ guiInputs);
          shellHook = ''
            exec $SHELL
          '';
          nativeBuildInputs = with pkgs; [ rustc cargo rust-analyzer ] ++ commonBuildInputs;
        };
      }
    );
}
