# AGENTS.md

arbor: a native file explorer in Rust with iced 0.14, for Windows, Linux, and macOS. Core rule: the UI thread never waits on disk. No I/O, decoding, or large-list sorting runs on it.

## Stack

- Rust 1.100.0, pinned in `rust-toolchain.toml`; edition 2021; iced 0.14 with the wgpu backend.
- Workspace: `crates/arbor-core` (core library), `crates/arbor-bench` (bench suite). Root package `arbor` (`src/`, `build.rs`).
- Node 22 for the release planner in `scripts/*.mjs`.
- Lint: Clippy with `-D warnings`. Formatting: `rustfmt.toml` (`max_width = 140`).

## Commands

```sh
rustup toolchain install                                  # pinned toolchain
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked                           # CI installs ffmpeg and poppler-utils first
cargo deny check                                          # advisories, licenses, sources (deny.toml)
node --test scripts/*.test.mjs                            # release planner
cargo build --release --locked
cargo run --release -p arbor-bench -- suite /tmp/bench-100k --check
scripts/bench-compare.sh /tmp/bench-100k                  # against ls, eza, find, fd, fzf
```

## Structure

- `src/`, `build.rs`: the `arbor` app.
- `crates/arbor-core`: core library. `crates/arbor-bench`: bench suite.
- `patches/iced_wgpu`: local patch of `iced_wgpu`, applied through `[patch.crates-io]`.
- `packaging/`: platform packaging. `scripts/`: release planner, bench, and package scripts.
- `docs/`: performance (`desempenho.md`), Windows explorer inventory, themes, visuals.
- `.github/workflows/`: `ci.yml`, `validate.yml` (base-actions), `bench.yml`, `gui-smoke.yml`, `release.yml`.

## Conventions

- Commits follow Conventional Commits, and they set the version. `feat` gives a minor bump, `fix` and other types a patch, and `!` a major.
- Every push to `main` ships a release automatically (`release.yml`). There are no manual release steps.
- Performance numbers come from the bench suite (`docs/desempenho.md`).

## Gotchas

- The toolchain is pinned on purpose. A Clippy lint change arrives in its own commit, so bump `rust-toolchain.toml` only in a deliberate, separate commit.
- CI uses `--locked`. Do not edit `Cargo.lock` by hand.
- Dependencies come only from crates.io or the `vislo` repository (`deny.toml`). New licenses must be compatible with MIT.
- When upgrading iced, keep the `arbor:`-marked line in `patches/iced_wgpu/src/window/compositor.rs`.
- `arbor-bench suite <dir>` generates the 100k-file folder in `<dir>` when it is missing (README uses `/tmp/bench-100k`).

<!-- base-actions-main-policy -->
## GitHub Actions — padrão obrigatório (@main)

- Referencie as actions internas sempre pelo branch, nunca por SHA:
  - `uses: juninmd/base-actions/<action>@main`
  - `uses: juninmd/base-actions/.github/workflows/<arquivo>.yml@main`
- Não fixe `@<sha>` em `juninmd/base-actions`: a atualização é centralizada no
  `main` e passa a valer no próximo run.
- PRs cujos checks não iniciam por billing/spending limit são mesclados pelo bot central
  `juninmd/base-actions/.github/workflows/auto-merge-billing-blocked.yml`.
  Para impedir o merge automático de um PR, aplique a label `do-not-merge`.
