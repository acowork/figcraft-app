# ACowork hosted adaptation

This repository is the **ACowork fork** of [storytold/figcraft](https://github.com/storytold/figcraft).

Goal: ship a supervised hosted app (ADR-116) that Desktop can install via the
[ACoworkAppGallery](https://github.com/acowork/ACoworkAppGallery) entry
`apps/acowork/figcraft` and open inside an iframe.

## Layout

| Path | Role |
|---|---|
| [`apps/figcraft-acowork`](apps/figcraft-acowork) | Loopback host binary: `/health`, `/ui/*`, stub `/mcp` |
| [`packaging/acowork`](packaging/acowork) | `app.toml` + stage scripts for an unsigned `.app` tree |
| [`apps/figcraft-web`](apps/figcraft-web) | Upstream Trunk/WASM UI (copied into package `ui/` when ready) |

## Build a package tree (Windows)

```powershell
# Placeholder UI (fast) — enough to exercise install + iframe shell
.\packaging\acowork\package.ps1

# Real web UI (needs trunk + wasm target)
.\packaging\acowork\package.ps1 -WithWebUi
```

Then, from the ACowork monorepo (or a release of `acowork-pack-app`):

```text
acowork-pack-app --app-dir <staged-dir> --output figcraft-<ver>-windows-x86_64.app
# optional: sign with acowork-sign, publish GitHub Release, set gallery package.url + sha256
```

## Remaining work

1. ~~Serve loopback HTTP with `/health` + `/ui/`~~ — `figcraft-acowork`
2. ~~`app.toml` + stage scripts~~ — `packaging/acowork`
3. Publish a GitHub Release and set `package.url` + `package.sha256` in the gallery
4. Ship Trunk `figcraft-web` inside `ui/` (replace placeholder) for a usable design surface

Until step 3, Gallery shows the README and a repository link (no Install download).
