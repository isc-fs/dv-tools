# Auto-update (ISC MingoROS)

On launch the app checks a signed `latest.json` on the shared **iskApps** channel
and, if a newer version is published, offers **Install & restart** (downloads →
verifies the minisign signature → installs → relaunches). Same pattern as
WarioCharger, done the Tauri-native way (`tauri-plugin-updater`).

```
app launch → tauri-plugin-updater.check()
           → GET raw.githubusercontent.com/isc-fs/iskapps/main/mingoros/latest.json
           → newer + signature verifies against the pubkey in tauri.conf → banner
```

## One-time setup (required before the next release)

The plumbing is in place; it needs **three CI secrets** + the **iskApps** repo.

### 1. The signing key
A minisign keypair was generated at **`~/.tauri/mingoros-updater.key`** (private)
and `…​.key.pub` (public). The **public** key is already committed in
`tauri.conf.json` → `plugins.updater.pubkey`. Add the **private** key as repo
secrets on `isc-fs/dv-tools`:

- **`TAURI_SIGNING_PRIVATE_KEY`** = the contents of `~/.tauri/mingoros-updater.key`
  (`cat ~/.tauri/mingoros-updater.key | pbcopy`, paste into the secret).
- **`TAURI_SIGNING_PRIVATE_KEY_PASSWORD`** = empty (the key was generated without
  a password).

> ⚠️ Keep the private key safe + backed up. Lose it and you can't sign updates the
> installed apps will accept — you'd have to ship a new pubkey and everyone
> re-installs. `bundle.createUpdaterArtifacts` is `true`, so **builds need this
> key** — set it before cutting a release (and export it locally if you build the
> app by hand).

### 2. The iskApps channel
- Create/confirm **`isc-fs/iskapps`** exists with a `main` branch (it already
  hosts `wario-charger/`). MingoROS uses the path **`mingoros/`**.
- Add **`RELEASES_REPO_TOKEN`** as a repo secret on `isc-fs/dv-tools`: a
  fine-grained PAT with **Contents: read & write on `isc-fs/iskapps`**. If it's
  absent the mirror job logs a warning and skips (the release still succeeds).

## What a release does

`mingoros-vX.Y.Z` tag → `mingoros-release.yml`:
1. builds + **signs** the updater artifacts (`.app.tar.gz` / `.AppImage` / NSIS
   `.exe`) and attaches everything to the dv-tools GitHub Release;
2. `mirror-iskapps` publishes the installers + updater artifacts to
   `isc-fs/iskapps` (release `mingoros-vX.Y.Z`) and commits a combined
   `latest.json` (all platforms) to `iskapps/mingoros/latest.json` — the endpoint
   the app polls.

The next launch of any older install sees the new manifest and offers the update.

## Notes
- **The endpoint is public** — `dv-tools` is public too, so you could instead
  point `plugins.updater.endpoints` straight at the dv-tools release
  (`…/releases/latest/download/latest.json`) and drop the iskApps mirror. The
  iskApps channel keeps all team apps in one place, matching WarioCharger.
- **First release is the shakedown** — the combined-manifest step globs artifacts
  by extension; sanity-check the published `latest.json` platform URLs on the
  first `mingoros-v*` tag.
- Updates verify only if signed by the key whose public half is in `tauri.conf`;
  an unsigned or wrongly-signed bundle is rejected.
