# Shell configuration to build rsure.
{ pkgs ? import <nixos> {} }:
let
  lib = pkgs.lib;
  stdenv = pkgs.stdenv;
in
pkgs.mkShell {
  nativeBuildInputs = [
    pkgs.openssl.dev
    pkgs.pkg-config
    pkgs.sqlite.dev
    pkgs.rustup
  ];
}
