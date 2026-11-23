# A flake-parts module declaring the crate's dev shells with `make-shells`,
# for flakes built with flake-parts rather than `mkShell`, and an app
# running Cargo, written once for every system.
{lib, ...}: let
  targets = ["linux" "macos"];
in {
  perSystem = {pkgs, ...}: let
    target-scripts = target: [
      (pkgs.writeShellScriptBin "check-${target}" ''
        cargo check --target ${target}
      '')
    ];

    every-target-script = lib.pipe targets [
      (lib.map target-scripts)
      lib.flatten
    ];

    log-level = "info";
    RUST_BACKTRACE = "1";
  in {
    apps.check.program = "${pkgs.cargo}/bin/cargo";

    make-shells.default.env.CARGO_TARGET_DIR = "target";

    make-shells.default = {
      env = {
        RUST_LOG = log-level;
        inherit RUST_BACKTRACE;
      };

      packages =
        [pkgs.cargo]
        ++ every-target-script
        ++ [
          (pkgs.writeShellScriptBin "resolve" ''
            cargo run --quiet -- resolve "$@"
          '')
        ];
    };
  };
}
