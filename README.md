# RustGuard 🛡️

A CLI-based WireGuard VPN manager written in Rust.

## Features 🚀

- List available WireGuard VPN profiles

- Start and stop VPN connections

- View currently active VPN connections

- WireGuard Configuration Editor

- Kill-Switch

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
Manager Screen:

↑ / ↓ or h / j / k / l - Navigate profiles
Enter - Connect/Disconnect VPN
D - View VPN details
Shift+K - Toggle the kill-switch
E - Edit WireGuard configuration
S - View status log
H - Open help screen
Q - Quit

Editor Screen:

Normal mode: i, a, o, h/j/k/l, x, D, ?, Ctrl+S, Esc
Insert mode: Standard text input; press Esc to return to Normal mode.

```
## Requirements 🛠️

- Rust

- WireGuard (wg-quick and wg installed)

- sudo privileges to manage VPNs

## License 📜

RustGuard is licensed under the [Apache License 2.0](LICENSE).
