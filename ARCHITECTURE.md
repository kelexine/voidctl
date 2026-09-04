<!-- Author: kelexine <https://github.com/kelexine> -->

# ADR-0001: Personal Machine Operations CLI

| Field    | Value                          |
|----------|--------------------------------|
| Status   | Accepted                       |
| Author   | kelexine                       |
| Date     | 2026-09-04                     |
| Machine  | TheVoid (HP EliteBook 840 G6, CachyOS, 8GB RAM) |
| Edition  | Rust 2024                      |
| Toolchain| rustc/cargo 1.98.0             |

---

## 1. Context

Daily friction on TheVoid currently spans three unrelated concerns handled
ad-hoc (shell aliases, manual `du`/`find` invocations, memory of what's
tracked in the dotfiles repo):

1. **Dev workflow** — jumping between project directories, and re-typing the
   same command sequences (build, test, run) per project.
2. **System hygiene** — build artifacts, stale logs/caches, and disk hotspots
   accumulate machine-wide with no single place to review and clear them.
3. **Config drift** — a private bare-repo dotfiles setup exists, but nothing
   detects when a tracked file has been modified locally, or when a new file
   should be tracked but isn't.

No existing tool (findx included — explicitly out of scope, standalone) owns
this. This ADR scopes a single new binary to own all three.

## 2. Decision

Build **`voidctl`** — a single Rust 2024 binary, git-style subcommand
interface, TOML-configured, targeting only TheVoid (single-machine, no sync
protocol, no daemon).

### 2.1 Subcommand surface

```
voidctl jump <alias>          # print path for shell wrapper to cd into
voidctl jump --add <alias> <path>
voidctl jump --list

voidctl run <alias> [cmd-name] # run a saved command for a registered project
voidctl run --add <alias> <cmd-name> "<shell command>"
voidctl run --list <alias>

voidctl clean scan             # report reclaimable space, machine-wide
voidctl clean select           # interactive multi-select + category toggles, then delete on commit

voidctl drift status           # status of dotfiles symlinks, permissions, and git repo state
voidctl drift links            # symlink integrity and mode-bit validation
```

No hidden subcommands, no REPL, no daemon. Every invocation is a single
process that does one thing and exits.

### 2.2 Shell integration (project jump)

A subprocess cannot mutate its parent shell's working directory. Following
the `zoxide` pattern: `voidctl` itself never `cd`s. It only ever *prints* a
resolved path to stdout. A thin shell function (fish + zsh, matching your
dual-shell setup) wraps the binary and `eval`s or `cd`s from the printed
path.

```fish
# ~/.config/fish/functions/j.fish
function j
    set target (voidctl jump $argv)
    and cd "$target"
end
```

```zsh
# ~/.zshrc
j() { local t; t="$(voidctl jump "$@")" && cd "$t"; }
```

This keeps the binary itself pure and testable — `jump` resolution is a
string-in/string-out function with zero shell-state coupling, independently
unit-testable.

### 2.3 Machine-wide scanning + privilege boundary

"Machine-wide" and "sudo-aware" together imply a specific hazard: a
long-running personal tool that silently expects elevated privileges is how
personal tools become attack surface. Decision:

- **`voidctl` never re-execs itself as root and never calls `sudo` internally.**
- Default scan roots: `$HOME` plus a small, explicit allowlist of
  system-level paths that are *readable* without elevation and commonly
  accumulate cruft on Arch-based systems (`/var/cache/pacman/pkg`,
  `/var/log` read-only stat pass, `/tmp`, `/var/tmp`).
- If a scan encounters a path requiring elevated permissions to read
  (`EACCES`), it is recorded as a **skipped-permission-denied** entry in the
  report, never silently swallowed, never auto-elevated.
- The report surfaces a final line: `N paths skipped (permission denied) —
  rerun under sudo to include them`. The user decides, explicitly, per
  invocation, whether to run `sudo voidctl clean scan`. If run under sudo,
  the tool detects `EUID == 0` and widens scan roots to the full
  allowlist including root-owned system caches (`/var/cache`,
  `journalctl`-reported disk usage via `journalctl --disk-usage`).
- Deletion under sudo requires the same explicit multi-select-then-commit
  flow as unprivileged mode — no separate "nuke everything" fast path.

This makes privilege a **runtime-detected mode**, not a design branch: the
same binary, same subcommands, same selection UI: it just sees more when
`EUID == 0`.

### 2.4 Cleanup categories (scan targets)

| Category           | Detection strategy                                      |
|---------------------|----------------------------------------------------------|
| Build artifacts     | Known marker files per ecosystem: `Cargo.toml`→`target/`, `package.json`→`node_modules/`, `__pycache__/` anywhere, `*.pyc` |
| Logs/temp/cache     | `*.log` age-bucketed, `/tmp`, `/var/tmp`, `~/.cache` subtrees over a configurable age threshold |
| Disk hotspots       | Top-N largest dirs/files by size, walked once, reused for the above categories (single filesystem walk, multiple classifiers) |

