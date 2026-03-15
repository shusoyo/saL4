{
  description = "rcore tutorial development environment";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, fenix, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
        toolchain = fenix.packages.${system}.fromToolchainFile {
          file = ./rust-toolchain.toml;
          sha256 = "sha256-qqF33vNuAdU5vua96VKVIwuc43j4EFeEXbjQ6+l4mO4="; 
        };
      in {
        devShells.default = pkgs.mkShell {
          name = "rcore-dev-shell";

          nativeBuildInputs = [
            toolchain
            pkgs.qemu
            # pkgs.gdb
            # pkgs.gcc-riscv64-unknown-elf
            pkgs.cargo-binutils
            pkgs.cargo-clone
          ];

          shellHook = ''
            echo "Welcome to rcore tutorial dev environment"
            echo "Rust version: $(rustc --version)"
            echo "QEMU version: $(qemu-system-riscv64 --version | head -n1)"
            
            export RUST_SRC_PATH="${toolchain}/lib/rustlib/src/rust/library"
            exec fish
          '';
        };
      });
}
