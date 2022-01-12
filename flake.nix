{
  description = "Forest filecoin server";
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
    utils = {
      url = github:yatima-inc/nix-utils;
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.naersk.follows = "naersk";
    };
  };

  outputs =
    { self
    , nixpkgs
    , flake-utils
    , utils
    , naersk
    }:
    flake-utils.lib.eachDefaultSystem (system:
    let
      lib = utils.lib.${system};
      pkgs = nixpkgs.legacyPackages.${system};
      inherit (lib) buildRustProject testRustProject rustDefault filterRustProject;
      rust = rustDefault;
      crateName = "forest";
      root = ./.;
      llvmPackages = pkgs.llvmPackages_13;
      env = {
        LIBCLANG_PATH = "${llvmPackages.libclang.lib}/lib/";
        PROTOC = "${pkgs.protobuf}/bin/protoc";
        ROCKSDB = "${pkgs.rocksdb}/lib/librocksdb.so";
        C_INCLUDE_PATH = lib.concatStringsSep ":"
          [ "${llvmPackages.libclang.lib}/lib/clang/${llvmPackages.libclang.version}/include"
          ];
        PKG_CONFIG_PATH = lib.concatStringsSep ":"
        [ "${pkgs.libudev.dev}/lib/pkgconfig"
          "${pkgs.hidapi}/lib"
          "${pkgs.openssl.out}/lib"
          "${pkgs.openssl.dev}/lib/pkgconfig"
        ];
      };
      buildInputs = with pkgs; [
        openssl
        libclang
        pkg-config
      ];
      project = buildRustProject ({
        inherit buildInputs root;
        copySources = [ "utils" "blockchain" "vm" "node" "crypto" "encoding" "ipld" "key_management" ];
      } // env);
    in
    {
      packages.${crateName} = project;
      checks.${crateName} = testRustProject { inherit root; };

      defaultPackage = self.packages.${system}.${crateName};

      # To run with `nix run`
      apps.${crateName} = flake-utils.lib.mkApp {
        drv = project;
      };

      # `nix develop`
      devShell = pkgs.mkShell ({
        inputsFrom = builtins.attrValues self.packages.${system};
        nativeBuildInputs = [ rust ];
        buildInputs = with pkgs; buildInputs ++ [
          rust-analyzer
          clippy
          rustfmt
        ];
      } // env);
    });
}
