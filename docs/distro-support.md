# Distro Support in ForgeISO

This document describes the install method, capabilities, and limitations for each supported distro.

---

## Ubuntu

**Install method:** cloud-init autoinstall (`nocloud` datasource)

**Support level:** Full — CI-tested

**How it works:**

ForgeISO generates a `user-data` YAML (cloud-init autoinstall format) and a blank `meta-data` file, places them in `/nocloud/` at the ISO root, and patches the boot config with:

```
autoinstall ds=nocloud;s=/cdrom/nocloud/
```

Ubuntu's installer (subiquity) reads this datasource automatically, applies the config, and installs completely hands-free.

**What is configured:**

- Locale, keyboard, timezone
- Hostname, user account with SHA-512-crypt password hash
- SSH (authorized keys, password auth toggle, server install)
- Static IP or DHCP, DNS, NTP, proxy
- APT mirror, custom APT repos (PPAs)
- Packages, storage layout (lvm/direct/zfs), LUKS encryption
- UFW firewall, swap file, sysctl, mounts
- Systemd service enable/disable
- Docker, Podman
- GRUB timeout/cmdline/default
- Late commands (cloud-init `late-commands`)
- Post-install run commands

**Limitations:**

- Requires Ubuntu Server ISO (not desktop live ISO — desktop uses a different autoinstall path)
- LUKS passphrase is stored in the cloud-init YAML in plaintext (by design — cloud-init requirement)
- Merge mode (`--autoinstall user-data.yaml`) requires a valid existing cloud-init YAML

---

## Fedora

**Install method:** Kickstart (`ks.cfg` embedded in ISO root)

**Support level:** Full — CI-tested

**How it works:**

ForgeISO generates a `ks.cfg` Kickstart config and places it at the ISO root. The boot config is patched with:

```
inst.ks=cdrom:/ks.cfg
```

Fedora's Anaconda installer reads the Kickstart automatically and installs without prompts.

**What is configured:**

- Locale, keyboard, timezone
- Hostname (via `network` directive)
- User account with SHA-512-crypt password hash, groups, sudo
- SSH authorized keys
- Static IP or DHCP, DNS, NTP
- DNF mirror override, custom DNF repos
- Packages, storage layout
- Firewall (firewalld)
- Service enable/disable
- Sysctl, mounts, custom `%post` commands

**Limitations:**

- RHEL family (CentOS Stream, RHEL) uses the same code path but is not CI-tested
- Complex partition layouts (RAID, LVM with custom volumes) are not yet supported — uses the simplest `autopart` directive
- `%pre` hook is not generated (only `%post`)

---

## Linux Mint

**Install method:** preseed.cfg (Calamares-based live installer)

**Support level:** Partial — not CI-tested with a real ISO

**How it works:**

Linux Mint uses Calamares as its live desktop installer. Calamares supports Debian-style preseed files for unattended installation. ForgeISO generates a `preseed.cfg` and places it at the ISO root. The boot config is patched with:

```
auto=true priority=critical preseed/file=/cdrom/preseed.cfg
```

This tells Calamares to operate in automated mode using the preseed file.

**What is configured:**

- Locale, keyboard, timezone
- Hostname, user account (SHA-512-crypt password)
- User groups
- Network (DHCP or static via preseed network stanzas)
- APT mirror
- Partition method (guided, entire disk, ext4)
- Extra packages
- NTP server

**Limitations:**

- Calamares preseed support varies by Mint version and Calamares build — not all fields may be respected
- Overlay-only remaster (without preseed) is also supported via the `build` subcommand
- Complex partition layouts are not supported
- Not CI-tested with a real Mint ISO — use Ubuntu if you need CI-verified results

**Recommended approach for production:**

Test the generated ISO in a VM before deploying to bare metal. The preseed path is less well-specified than Ubuntu's cloud-init or Fedora's Kickstart.

---

## Arch Linux

**Install method:** archinstall JSON config + `archiso_script=` boot hook

**Support level:** Partial — not CI-tested with a real ISO

**How it works:**

ForgeISO generates an `archinstall-config.json` and a `run-archinstall.sh` launcher script, placing both in `/arch/boot/` inside the ISO. The boot entries are patched with:

For syslinux (APPEND line):
```
archiso_script=/arch/boot/run-archinstall.sh
```

For systemd-boot (options line):
```
archiso_script=/arch/boot/run-archinstall.sh
```

When the Arch live environment boots, `archiso_script=` causes archiso to execute the script, which runs:
```bash
archinstall --config /run/archiso/bootmnt/arch/boot/archinstall-config.json: --silent
```

**What is configured:**

- Hostname, user account (SHA-512-crypt password)
- Timezone, locale, keyboard layout
- Mirror region
- Extra packages, enabled services

**Limitations:**

- Requires archiso 2023+ for `archiso_script=` support
- The archinstall config format has changed across archinstall versions — the generated JSON may need adjustment for older/newer archinstall
- Full archinstall features (complex disk layouts, multiple profiles, network config) are not exposed through InjectConfig
- The `--silent` flag suppresses prompts; archinstall may still halt if required fields are missing from the config
- Not CI-tested with a real Arch ISO

**Manual fallback:**

If the automatic trigger fails, you can boot the Arch live environment normally and run:
```bash
archinstall --config /run/archiso/bootmnt/arch/boot/archinstall-config.json:
```

---

## RHEL-family (CentOS Stream, Rocky Linux, AlmaLinux)

**Install method:** Kickstart (same code path as Fedora)

**Support level:** Community — not CI-tested

These distros use Anaconda and Kickstart in the same way as Fedora. The `--distro fedora` flag generates a compatible `ks.cfg`. You must supply your own ISO (ForgeISO does not distribute RHEL family ISOs).

Boot entry patching uses `inst.ks=cdrom:/ks.cfg` which is standard across all RHEL-family installers.

---

## Summary table

| Distro | Flag | Install method | Unattended | CI tested | Notes |
|---|---|---|---|---|---|
| Ubuntu | `--distro ubuntu` (default) | cloud-init autoinstall | Full | Yes | Production-ready |
| Fedora | `--distro fedora` | Kickstart | Full | Yes | Production-ready |
| Linux Mint | `--distro mint` | preseed.cfg | Full (limited) | No | Test in VM first |
| Arch Linux | `--distro arch` | archinstall JSON | Partial | No | May need manual steps |
| RHEL family | `--distro fedora` | Kickstart | Full | No | BYO ISO; use same flag as Fedora |