All three share **one walk** of the filesystem (via `jwalk` or `ignore` crate
for parallelism + `.gitignore`-aware skipping), classified in a single pass
— not three separate walks. This matters concretely on an i5-8365U: a cold
machine-wide walk is the single most expensive operation this tool performs,
so it happens once per `scan` invocation, cached to a report file that
`select` then reads.

### 2.5 Config drift detection

**Revised after inspecting the actual dotfiles repo** (originally this ADR
assumed a bare-repo/`$HOME`-as-work-tree pattern — that is not what exists).

The real setup: `~/.dotfiles` is a normal git repository. `install.sh`
creates a **fixed, explicit set of symlinks** from files inside the repo to
locations under `$HOME` (see its `_symlink` calls and the mirrored
`SYMLINK_MAP` in `uninstall.sh`). There is no fuzzy "is this file tracked"
question — the mapping is small, explicit, and already exists on disk
twice (once per script). Drift detection targets that reality directly,
sourced from one place: `voidctl.toml`.

Three independent, cheap checks — no bare-repo plumbing, no untracked-file
sweep of `$HOME`:

| Check | Meaning | Detection |
|---|---|---|
| **Link integrity & permissions** | Does `$HOME/<target>` still resolve to `<dotfiles_dir>/<source>`, and have file mode bits drifted? | `fs::read_link` per mapped pair, compare resolved path and check POSIX permissions (`std::os::unix::fs::PermissionsExt`). Catches: broken link, link replaced by a real file, missing link, or altered permission bits |
| **Repo-side drift** | Are any files *inside the repo itself* modified or staged-but-uncommitted? | `git -C <dotfiles_dir> status --porcelain` — this is an ordinary repo, so ordinary `git status` is correct and sufficient |
| **Backup accumulation** | How much has piled up in `~/.dotfiles_backup/<timestamp>/`? | Surfaced as a `clean` candidate (see §2.4), not a `drift` finding — these are disposable once you trust current state, not evidence of drift |

