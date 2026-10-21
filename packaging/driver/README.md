# msi-ec-dkms — packaged driver source

This is the packaging for the `msi-ec-dkms` release asset, built from the
patched driver source installed in `/usr/src/msi_ec-0.13` (the MS-15HK tree,
with the Linux 7.2 `acpi_battery_hook_register()` compatibility fix).

The package only owns the DKMS **source** under `/usr/src/msi_ec-0.13`; the
module itself is built by DKMS for each kernel (normal for DKMS).

The installer downloads the prebuilt package from the release. To rebuild it
after changing the source:

```sh
cd packaging/driver
tar -C /usr/src --exclude='*.bak*' -czf msi_ec-0.13.tar.gz msi_ec-0.13
sha256sum msi_ec-0.13.tar.gz   # put the hash in PKGBUILD
makepkg -f --nodeps
```

The package provides/conflicts with `msi-ec-dkms-git`, so only one DKMS
registration for `msi_ec` can be installed.
