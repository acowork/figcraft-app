# ACowork hosted adaptation

This repository is the **ACowork fork** of [storytold/figcraft](https://github.com/storytold/figcraft).

Goal: ship a supervised hosted app (ADR-116) that Desktop can install via the
[ACoworkAppGallery](https://github.com/acowork/ACoworkAppGallery) entry
`apps/acowork/figcraft` and open inside an iframe.

## Name and marks

The hosted app **id** stays `figcraft`. The public name is **FigCraft (ACowork)**
so it is not presented as an ArtCraft product.

- You may say, in plain text, that this work is based on FigCraft by the ArtCraft team.
- Do **not** ship or display the ArtCraft name, wordmark or logos (`docs/brand/` is not open source; see upstream `docs/brand/LICENSE-brand.txt`).
- The FigCraft **app icon** (`assets/app-icon/`) is original FigCraft artwork under MIT OR Apache-2.0. That is what the package and gallery use — not the ArtCraft mark.
- Homepage for this adaptation is this fork. Upstream stays `https://github.com/storytold/figcraft`.
- Every `.app` tree copies `LICENSE-MIT`, `LICENSE-APACHE`, `packaging/acowork/NOTICE`, and `assets/ICON-LICENSE.txt`.

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

# Real web UI from the published FigCraft web zip (no local wasm compile)
.\packaging\acowork\package.ps1 -WebArchive .\dist\figcraft-web-0.18.1.zip

# Real web UI built from this tree (needs trunk + wasm32 target)
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
3. ~~Serve a prebuilt `ui/` inside the opaque Desktop iframe~~ — CORS + wasm/js content types on the host; `allow-downloads` on the Desktop frame. `allow-same-origin` stays off.
4. Publish a GitHub Release of the package built with `-WebArchive` or `-WithWebUi`, and set `package.url` + `package.sha256` in the gallery

Until step 4, Gallery still points at the placeholder package. A package staged locally already contains the real web UI and installs without a compile.
