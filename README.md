![ISC Logo](http://iscracingteam.com/wp-content/uploads/2022/03/Picture5.jpg)

# IFS08 · dv-tools

Operational tooling for the **IFS08 Driverless** stack — host-side utilities
that support [IFSSIM](https://github.com/isc-fs/IFSSIM), the
[DV pipeline](https://github.com/isc-fs/IFS08-DV_PIPELINE) and the
[uDV](https://github.com/isc-fs/IFS08-DV-uDV) micro-ROS gateway. A polyglot
monorepo: each tool is a self-contained subdirectory with its own build.

## Tools

| Tool | Language | Purpose |
|---|---|---|
| [`hesai-pcap2mcap`](hesai-pcap2mcap/) | Python + Docker | Replay a Hesai LiDAR `.pcap` capture through the Hesai driver and record it into an MCAP bag that opens directly in Lichtblick / Foxglove. |
| [`mingoros`](mingoros/) | Rust | ROS2-topic debugger for the DV stack — `topics` / `echo` / `hz` / `pub`, a live safety/state **dashboard** (`state`) for stopped-car commissioning, and uDV bring-up (`udv` detect + `agent` bridge). MingoCAN, but for ROS topics. |

## Repository layout

Each tool lives in its own top-level directory and owns its build system
(Dockerfile, Cargo workspace, etc.). There is no repo-wide build — clone the
repo, `cd` into a tool, follow that tool's `README`.

---

## How we work with this repository

This repo follows the same branch model as the rest of the IFS08 Driverless
repos ([IFS08-DV_PIPELINE](https://github.com/isc-fs/IFS08-DV_PIPELINE),
[IFS08-DV-uDV](https://github.com/isc-fs/IFS08-DV-uDV)).

### Main branches

Two permanent branches, **neither worked on directly**:

- **`main`** — production. Only validated code. Protected: changes arrive by PR.
- **`dev`** — integration. Where everyone's work comes together. Protected: changes arrive by PR from a feature branch.

```
main  ──────────────────●──────────────────────●──▶  (validated releases only)
                        ↑                      ↑
dev   ──────●───●───●───●───●───●───●───●───●──●──▶  (continuous integration)
            ↑   ↑       ↑   ↑   ↑       ↑   ↑
          feat/1 fix/1 feat/2 fix/2   feat/3 fix/3
```

### Feature branches

All work — feature or fix — happens on a branch cut from `dev`, opened as a PR
**toward `dev`**, reviewed, merged, and deleted. Two independent numeric
counters:

```
feat/<n>   →  new functionality  (feat/1, feat/2, feat/3 ...)
fix/<n>    →  bug fix            (fix/1,  fix/2,  fix/3  ...)
```

`feat` and `fix` counters are independent: `feat/2` and `fix/2` can coexist.

### Tracking branch history

Feature branches are deleted after merge; their history is preserved in
**GitHub Issues**. A GitHub Actions workflow (`.github/workflows/branch-issue.yml`)
opens a tracking issue automatically when a `feat/*` or `fix/*` branch is
pushed — labelled `feat`/`fix`, titled `[feat/N] …`, with the description
auto-filled from your first commit message. It also warns if the branch number
isn't the next expected one. A second workflow
(`.github/workflows/close-on-dev-merge.yml`) closes the linked issue when the
PR merges into `dev` (via `Closes #N` in the PR body).

- **Active branches:** filter issues by label + status `open`.
- **Full history:** filter by label + status `closed`.
- **Next number:** last closed issue of that type + 1.

### Step by step

```bash
# 1. cut the branch from an up-to-date dev
git checkout dev && git pull origin dev
git checkout -b feat/5            # next available number for its type

# 2. push — the tracking issue opens automatically within seconds
git push -u origin feat/5

# 3. work, commit (the FIRST commit message fills the issue description), push
git commit -am "short description of what this does"
git push

# 4. open a PR toward dev, with `Closes #<issue-number>` in the body
gh pr create --base dev
```

Merging `dev → main` happens only after full validation, via a PR opened by a
responsible team member.

---

*ISC Racing Team — IFS08 Driverless*