`voidctl` still shells out to the system `git` binary rather than embedding
`git2`/`libgit2` (same rationale as before — thin wrapper over a command
you'd run by hand), but the wrapper is now much thinner: one command
(`status --porcelain`), parsed line by line. No bare/work-tree flags
needed since this is a normal repo.

`voidctl` becomes the **single source of truth** for the symlink map going
forward — `install.sh`/`uninstall.sh` keep their own copies for now (out of
scope to touch your existing, working, CI-tested installer in this ADR),
but `voidctl.toml`'s `[drift.links]` table should be kept in sync with them
manually until/unless a future ADR proposes generating one from the other.

### 2.6 Config format

Single `~/.config/voidctl/voidctl.toml`:

```toml
[jump]
# alias -> path
findx = "/home/kelexine/projects/findx"

[jump.commands.findx]
# alias.command-name -> shell string
test = "cargo test --workspace"
build = "cargo build --release"

[clean]
scan_roots = ["/var/cache/pacman/pkg", "/tmp", "/var/tmp"]
age_threshold_days = 7
exclude = [".git", ".cargo/registry"]

[drift]
dotfiles_dir = "/home/kelexine/.dotfiles"

[drift.links]
# source (relative to dotfiles_dir) -> target (relative to $HOME)
# Mirrors uninstall.sh's SYMLINK_MAP — keep in sync manually for now.
"bash/.bashrc"                = ".bashrc"
"zsh/.zshrc"                  = ".zshrc"
"fish/config.fish"            = ".config/fish/config.fish"
"fish/functions/extract.fish" = ".config/fish/functions/extract.fish"
"git/.gitconfig"              = ".gitconfig"
"git/.gitignore_global"       = ".gitignore_global"
"ssh/config"                  = ".ssh/config"
"ripgrep/ripgreprc"           = ".config/ripgrep/ripgreprc"

[clean]
# ~/.dotfiles_backup/<timestamp>/ trees are a first-class clean target,
# not a drift finding -- see 2.4 and 2.5.
extra_scan_roots = ["/home/kelexine/.dotfiles_backup"]
```

### 2.7 Crate layout

```
voidctl/
├── Cargo.toml
├── src/
│   ├── main.rs              # clap dispatch only, no logic
│   ├── config/
│   │   ├── mod.rs           # public Config type + load()
│   │   └── schema.rs        # serde structs matching opsctl.toml
│   ├── jump/
│   │   ├── mod.rs
│   │   └── registry.rs      # add/list/resolve alias -> path
│   ├── runner/
│   │   ├── mod.rs
│   │   └── exec.rs          # saved-command execution, per-project
│   ├── clean/
│   │   ├── mod.rs
│   │   ├── walker.rs        # multi parallel fs walk
│   │   ├── classifier/{mod.rs, etc}      # artifact/log/hotspot classifiers over walk results
│   │   ├── privilege.rs     # EUID detection, root-owned path allowlist
│   │   └── select.rs        # interactive multi-select (dialoguer/inquire) + delete
│   ├── drift/
│   │   ├── mod.rs
│   │   ├── links.rs         # symlink-integrity check against configured map
│   │   └── git_shell.rs     # thin wrapper: `git status --porcelain` in dotfiles_dir
│   └── report/
│       └── mod.rs           # shared report/output formatting (used by clean + drift)
├── tests/
│   ├── jump_test.rs
│   ├── clean_test.rs
│   └── drift_test.rs
└── README.md
```

Each domain (`jump`, `runner`, `clean`, `drift`) is an independently
testable module with no cross-imports except through `config`. `main.rs`
is pure dispatch — no business logic lives there.

## 3. Alternatives Considered

| Alternative | Rejected because |
|---|---|
| Extend findx to cover this | Explicitly ruled out — different problem domain (search vs. ops), would bloat findx's scope and coupling |
| `zoxide` + `direnv` + `bleachbit` + manual git status scripts | Already the status quo — this ADR exists because that's fragmented across 4+ tools with no shared config or report format |
| Background daemon (inotify-watch for drift, continuous scan) | Rejected for now: 8GB RAM machine, adds a persistent process for a problem that's fine solved on-demand; revisit only if on-demand latency becomes the actual pain |
| TUI dashboard (ratatui) as primary interface | Rejected as primary — adds real complexity for marginal gain over a clean CLI + `select` prompt; `clean select`'s interactive multi-select is the only place richer UI earns its cost |
| Embed `libgit2` via `git2` crate for drift | Rejected — shelling out to system `git` is simpler, correct by construction (reuses git's own semantics), and this isn't a perf-critical path |
| Bare-repo/`--work-tree=$HOME` drift model (original draft of this ADR) | Rejected after inspecting the actual dotfiles repo — it's a normal repo with an explicit symlink map, not a bare-repo/work-tree setup. Designing for a workflow that doesn't exist would mean shipping a `drift` module that never matches reality |
| Reverse-engineer symlink map by parsing `install.sh`/`uninstall.sh` at runtime | Rejected for v1 — fragile (couples `voidctl` to shell-script internals, breaks silently if either script's format changes); explicit config in `voidctl.toml`, manually kept in sync, is simpler and visible |
| Auto-elevate via internal `sudo` re-exec | Rejected outright — a personal tool that silently escalates itself is a bad precedent regardless of "trusted machine" status |

## 4. Consequences

**Positive:**
- One config file, one binary, one report format across all three concerns
- Shared filesystem walk means `clean` categories are cheap to add later
  (just another classifier function over the same walk results)
- Shelling out to `git` for drift means zero risk of divergent behavior
  from your actual bare-repo workflow
- Privilege handling is transparent and auditable — one boolean (`EUID==0`)
  gates scope, no magic

**Negative / accepted tradeoffs:**
- Shelling out to `git` means a hard runtime dependency on `git` being on
  `$PATH` (acceptable — it always will be on this machine)
- No daemon means no proactive drift/cleanup notifications — purely
  on-demand (accepted per §3, revisit later if wanted)
- Single-machine only — no sync protocol, no remote state (explicitly
  out of scope per your answer)

## 5. Dependencies (proposed)

| Crate | Purpose |
|---|---|
| `clap` (derive) | subcommand parsing |
| `serde` + `toml` | config schema |
| `anyhow` | binary-level error propagation |
| `thiserror` | typed errors within library-style modules (`clean`, `drift`) |
| `ignore` | parallel, gitignore-aware fs walk (reused from ripgrep) |
| `inquire` | interactive multi-select and bulk category toggles for `clean select` |
| `tracing` + `tracing-subscriber` | structured logging |
| `humansize` | human-readable size formatting in reports |

No `tokio` — every operation here is either a fast local fs walk or a
subprocess call; async buys nothing and adds ceremony.

## 6. Testing Strategy

- `jump`: registry add/list/resolve — pure functions, fully unit-testable
  without touching real `$HOME`
- `runner`: command template resolution unit-tested; actual exec tested via
  integration test against a throwaway temp project fixture
- `clean::classify`: unit tests over synthetic `walkdir` results (no real
  fs needed — inject fixtures)
- `clean::walker` + `clean::select`: integration test against a temp
  directory tree fixture (create `target/`, `node_modules/`, aged files,
  assert classification output)
- `drift::links`: unit tests over a synthetic symlink map + `tempdir()`
  fixture tree — assert correct/broken/missing/replaced-by-real-file and
  permission bit drifts are each classified correctly
- `drift::git_shell`: integration test against a real throwaway repo
  created fresh in `tempdir()` with `git init`, not your actual
  `~/.dotfiles`
- No test ever touches your real `$HOME`, your real dotfiles repo, or
  requires root — all privilege-path logic (`clean::privilege`) is unit
  tested by injecting a mock EUID rather than requiring an actual root
  test run

## 7. Resolved Architectural Decisions

1. **Binary Name**: Confirmed as `voidctl`.
2. **Clean Selection Flow**: Both granular terminal multi-select and category-level bulk toggles supported in `clean select`.
3. **Log/Cache Age Threshold**: Default set to 7 days (`age_threshold_days = 7`).
4. **Drift Permission Tracking**: Track and report file mode / permission changes (`PermissionsExt`) on symlink targets and managed files.
