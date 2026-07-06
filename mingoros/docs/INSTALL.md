# Installing ISC MingoROS

Download the latest artifacts from
[**Releases**](https://github.com/isc-fs/dv-tools/releases) (tag `mingoros-v*`),
then follow your platform below. `<ver>` is the release version, e.g. `0.4.2`.

---

## Linux (x86_64 / amd64)

The desktop app is built on **Tauri 2 / WebKitGTK 4.1**, so it needs a glibc
distro that ships **`libwebkit2gtk-4.1-0`** — in practice **Ubuntu 22.04 or
newer** (or an equivalent).

> ⚠️ **Ubuntu 20.04 is not supported** — it has no WebKitGTK 4.1 package. Use
> 22.04+ for the desktop app. (The `mingoros` **CLI** still works on 20.04.)

### `.deb` — Debian / Ubuntu (recommended)

```bash
sudo apt install ./ISC.MingoROS_<ver>_amd64.deb
```

Install with **`apt`, not `dpkg -i`** — `apt` resolves the declared
dependencies (`libwebkit2gtk-4.1-0`, `libgtk-3-0`). If you already ran
`sudo dpkg -i …` and it reported unmet dependencies, recover with:

```bash
sudo apt install -f
```

On **Ubuntu 22.04**, `libwebkit2gtk-4.1-0` lives in the `universe` component —
enable it first if needed: `sudo add-apt-repository universe`.

### `.AppImage` — portable, no install

```bash
chmod +x ISC.MingoROS_<ver>_amd64.AppImage
./ISC.MingoROS_<ver>_amd64.AppImage
```

Ubuntu 22.04/24.04 ship FUSE 3, but the AppImage runtime needs **FUSE 2**:

```bash
sudo apt install libfuse2
```

### `.rpm` — Fedora / RHEL / openSUSE

```bash
sudo dnf install ./ISC.MingoROS-<ver>-1.x86_64.rpm
```

### CLI (`mingoros`)

```bash
tar -xzf mingoros-mingoros-v<ver>-x86_64-unknown-linux-gnu.tar.gz
./mingoros --help
```

The CLI depends only on glibc + **`libudev1`** (present on any systemd distro —
so it also runs on 20.04). It does **not** need pcap on Linux.

### Troubleshooting (Linux)

| Symptom | Cause | Fix |
|---|---|---|
| `dependency is not satisfiable: libwebkit2gtk-4.1-0` | Ubuntu < 22.04, `universe` disabled, or installed with `dpkg -i` | Use Ubuntu 22.04+, `sudo add-apt-repository universe`, then `sudo apt install ./file.deb` |
| `AppImage requires FUSE to run` | FUSE 2 not installed | `sudo apt install libfuse2` |
| Installs, but the window is blank / won't open | WebKitGTK missing or too old | Confirm `libwebkit2gtk-4.1-0` is installed (Ubuntu 22.04+) |
| CLI: `error while loading shared libraries: libudev.so.1` | No udev (very rare) | `sudo apt install libudev1` |

> **Note:** unlike the Windows build, the Linux build does **not** use pcap.
> Its `pnet` dependency talks to raw `AF_PACKET` sockets, so **no libpcap /
> Npcap is required** — the only shared-library deps are WebKitGTK + GTK (app)
> and libudev (CLI).

---

## macOS

`.dmg` — universal (Apple Silicon + Intel), **ad-hoc signed**. On first launch
Gatekeeper will block it; **right-click → Open → confirm** once, then it opens
normally thereafter.

---

## Windows

`-setup.exe` (NSIS installer). Needs **[Npcap](https://npcap.com)** installed at
runtime — on Windows the RustDDS transport uses the pcap libraries (`wpcap.dll`).
Install Npcap (the same driver Wireshark uses) before first launch.

---

Once installed, connect the laptop to the car's DV PC over a direct Ethernet
cable per **[CONNECT.md](CONNECT.md)**. Auto-update details are in
**[UPDATES.md](UPDATES.md)**.
