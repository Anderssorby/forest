{
  description = "Forest";
  inputs = {
    nixpkgs.url = github:nixos/nixpkgs;
    flake-utils = {
      url = github:numtide/flake-utils;
      inputs.nixpkgs.follows = "nixpkgs";
    };
    naersk = {
      url = github:nix-community/naersk;
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    { self
    , nixpkgs
    , flake-utils
    , naersk
    }:
    flake-utils.lib.eachDefaultSystem (system:
    let
      lib = nixpkgs.lib;
      pkgs = nixpkgs.legacyPackages.${system};
      rustTools = import ./nix/rust.nix {
        nixpkgs = pkgs;
      };
      getRust =
        { channel ? "nightly"
        , date
        , sha256
        , targets ? [
          "wasm32-unknown-unknown"
          "wasm32-wasi"
          # "wasm32-unknown-emscripten"
        ]
        }: (rustTools.rustChannelOf {
          inherit channel date sha256;
        }).rust.override {
          inherit targets;
          extensions = [ "rust-src" "rust-analysis" ];
        };
      rust2022-03-15 = getRust { date = "2022-03-15"; sha256 = "sha256-C7X95SGY0D7Z17I8J9hg3z9cRnpXP7FjAOkvEdtB9nE="; };
      rust = rust2022-03-15;        
      # Get a naersk with the input rust version
      naerskWithRust = rust: naersk.lib."${system}".override {
        rustc = rust;
        cargo = rust;
      };
      llvmPackages = pkgs.llvmPackages_11;
      env = {
        LIBCLANG_PATH = "${llvmPackages.libclang.lib}/lib/";
        PROTOC = "${pkgs.protobuf}/bin/protoc";
        ROCKSDB = "${pkgs.rocksdb}/lib/librocksdb.so";
        C_INCLUDE_PATH = lib.concatStringsSep ":"
          [ "${llvmPackages.libclang.lib}/lib/clang/${llvmPackages.libclang.version}/include"
          ];
        PKG_CONFIG_PATH = with pkgs; lib.concatStringsSep ":"
        [ "${pkgs.udev.dev}/lib/pkgconfig"
          "${pkgs.hidapi}/lib"
          "${nghttp2.out}/lib"
          "${nghttp2.dev}/lib/pkgconfig"
          "${pkgs.openssl.out}/lib"
          "${pkgs.openssl.dev}/lib/pkgconfig"
        ];
      };
      buildInputs = with pkgs; [
        openssl
        libclang
        pkg-config
        zlib
        curl
      ];
      # Naersk using the default rust version
      buildRustProject = pkgs.makeOverridable ({ rust, naersk ? naerskWithRust rust, ... } @ args: naersk.buildPackage ({
        inherit buildInputs;
        targets = [ "forest" ];
        copyLibs = true;
        remapPathPrefix =
          true; # remove nix store references for a smaller output package
      } // env // args));

      # Load a nightly rust. The hash takes precedence over the date so remember to set it to
      # something like `lib.fakeSha256` when changing the date.
      crateName = "forest";
      root = ./.;
      # This is a wrapper around naersk build
      # Remember to add Cargo.lock to git for naersk to work
      project = buildRustProject {
        inherit root rust;
        copySources = [ "forest" "utils" "blockchain" "vm" "node" "crypto" "encoding" "ipld" "key_management" ];
      };
      test = project.override {
        doCheck = true;
      };
    in
    {
      packages = {
        ${crateName} = project;
        "${crateName}-test" = test;
      };

      defaultPackage = self.packages.${system}.${crateName};

      # `nix develop`
      devShell = pkgs.mkShell ({
        inputsFrom = builtins.attrValues self.packages.${system};
        nativeBuildInputs = [ rust ];
        buildInputs = with pkgs; buildInputs ++ [
          rust-analyzer
          clippy
          rustfmt
        ];
      } // env) ;
    });
}
