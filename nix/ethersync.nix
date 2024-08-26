{rustPlatform, lib, stdenv, darwin, ...}: rustPlatform.buildRustPackage rec {
  # when upstreaming this to nixpkgs,
  # - change the version to the version you plan to pin
  # - replace src with a fetchFromGitHub derivation that fetches ethersync at the specified, released version
  # - replace the cargoLock.lockFile attribute with the cargoHash attribute
  # - finally add yourself to meta.maintainers

  pname = "ethersync";
  version = "latest";
  src = ../daemon;

  cargoLock.lockFile = "${src}/Cargo.lock";

  buildInputs = lib.optionals stdenv.isDarwin [
    darwin.apple_sdk.frameworks.CoreServices
    darwin.apple_sdk.frameworks.SystemConfiguration
  ];

  meta = {
    description = "real-time co-editing of text files across multiple editors";
    homepage = "https://github.com/ethersync/ethersync";
    licenses = [ lib.licenses.agpl3Only ];
  };
}
