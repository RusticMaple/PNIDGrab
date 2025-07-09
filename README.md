# PNIDGrab
A CLI PID and PNID grabber for Splatoon 2.12.1 (v288) on Cemu

![image](https://github.com/user-attachments/assets/31db40d0-0235-4ac4-9b79-689fdf9660cc)

## Output
The output contains the following:
- Player's index and name
- Player's PID (Hex and Decimal)
- Player's PNID (fetched from the API)
- Session ID
- Fetch date (when the tool was run)

## Build yourself
You'll need the Rust toolchain, cc and openssl to compile the tool.

### Installing dependencies
**Alpine Linux:**

```bash
doas apk add cargo openssl-devel alpine-sdk
```

**Arch Linux:**

```bash
sudo pacman -S --needed rust openssl
```

**Debian/Ubuntu/Linux Mint:**

```bash
sudo apt install build-essential cargo libssl-dev
```

**Fedora:**

```bash
sudo dnf install openssl-devel cargo gcc
```

**Gentoo:**

```bash
sudo emerge -a rust-bin openssl
```

**NixOS:**

Clone the repo, cd into the folder and run:
```bash
nix-shell ./shell.nix
```

**openSUSE:**

```bash
sudo zypper in rust libopenssl-3-devel patterns-devel-base-devel_basis
```

**Void Linux:**

```bash
sudo xbps-install -S base-devel openssl-devel cargo
```

### Compiling the tool
To compile, simply run:

```bash
cargo build --release
```

The executable can be found in `target/release/` afterwards.

## Running the tool
**IMPORTANT:** NixOS has a quirk where it can't run dynamically linked binaries out of the box. To get around this issue, you need to set up `nix-ld`. A reference implementation can be found [here](https://github.com/JerrySM64/dotfiles/blob/be74d805c2c11034fe121d99a50c94c777870c6f/nixos/fhs-appimage.nix)

As this tool needs access to another process' memory, it may need to be run as root or a user with sufficient permissions:

```bash
sudo ./pnidgrab
```
## Credits
* [c8ff](https://github.com/c8ff) for finding the addresses in Cemu's memory
