{
  inputs = {
    nixpkgs = {
      url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    };
  };

  outputs = {nixpkgs, ...}: let
    pkgs = nixpkgs.legacyPackages.x86_64-linux;

    lint = pkgs.writeShellScriptBin "lint" ''
      cargo run --quiet -- lint
    '';

    client-tests = pkgs.writeShellApplication {
      name = "test-client";
      runtimeInputs = [pkgs.cargo];
      text = "cargo test";
    };

    backlinks = pkgs.writeShellScriptBin "backlinks" ''
      cargo run --quiet -- backlinks "$@"
    '';
  in {
    devShells.x86_64-linux.default = pkgs.mkShell {
      nativeBuildInputs = [pkgs.cargo];
      buildInputs = [lint];
      packages = [client-tests] ++ [backlinks];
    };
  };
}
