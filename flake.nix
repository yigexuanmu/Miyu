{
  description = "Miyu - TUI Diff 显示 (Nix flake)";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    miyu-src = {
      url = "github:SHORiN-KiWATA/Miyu";
      flake = false;
    };
  };

  outputs =
    { self, nixpkgs, flake-utils, rust-overlay, miyu-src }:
    flake-utils.lib.eachDefaultSystem (system:
    let
      overlays = [ (import rust-overlay) ];
      pkgs = import nixpkgs { inherit system overlays; };
      rustToolchain = pkgs.rust-bin.stable.latest.default.override {
        extensions = [ "rust-src" "rust-analyzer" ];
      };
    in
    {
      devShells.default = pkgs.mkShell {
        buildInputs = [
          rustToolchain
          pkgs.pkg-config
          pkgs.alsa-lib
          pkgs.openssl
          pkgs.sqlite
        ];

        shellHook = ''
          echo "Miyu development shell"
          echo "  Source: ${miyu-src}"
        '';
      };

      packages.default = pkgs.rustPlatform.buildRustPackage {
        pname = "miyu";
        version = "0.1.14";
        src = miyu-src;
        cargoLock.lockFile = "${miyu-src}/Cargo.lock";
        nativeBuildInputs = [ pkgs.pkg-config ];
        buildInputs = [
          pkgs.alsa-lib
          pkgs.openssl
          pkgs.sqlite
        ];
        doCheck = false;
        meta = with pkgs.lib; {
          description = "Command-line AI assistant with TUI diff display";
          homepage = "https://github.com/SHORiN-KiWATA/Miyu";
          license = licenses.mit;
        };
      };
    });
}
