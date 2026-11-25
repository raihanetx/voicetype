{ pkgs ? import <nixpkgs> {} }:
pkgs.mkShell {
  buildInputs = with pkgs; [
    rustc cargo gcc pkg-config
    gtk3 at-spi2-core at-spi2-atk alsa-lib pulseaudio
    xorg.libX11 xorg.libXi
  ];
}
