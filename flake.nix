{
  description = "A basic Rust development environment";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, rust-overlay, flake-utils, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };
        # Pinned so local dev, `cargo clippy`, and CI all agree on one
        # toolchain. Keep this version in sync with RUST_TOOLCHAIN in
        # .github/workflows/ci.yml.
        rustToolchain = pkgs.rust-bin.stable."1.96.1".default.override {
          extensions = [ "rust-src" "rust-analyzer" ];
        };
      in
      {
        devShells.default = pkgs.mkShell {
          buildInputs = with pkgs; [
            rustToolchain
            pkg-config
            openssl
            cargo-release
            python314
          ];

          shellHook = ''
            echo "Rust development environment loaded"
            rustc --version
          '';
        };
      }
    );
}
