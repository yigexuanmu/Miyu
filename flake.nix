{
  description = "Miyu - TUI Diff 显示";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay.url = "github:oxalica/rust-overlay";
  };

  outputs = { self, nixpkgs, flake-utils, rust-overlay }:
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
        };

        packages.default = pkgs.rustPlatform.buildRustPackage {
          pname = "miyu";
          version = "0.1.14";
          src = ./.;
          cargoLock.lockFile = ./Cargo.lock;
          nativeBuildInputs = [ pkgs.pkg-config ];
          buildInputs = [
            pkgs.alsa-lib
            pkgs.openssl
            pkgs.sqlite
          ];
          doCheck = false;
          meta = with pkgs.lib; {
            description = "Command-line AI assistant with TUI diff display";
            homepage = "https://github.com/yigexuanmu/Miyu";
            license = licenses.mit;
          };
        };
      });
}
