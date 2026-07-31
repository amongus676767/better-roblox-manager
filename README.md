# BRM — Better Roblox Manager by Amongus676767

A fast, lightweight Roblox account manager built with Rust and [egui](https://github.com/emilk/egui). Manage multiple accounts, launch games, and switch sessions without touching a browser.

A fork of [RM](https://gitlab.com/centerepic/robloxmanager), created to fix a bug that silently destroyed saved account lists — then extended with a full appearance system.

[![Release](https://img.shields.io/github/v/release/amongus676767/Better-Roblox-Manager?style=flat-square)](https://github.com/amongus676767/Better-Roblox-Manager/releases/latest)
[![License](https://img.shields.io/badge/license-MIT-blue?style=flat-square)](LICENSE)
[![Platform](https://img.shields.io/badge/platform-Windows%2010%2F11-blue?style=flat-square)]()

> **⚠️ Disclaimer**
> This tool handles Roblox authentication cookies and game-launching internals. Use at your own risk.
>
> `.ROBLOSECURITY` cookies grant full account access and **bypass 2FA** — treat them like passwords, and only ever run builds from source you trust.
>
> The **multi-instance** feature bypasses Roblox's singleton mutex, which may conflict with Hyperion anti-cheat and **could carry ban risk**. It is off by default and opt-in.
>
> Not affiliated with or endorsed by Roblox Corporation.

---

## Download

Grab the latest `brm.exe` from the [**Releases page**](https://github.com/amongus676767/Better-Roblox-Manager/releases/latest). No installer — it's a single portable executable.

Prefer to build it yourself? [Skip to Building from Source](#building-from-source).

---

## Why this fork exists

RM stored your accounts in one of two backends — an encrypted file, or Windows Credential Manager. Three bugs in that layer compounded into a genuinely destructive one:

- In Credential Manager mode the app never asked for a master password, but the save path refused to run without one. **Every save was silently dropped**, so the entire account list vanished the moment you closed the app.
- Switching storage backends migrated nothing, stranding cookies in the backend you'd just left and producing `Keyring error: no matching entry found in data storage`.
- Cookie reads were all-or-nothing against a single backend, so once the two fell out of sync there was **no way back** — even though the cookie was usually still sitting intact in the other one.

BRM fixes all three. Reads now fall back between backends, which repairs already-broken installs **without re-adding a single account**. See [issue #6](https://github.com/centerepic/robloxmanager/issues/6) and the [changelog](CHANGELOG.md) for the full account.

---

## Features

### Account management
- **Multi-account** — add, remove, group and reorder accounts with cookie-based auth
- **Encrypted storage** — AES-256-GCM behind a master password, or Windows Credential Manager (DPAPI-backed, no prompt on launch)
- **Live presence** — real-time Online / In Game / In Studio / Offline indicators
- **Bulk launch** — send multiple accounts into the same server sequentially
- **Presets & private servers** — save launch configurations and private server links
- **Privacy mode** — clears tracking cookies before each launch
- **Anonymize** — blur avatars and mask names for screenshots and streaming

### Launching
- **Multi-instance** — run multiple Roblox clients at once *(opt-in, see disclaimer)*
- **Auto window tiling** — arrange Roblox windows in a grid after launch
- **Launch delay** — throttle sequential launches, since Roblox rate-limits some IPs

### Appearance
- **Custom backgrounds** — PNG, JPG, GIF and WebP, including animated GIF and WebP, with opacity, fit modes, a dim scrim and panel transparency
- **Six themes** — Cosmos, Nebula, Aurora, Abyss, Ember and Slate, switchable without restarting
- **Animated effects** — nebula clouds, starfield, rain and a cursor glow, each independently toggleable *(all off by default)*
- **Rain ambience** — synthesised in real time, or supply your own audio file
- **Corner image** — optional decorative art from nekos.best / nekos.life, with artist credit
- **CJK font support** — Japanese, Chinese and Korean text renders properly instead of empty boxes

Everything visual is **off by default**. A fresh install looks and behaves like a plain utility app.

---

## Building from Source

### Prerequisites

| Requirement | Notes |
|---|---|
| Windows 10/11 | Required — the app uses Win32 APIs and Windows Credential Manager |
| [Rust](https://rustup.rs/) 1.85+ | The dependency tree requires edition 2024 |
| VS Build Tools | "Desktop development with C++" workload — supplies the MSVC linker |

Without the C++ workload the build fails at the link step with `linker 'link.exe' not found`. Install it with:

```powershell
winget install Microsoft.VisualStudio.2022.BuildTools --override "--quiet --wait --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
```

### Build

```powershell
git clone https://github.com/amongus676767/Better-Roblox-Manager.git
cd Better-Roblox-Manager
cargo build --release
```

The binary lands at `target\release\brm.exe`.

### Development

```powershell
# Fast error check without a full build
cargo check --workspace

# Lint (CI gates on this)
cargo clippy --workspace -- -D warnings

# Run with debug logging
$env:RUST_LOG="debug"; cargo run --release
```

Logs are written to `%APPDATA%\RM\rm.log`.

---

## Usage

1. **First launch** — set a master password when adding your first account, or switch on Windows Credential Manager in Settings to skip password prompts entirely
2. **Add accounts** — click **+ Add Account** and either sign in through the built-in browser or paste a `.ROBLOSECURITY` cookie
3. **Launch** — select an account, enter a Place ID, hit Launch
4. **Bulk launch** — Ctrl+click or Shift+click to multi-select, then use the group panel
5. **Customize** — Settings → Appearance for backgrounds, themes and effects

### Where your data lives

| Path | Contents |
|---|---|
| `%APPDATA%\RM\accounts.dat` | Encrypted account store (plus a `.bak` copy) |
| `%APPDATA%\RM\config.json` | Settings and preferences |
| `%APPDATA%\RM\presets\` | Saved launch presets |
| Credential Manager → `RM-Rust` | Cookies, when using the Credential Manager backend |

Back up `accounts.dat` before experimenting. If it ever fails to load, the `.bak` copy is tried automatically.

---

## Credits

- **[Roblox Account Manager](https://github.com/ic3w0lf22/Roblox-Account-Manager)** by ic3w0lf22 — the original, and the primary reference for this project
- **[RM / RobloxManager](https://gitlab.com/centerepic/robloxmanager)** by sashaa169 — the spiritual successor to RAM, and the direct upstream of BRM

BRM is a fork, not a rewrite. The account handling, launcher and Roblox API layer are upstream's work.

---

## Background

RM was the spiritual successor to [ByeBanAsync](https://github.com/centerepic/ByeBanAsync), built because simply clearing `RobloxCookies.dat` stopped being effective. Avoiding the browser entirely limits Roblox's ability to link your accounts.

BRM continues that, with the storage layer repaired and a customization system on top.

---
## Notes

Developed with assistance from [Claude](https://claude.ai). All changes were reviewed, built and tested before shipping.

---

## License

[MIT](LICENSE) — same as upstream. The original copyright notice is retained.
