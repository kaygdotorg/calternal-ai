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

          # Chromium for the browser suites (`npm run e2e:gate -w apps/web`), from this pinned nixpkgs so every Linux dev VM has the same build and its libraries.
          # Playwright looks for the browser revision of its own version, so `playwright` in apps/web/package.json is pinned to the version of
          # `pkgs.playwright-driver` in this nixpkgs (1.63.0): raise both together.
          shellHook = lib.optionalString stdenv.isLinux ''
            export PLAYWRIGHT_BROWSERS_PATH=${pkgs.playwright-driver.browsers}
            export PLAYWRIGHT_SKIP_VALIDATE_HOST_REQUIREMENTS=true
            export PLAYWRIGHT_SKIP_BROWSER_DOWNLOAD=1
            export FONTCONFIG_FILE=${pkgs.makeFontsConf { fontDirectories = [ pkgs.dejavu_fonts pkgs.liberation_ttf ]; }} # the nix Chromium finds no system fonts
          '';
        };
      });
}
