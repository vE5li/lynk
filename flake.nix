{
  inputs = {
    crane = {
      url = "github:ipetkov/crane";
    };

    flake-utils = {
      url = "github:numtide/flake-utils";
    };

    nixpkgs = {
      url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    };

    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = {
    self,
    crane,
    flake-utils,
    nixpkgs,
    rust-overlay,
  }:
    flake-utils.lib.eachDefaultSystem (system: let
      overlays = [(import rust-overlay)];

      pkgs = (import nixpkgs) {inherit system overlays;};

      toolchain = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;

      craneLib = (crane.mkLib pkgs).overrideToolchain (_: toolchain);

      craneArgs = {
        src = pkgs.lib.cleanSource ./.;
        strictDeps = true;
        nativeBuildInputs = [pkgs.pkg-config];
        buildInputs = [pkgs.openssl];
      };

      cargoArtifacts = craneLib.buildDepsOnly craneArgs;

      package = craneLib.buildPackage (craneArgs
        // {
          inherit cargoArtifacts;
          doCheck = false;
        });

      build-rust = pkgs.writeShellApplication {
        name = "build-rust";
        runtimeInputs = [toolchain];
        text = "cargo build --workspace";
      };

      build = pkgs.writeShellApplication {
        name = "build";
        runtimeInputs = [build-rust];
        text = ''
          build-rust
        '';
      };

      # Schemas that tombi's built-in schemas refer to, for `--offline`. The
      # entry names follow tombi's cache layout (`<scheme>/<host>/<path>`).
      tombiCache = pkgs.linkFarm "tombi-schema-cache" {
        "https/www.schemastore.org/cargo-lints-clippy.json" = pkgs.fetchurl {
          url = "https://raw.githubusercontent.com/SchemaStore/schemastore/45d08e79bf6599e618bae768389d2679f0aa7f78/src/schemas/json/cargo-lints-clippy.json";
          hash = "sha256-I9QO7h6ADve5ktX2Rl4yh7/aA3UnifDXVAT6oGY01Js=";
        };
      };

      nexus = pkgs.writeShellScriptBin "nexus" ''
        set -e
        root=$(jj root)
        cargo run --quiet --release --manifest-path "$root/Cargo.toml" -- --root "$root" "$@"
      '';

      format = pkgs.writeShellApplication {
        name = "format";
        runtimeInputs = [format-nix format-toml format-rust];
        text = ''
          format-nix
          format-toml
          format-rust
        '';
      };

      format-check = pkgs.writeShellApplication {
        name = "format-check";
        runtimeInputs = [format-nix-check format-toml-check format-rust-check];
        text = ''
          format-nix-check
          format-toml-check
          format-rust-check
        '';
      };

      format-rust = pkgs.writeShellApplication {
        name = "format-rust";
        runtimeInputs = [toolchain];
        text = ''
          cargo fmt --all -- \
            --config condense_wildcard_suffixes=true,edition=2024,fn_call_width=80,format_macro_matchers=true,format_strings=true,group_imports=StdExternalCrate,hex_literal_case=Upper,imports_granularity=Module,max_width=140,overflow_delimited_expr=true,reorder_impl_items=true,style_edition=2024,use_field_init_shorthand=true,wrap_comments=true
        '';
      };

      format-rust-check = pkgs.writeShellApplication {
        name = "format-rust-check";
        runtimeInputs = [toolchain];
        text = ''
          cargo fmt --all -- --check \
            --config condense_wildcard_suffixes=true,edition=2024,fn_call_width=80,format_macro_matchers=true,format_strings=true,group_imports=StdExternalCrate,hex_literal_case=Upper,imports_granularity=Module,max_width=140,overflow_delimited_expr=true,reorder_impl_items=true,style_edition=2024,use_field_init_shorthand=true,wrap_comments=true
        '';
      };

      format-nix = pkgs.writeShellApplication {
        name = "format-nix";
        runtimeInputs = [pkgs.alejandra];
        text = "alejandra .";
      };

      format-nix-check = pkgs.writeShellApplication {
        name = "format-nix-check";
        runtimeInputs = [pkgs.alejandra];
        text = "alejandra --check .";
      };

      format-toml = pkgs.writeShellApplication {
        name = "format-toml";
        runtimeInputs = [pkgs.tombi];
        runtimeEnv.TOMBI_CACHE_HOME = tombiCache;
        text = "tombi format --offline";
      };

      format-toml-check = pkgs.writeShellApplication {
        name = "format-toml-check";
        runtimeInputs = [pkgs.tombi];
        runtimeEnv.TOMBI_CACHE_HOME = tombiCache;
        text = "tombi format --check --offline";
      };

      lint-toml = pkgs.writeShellApplication {
        name = "lint-toml";
        runtimeInputs = [pkgs.tombi];
        runtimeEnv.TOMBI_CACHE_HOME = tombiCache;
        text = "tombi lint --offline --error-on-warnings";
      };

      lint-rust = pkgs.writeShellApplication {
        name = "lint-rust";
        runtimeInputs = [toolchain];
        text = "cargo clippy --workspace --all-targets -- --deny warnings";
      };

      lint = pkgs.writeShellApplication {
        name = "lint";
        runtimeInputs = [lint-toml lint-rust];
        text = ''
          lint-toml
          lint-rust
        '';
      };

      test-rust = pkgs.writeShellApplication {
        name = "test-rust";
        runtimeInputs = [toolchain];
        text = "cargo test --workspace";
      };

      tests = pkgs.writeShellApplication {
        name = "tests";
        runtimeInputs = [test-rust];
        text = ''
          test-rust
        '';
      };
    in {
      formatter = pkgs.alejandra;

      apps.build = {
        type = "app";
        program = pkgs.lib.getExe build;
        meta.description = "Build everything";
      };

      apps.format = {
        type = "app";
        program = pkgs.lib.getExe format;
        meta.description = "Format everything";
      };

      apps.format-check = {
        type = "app";
        program = pkgs.lib.getExe format-check;
        meta.description = "Check the formatting of everything";
      };

      apps.lint = {
        type = "app";
        program = pkgs.lib.getExe lint;
        meta.description = "Lint everything";
      };

      apps.tests = {
        type = "app";
        program = pkgs.lib.getExe tests;
        meta.description = "Run all tests";
      };

      checks.build-rust = package;

      checks.format-nix-check = pkgs.runCommand "format-nix-check" {} ''
        cd ${self}
        ${pkgs.lib.getExe format-nix-check}
        touch $out
      '';

      checks.format-rust-check = pkgs.runCommand "format-rust-check" {} ''
        cd ${self}
        ${pkgs.lib.getExe format-rust-check}
        touch $out
      '';

      checks.format-toml-check = pkgs.runCommand "format-toml-check" {} ''
        cd ${self}
        ${pkgs.lib.getExe format-toml-check}
        touch $out
      '';

      checks.lint-toml = pkgs.runCommand "lint-toml" {} ''
        cd ${self}
        ${pkgs.lib.getExe lint-toml}
        touch $out
      '';

      checks.lint-rust = craneLib.cargoClippy (craneArgs
        // {
          inherit cargoArtifacts;
          cargoClippyExtraArgs = "--all-targets -- --deny warnings";
        });

      checks.test-rust = craneLib.cargoTest (craneArgs
        // {
          inherit cargoArtifacts;
        });

      packages.default = package;

      devShells.default = pkgs.mkShell {
        nativeBuildInputs = [
          toolchain
        ];
        packages = [
          nexus
          build-rust
          build
          format
          format-check
          format-rust
          format-rust-check
          format-nix
          format-nix-check
          format-toml
          format-toml-check
          lint-toml
          lint-rust
          lint
          test-rust
          tests
        ];

        # For any tools that need to see the rust toolchain src
        RUST_SRC_PATH = pkgs.rustPlatform.rustLibSrc;
      };
    });
}
