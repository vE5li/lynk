# A flake-parts module declaring the crate's dev shells with `make-shells`,
# for flakes built with flake-parts rather than `mkShell`.
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
  in {
    make-shells.default = {
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
