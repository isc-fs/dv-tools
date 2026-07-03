# Auto-update (ISC MingoROS)

On launch the app checks the signed `latest.json` attached to the newest
**dv-tools GitHub Release** and, if a newer version is published, offers
**Install & restart** (downloads → verifies the minisign signature → installs →
relaunches). Same idea as WarioCharger, done the Tauri-native way
(`tauri-plugin-updater`) — served straight from this repo's own releases, so no
second repo or cross-repo token is involved.

```
app launch → tauri-plugin-updater.check()
           → GET github.com/isc-fs/dv-tools/releases/latest/download/latest.json
           → newer + signature verifies against the pubkey in tauri.conf → banner
```

## One-time setup (required before the next release)

The plumbing is in place; it needs **two CI secrets** on `isc-fs/dv-tools` — the
minisign signing key. That's it (no PAT, no second repo).

### The signing key
A minisign keypair was generated at **`~/.tauri/mingoros-updater.key`** (private)
and `…​.key.pub` (public). The **public** key is already committed in
`tauri.conf.json` → `plugins.updater.pubkey`. Add the **private** key as repo
secrets on `isc-fs/dv-tools`:

- **`TAURI_SIGNING_PRIVATE_KEY`** = the contents of `~/.tauri/mingoros-updater.key`:
  ```bash
  gh secret set TAURI_SIGNING_PRIVATE_KEY --repo isc-fs/dv-tools < ~/.tauri/mingoros-updater.key
  ```
- **`TAURI_SIGNING_PRIVATE_KEY_PASSWORD`** = empty (the key was generated without
  a password):
  ```bash
  printf '' | gh secret set TAURI_SIGNING_PRIVATE_KEY_PASSWORD --repo isc-fs/dv-tools
  ```

> ⚠️ Keep the private key safe + backed up. Lose it and you can't sign updates the
> installed apps will accept — you'd have to ship a new pubkey and everyone
> re-installs. `bundle.createUpdaterArtifacts` is `true`, so **builds need this
> key** — set it before cutting a release (and export it locally if you build the
> app by hand). If the secret is absent the desktop build **hard-fails** (`a
> public key has been found, but no private key`), so the release can't ship the
> app bundles at all — it's a loud failure, not a silent unsigned one.

## What a release does

`mingoros-vX.Y.Z` tag → `mingoros-release.yml`:
1. builds + **signs** the updater artifacts (`.app.tar.gz` / `.AppImage` / NSIS
   `-setup.exe` + `.sig`) and attaches everything to the dv-tools GitHub Release;
2. the `updater-manifest` job rebuilds ONE combined `latest.json` (all platforms,
   from the release's own signed artifacts) and re-attaches it to the same
   release with `--clobber`. Pointing at the versioned
   `releases/download/<tag>/` asset URLs, it's the file the app polls via
   `releases/latest/download/latest.json`.

Because the release is published `prerelease: false`, GitHub's
`releases/latest/…` alias resolves to it, so the next launch of any older
install sees the new manifest and offers the update.

## Notes
- **No cross-repo token.** The manifest is attached to this repo's own release
  with the built-in `GITHUB_TOKEN`; there's no `isc-fs/iskapps` mirror and no
  `RELEASES_REPO_TOKEN`.
- **The combined manifest matters.** Each matrix OS leg of `tauri-action` uploads
  its own single-platform `latest.json`; they'd clobber down to one OS. The
  post-build `updater-manifest` job rebuilds one file covering every platform and
  overwrites it — so don't remove that job.
- **First release is the shakedown** — the combined-manifest step globs artifacts
  by extension; sanity-check the published `latest.json` platform URLs on the
  first `mingoros-v*` tag (open
  `…/releases/latest/download/latest.json` and confirm the `darwin-*`,
  `linux-x86_64`, `windows-x86_64` URLs resolve).
- Updates verify only if signed by the key whose public half is in `tauri.conf`;
  an unsigned or wrongly-signed bundle is rejected.
