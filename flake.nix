{
  inputs = {
    flake-utils.url = "github:numtide/flake-utils";
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
  };

  outputs = {
    self,
    flake-utils,
    rust-overlay,
    nixpkgs,
  }:
    flake-utils.lib.eachDefaultSystem (system: let
      overlays = [(import rust-overlay)];
      pkgs = (import nixpkgs) {inherit system overlays;};

      nexus = pkgs.writeShellScriptBin "nexus" ''
        set -e
        root=$(jj root)
        cargo run --quiet --release --manifest-path "$root/Cargo.toml" -- --root "$root" "$@"
      '';

      cargo-fmt-command = ''
        cargo fmt --all -- \
          --config condense_wildcard_suffixes=true \
          --config edition=2024 \
          --config fn_call_width=80 \
          --config format_macro_matchers=true \
          --config format_strings=true \
          --config group_imports=StdExternalCrate \
          --config hex_literal_case=Upper \
          --config imports_granularity=Module \
          --config max_width=140 \
          --config overflow_delimited_expr=true \
          --config reorder_impl_items=true \
          --config style_edition=2024 \
          --config use_field_init_shorthand=true \
          --config wrap_comments=true'';

      format = pkgs.writeShellScriptBin "format" ''
        ${cargo-fmt-command}
      '';

      format-check = pkgs.writeShellScriptBin "format-check" ''
        ${cargo-fmt-command} --check
      '';
    in {
      devShell = pkgs.mkShell {
        nativeBuildInputs = [
          (pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml)
        ];
        buildInputs = [
          nexus
          format
          format-check
        ];

        # For any tools that need to see the rust toolchain src
        RUST_SRC_PATH = pkgs.rustPlatform.rustLibSrc;
      };
    });
}
