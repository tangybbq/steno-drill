# Shell configuration to build rsure.
{ pkgs ? import <nixos> {} }:
let
  lib = pkgs.lib;
  stdenv = pkgs.stdenv;
in
pkgs.mkShell {
  nativeBuildInputs = [
    pkgs.openssl.dev
    pkgs.pkgconfig
    pkgs.sqlite.dev
    pkgs.rustup
  ];
}
