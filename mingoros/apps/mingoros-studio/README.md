# MingoROS Studio

The native desktop dashboard — MingoCAN's `can-studio` parallel for ROS. A
**Tauri 2 + Svelte 5 + TypeScript** app: the same `mingoros-core` engine as the
CLI, in a window.

It runs on the **laptop** and joins the car PC's ROS 2 DDS graph directly over
**Ethernet** (the `ros2` / RustDDS backend). The window renders a live
**Go / No-Go board** — the autonomous state machine, the EBS / RES / ASMS safety
interlocks, and mission state at a glance, for bench-commissioning a *stationary*
car. The Svelte frontend calls the Rust commands (`get_state`, `get_meta`,
`connect`) over Tauri IPC.

```
apps/mingoros-studio/
├── index.html            # Vite entry
├── package.json          # Svelte 5 + Vite + @tauri-apps/cli
├── vite.config.ts · svelte.config.js · tsconfig.json
├── public/icon.png
├── src/                  # the frontend (Svelte 5, TypeScript)
│   ├── main.ts           #   mounts App onto #app
│   ├── app.css           #   global styles (the board's design system)
│   ├── App.svelte        #   state + 250 ms poll loop + layout
│   └── lib/
│       ├── api.ts        #   typed invoke() wrappers (+ browser demo fallback)
│       ├── types.ts · model.ts   # data contract + pure parse/classify logic
│       └── components/   #   AppBar, StatusBanner, StateHero, FactCards,
│                         #   ResBar, Checklist, RawTopics
└── src-tauri/            # the Tauri app (Rust)
    ├── src/main.rs       #   thin bootstrap → mingoros_studio::run()
    ├── src/lib.rs        #   AppState + commands; auto-connects domain 0
    ├── tauri.conf.json   #   window + bundle config
    ├── entitlements.plist · capabilities/default.json
    └── icons/icon.png    #   source icon (platform variants generated)
```

## Run / build

Needs Node 20 + the Rust toolchain. First time in this dir:

```bash
npm install
npx @tauri-apps/cli icon src-tauri/icons/icon.png   # generate .icns/.ico/png set
```

Then:

```bash
npm run tauri:dev      # launches the window (Vite dev server + Rust, hot reload)
npm run tauri:build    # bundles a .app / .dmg / .deb / .AppImage
```

- `npm run dev` alone serves just the frontend at `http://localhost:5173`; with
  no Tauri host it renders a **demo** that alternates a nominal and a fault
  snapshot (handy for pure-UI work).
- `MINGOROS_FAKE=1 npm run tauri:dev` runs the *app* against the in-process fake
  backend — the real window, demo data, no ROS graph needed.
- `npm run check` runs `svelte-check` (the CI gate).

`cargo check -p mingoros-studio` (from `mingoros/`) compiles the Rust after
`npm run build` has produced `dist/`. The app is a workspace member but **not** a
default member, so plain `cargo build` / the core+CLI CI stay Tauri-free.

## Connection note

The app does DDS **over the network** to the car — designed for wired,
same-subnet **Ethernet**, where RustDDS multicast discovery works. DDS over WiFi
is unreliable (multicast discovery + lossy reliable traffic), so keep the link
wired.

## Releases (Mac / Windows / Linux)

`.github/workflows/mingoros-release.yml` builds installers for all three OSes on
a `mingoros-v*` tag (and `mingoros-studio-ci.yml` compiles all three per PR):

| OS | Studio bundle | CLI |
|----|---------------|-----|
| macOS | `.dmg` (universal — Apple Silicon + Intel) | `mingoros` (aarch64, x86_64) |
| Linux | `.deb`, `.AppImage`, `.rpm` | `mingoros` (x86_64, aarch64) |
| Windows | `.msi` + NSIS `-setup.exe` | `mingoros.exe` (x86_64) |

Windows link note: the RustDDS transport pulls `pnet`, whose Windows backend
links `Packet.lib`/`wpcap.lib`, so the Windows jobs install the **Npcap SDK**.
Running the Windows build also needs **Npcap installed** at runtime (same
requirement as Wireshark and other pcap-based tools) — https://npcap.com.
