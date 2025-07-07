{
  pkgs ? import <nixpkgs> { },
}:
pkgs.mkShell {
  RUST_SRC_PATH = "${pkgs.rust.packages.stable.rustPlatform.rustLibSrc}";

  nativeBuildInputs = with pkgs.buildPackages; [
    gcc_multi
    cargo
    rustc
    openssl
    pkg-config
  ];

  shellHook = ''
    export PATH="$PATH:${pkgs.clang-tools}/bin"
  '';

  LD_LIBRARY_PATH = with pkgs; lib.makeLibraryPath [
    openssl
    pkg-config
    gcc
  ];
}
