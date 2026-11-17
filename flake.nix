{
  # Development shell for Calternal-AI. Modeled on the existing app's flake so both
  # projects build with the same pinned inputs. Use on the dev VMs:
  #   nix develop -c cargo test --workspace
  # Do not build on hd01/hd02 (see AGENTS.md, Machines).
  description = "Calternal-AI dev shell: Rust (rust-toolchain.toml), Node, OpenSSL, SQLite";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/1e7da2489c0ba3a06bceec1b444fbc02c5223ab1";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, rust-overlay, flake-utils }:
    flake-utils.lib.eachSystem [ "x86_64-linux" "aarch64-linux" "aarch64-darwin" ] (system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ rust-overlay.overlays.default ];
        };
        inherit (pkgs) lib stdenv;
        # The compiler version comes from rust-toolchain.toml, the single source of truth.
        rust = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
      in
      {
        devShells.default = pkgs.mkShell {
          packages = [
            rust
            pkgs.nodejs_24
            pkgs.pkg-config
            pkgs.openssl # webauthn-rs links it through openssl-sys
            pkgs.sqlite
            pkgs.git
            pkgs.cacert
            pkgs.python3 # contract/verify-cli.py
          ] ++ lib.optionals stdenv.isLinux [ pkgs.clang pkgs.mold ]
            ++ lib.optionals stdenv.isDarwin [ pkgs.libiconv ];
        };
      });
}
