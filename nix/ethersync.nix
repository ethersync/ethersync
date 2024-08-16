{naersk-lib, lib, hostPlatform, darwin, ...}: naersk-lib.buildPackage {
  src = ../daemon;
  nativeBuildInputs = lib.optionals hostPlatform.isDarwin [
    darwin.apple_sdk.frameworks.SystemConfiguration
  ];
}
