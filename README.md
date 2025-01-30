# RustGuard 🛡️

RustGuard is a terminal-based WireGuard VPN manager written in Rust.

## Features 🚀

- List available WireGuard VPN profiles

- Start and stop VPN connections with a single key press

- View currently active VPN connections

- Status log for recent actions

- Simple keyboard navigation

## Installation ⚙️

Make sure you have Rust installed. Then, clone the repository and build the project:
```sh
git clone https://github.com/mmrmagno/rustguard.git
cd rustguard
cargo build --release
```
## Usage 🖥️

### Run RustGuard with:

```sh
sudo ./target/release/rustguard
```

### Controls:

```
↑ / ↓ or h / j / k / l - Navigate profiles

Enter - Connect/Disconnect VPN

S - View Status Log

W - Return to VPN Manager

Q - Quit
```
## Requirements 🛠️

- Rust

- WireGuard (wg-quick and wg installed)

- sudo privileges to manage VPNs
