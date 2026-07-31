# Better RM - Better Roblox Manager by Amongus676767

A fast, lightweight Roblox account manager built with Rust and [egui](https://github.com/emilk/egui). Manage multiple Roblox accounts, launch games, and switch between sessions with ease. Rebranded and updated since currently, the original RM is nonfunctional.

> **⚠️ Disclaimer:** This tool interacts with Roblox authentication cookies and game-launching internals. Use at your own risk. The multi-instance feature bypasses Roblox's singleton mutex, which may conflict with Hyperion anti-cheat and could carry ban risk. This project is not affiliated with or endorsed by Roblox Corporation.

## Features

- **Multi-Account Management** - Add, remove, and organize Roblox accounts with cookie-based auth
- **Encrypted Storage** - AES-256-GCM encryption or Windows Credential Manager
- **Multi-Instance** - Launch multiple Roblox clients simultaneously
- **Bulk Launch** - Launch selected accounts into the same server sequentially
- **Privacy Mode** - Clears tracking cookies before each launch
- **Auto Window Tiling** - Arranges Roblox windows in a grid after launch
- **Live Presence** - Real-time Online / In Game / In Studio / Offline indicators
- **Optional themes and background effects to customize the client

## Building from Source

### Prerequisites

- [Rust](https://rustup.rs/) (stable, 1.85+ — the dependency tree requires edition 2024)
- Windows 10/11 (required for Win32 APIs)

### Build

```bash
# Clone the repository
git clone https://gitlab.com/centerepic/robloxmanager.git
cd robloxmanager

# Build in release mode
cargo build --release

# Run
cargo run --release
```

The compiled binary will be at `target/release/ram_ui.exe`.

### Development

```bash
# Check for errors without building
cargo check

# Run with debug logging (PowerShell)
$env:RUST_LOG="debug"; cargo run
```

## Usage

1. **First launch** - Set a master password when adding your first account
2. **Add accounts** - Click "+ Add Account" and paste your `.ROBLOSECURITY` cookie
3. **Launch** - Select an account, enter a Place ID, and click Launch
4. **Bulk launch** - Ctrl+click or Shift+click to select multiple accounts, then use the group panel
5. **Settings** - Configure multi-instance, privacy mode, auto-arrange, and more

## Credits

- [RobloxAccountManager](https://github.com/ic3w0lf22/Roblox-Account-Manager) by ic3w0lf22 - The original Roblox Account Manager that served as the primary reference for this project

## Background

BRM is a modified fork of RM, which was the spiritual successor to [ByeBanAsync](https://github.com/centerepic/ByeBanAsync), since just clearing RobloxCookies.dat isn't effective anymore. If you completely avoid the browser, this heavily limits Roblox's ability to link your accounts.

## License

[MIT](LICENSE)
