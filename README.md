# voidctl

> Unified personal machine operations CLI for TheVoid

## What is this?

`voidctl` is a unified operations CLI engineered specifically for TheVoid (HP EliteBook 840 G6, CachyOS). It eliminates friction across project navigation, system maintenance, and dotfiles management without introducing background daemons or privilege escalation hazards.

## Features

- **Project Jump**: Subprocess-pure directory jumping (`voidctl jump <alias>`) designed for lightweight fish and zsh shell integration functions.
- **Project Runner**: Manage and execute project-scoped command sequences (`voidctl run <alias> <cmd>`) across codebases.
- **System Hygiene & Cleaner**: Single-pass parallel disk traversal detecting build artifacts (`target/`, `node_modules/`, `__pycache__/`), aged logs/caches (>7 days), and disk hotspots with interactive multi-selection and bulk category toggles.
- **Config Drift Detection**: Verifies dotfiles symlink integrity, permissions/mode bits drift, and checks git status (`--porcelain`) for uncommitted changes.

## Installation

```bash
cargo build --release
cp target/release/voidctl ~/.local/bin/
```

### Shell Integration

#### fish (`~/.config/fish/functions/j.fish`)
```fish
function j
    set target (voidctl jump $argv)
    and cd "$target"
end
```

#### zsh (`~/.zshrc`)
```zsh
j() { local t; t="$(voidctl jump "$@")" && cd "$t"; }
```

## Usage

```bash
# Jump to a project
j mosaic
voidctl jump --add mosaic /home/kelexine/projects/mosaic
voidctl jump --list

# Run registered project commands
voidctl run mosaic test
voidctl run --add mosaic test "cargo test --workspace"
voidctl run --list mosaic

# Scan and clean machine cruft
voidctl clean scan
voidctl clean select

# Audit dotfiles symlinks and repository drift
voidctl drift status
voidctl drift links
```

## Architecture

See [ARCHITECTURE.md](ARCHITECTURE.md) for full architectural decision records, subsystem breakdowns, and security boundaries.

## Contributing

- Branch convention: `<type>/<short-slug>`
- Commit convention: Conventional Commits with sign-off (`Signed-off-by: kelexine <frankiekelechi@gmail.com>`)

## License

MIT OR Apache-2.0
