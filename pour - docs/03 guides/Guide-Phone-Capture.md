---
tags:
  - guide
  - pwa
  - serve
date created: Friday, October 2nd 2026, 10:40:00 pm
date modified: Friday, October 2nd 2026, 10:40:00 pm
---

# Guide: Capturing From Your Phone

`pour serve` is the second front door. It runs the same engine as the TUI behind a small [[ADR-005-PWA-Companion|PWA]], reachable from any browser on your LAN, and writes the same Markdown to the same vault. The vault never knows which door a capture came through.

## Starting the server

From the TUI dashboard, press `s`. The dashboard suspends, the server starts in the same terminal, and the QR code prints there. `Ctrl+C` stops the server and brings the dashboard back. One process, one terminal.

From the command line:

```bash
pour serve            # default port 8421
pour serve --port 9000
```

On startup pour prints a QR code and the raw URL. The URL carries your token as a query parameter for the first visit. After that the PWA stores the token and sends it as an `Authorization` header.

The capture path:

```
Phone → POST /api/v1/submit/:module → axum → engine → vault
```

The full HTTP contract is in [[pour-api-contract]] and `02 references/pour-openapi.yaml`.

## Add to Home Screen

iOS and Android browsers offer "Add to Home Screen". That gives the PWA an icon and app-like launch. No app store involved.

The PWA ships a vector `web/icon.svg`. Some home screens want raster PNGs. ImageMagick isn't bundled; to generate them yourself:

```bash
magick web/icon.svg -resize 180x180 web/icon-180.png
magick web/icon.svg -resize 192x192 web/icon-192.png
magick web/icon.svg -resize 512x512 web/icon-512.png
```

The binary embeds everything under `web/` at compile time and serves it at `/static/{filename}`, so PNGs placed in `web/` show up at `/static/icon-{size}.png` after a rebuild.

## Network and auth

The server binds `0.0.0.0:<port>`. Any device on the same network can reach it; nothing exposes it to the internet. Getting to it from outside your LAN is up to you. [Tailscale](https://tailscale.com) and ZeroTier both work with no extra config.

The first `pour serve` generates a `mobile_token` and writes it to `~/.pour/secrets.toml`. Token comparisons are constant-time, and the query-parameter bootstrap is only accepted when no `Authorization` header is present. To rotate the token, delete `mobile_token` from `secrets.toml`. The next `pour serve` generates a new one and prints a new QR code.

`post_write_shell` hooks don't run for phone captures unless the module sets `post_write_shell_on_serve = true`. See [[field-types#`post_write_shell`]].

## Hiding a module from the phone

```toml
[modules.secret]
mobile_visible = false
```

The PWA can't list or submit to a hidden module.

## Logs

`pour serve` writes structured, level-filtered logs to stderr. The default level is `info`. `POUR_LOG` changes it:

```bash
POUR_LOG=debug pour serve            # everything
POUR_LOG=pour=debug,tower_http=warn  # per target
```

At `info` pour logs server startup (bind address, transport, vault path), every request and response (method, URI, status, latency), auth outcomes (`accepted_via_query`, `rejected`), and submit results (module, vault path, autocreate count). It never logs token values, request bodies, field values, or anything you typed.

## What the PWA does

Module tiles, forms for every field type, submit, and history shipped in v0.3.0. Phase 2 (closed 2026-04-27) added the IndexedDB offline queue, a service-worker app-shell cache, the sub-form overlay for `create_template` fields, preset save/edit/delete/reorder, a 90-day history heatmap, bottom-tab navigation, and a paginated history list. The closeout report is `06 reports/v1.0.0-phase2-closeout.md`. TLS and mDNS (`pour.local`) are deferred; see [[pour-pwa-roadmap]].
