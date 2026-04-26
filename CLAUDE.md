# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```bash
cargo build                        # build
cargo build --release              # release build
cargo test                         # run all tests
cargo test <test_name>             # run a single test by name
cargo fmt                          # format code
cargo fmt -- --check               # check formatting without fixing
cargo clippy -- -D warnings        # lint (warnings are errors)
cargo doc --no-deps                # generate docs
```

CI runs markdownlint on all `.md` files — check `.markdownlint.yaml` for rules.

## Architecture

`dotpatina` is a Rust CLI that manages dotfiles via Handlebars-templated config files. Users define a **Patina** (a TOML file listing template→target file pairs and variables), and the tool either previews (`render`) or writes (`apply`) the rendered output.

**Data flow:**

```text
CLI args (clap)
  → PatinaEngine::render_patina() / apply_patina()
  → Patina::from_toml_file() + load_vars_files()   # loads & merges variables
  → templating::render_patina()                     # Handlebars rendering (strict, no escaping)
  → diff generation
  → user confirmation prompt (apply only)
  → file writing + permission preservation
```

**Key modules:**

- [src/cli.rs](src/cli.rs) — `PatinaCli` (clap), `CliPatinaInterface` implementing `PatinaInterface`
- [src/engine.rs](src/engine.rs) — `PatinaEngine<PI>`, the main orchestrator; generic over `PatinaInterface` so tests can inject a mock UI
- [src/engine/interface.rs](src/engine/interface.rs) — `PatinaInterface` trait (output, confirm, headers); the abstraction that decouples engine logic from I/O
- [src/patina.rs](src/patina.rs) — `Patina` struct (TOML deserialization, path resolution, tag filtering)
- [src/patina/patina_file.rs](src/patina/patina_file.rs) — `PatinaFile` (single template/target pair with optional tags and `preserve_permissions`)
- [src/patina/vars.rs](src/patina/vars.rs) — variable file loading and merging
- [src/templating.rs](src/templating.rs) — Handlebars rendering; returns `PatinaFileRender` structs
- [src/diff.rs](src/diff.rs) — `DiffAnalysis` trait and diff formatting for apply previews
- [src/utils.rs](src/utils.rs) — `Error` enum, `normalize_path()` (resolves `~` and env vars)

**`PatinaInterface` is the key seam for testing.** Engine tests create a `TestPatinaInterface` that captures output without writing to disk, allowing `render_patina` and `apply_patina` to be tested without side effects.

**Path resolution** (`normalize_path`) expands `~` to the home directory and substitutes `$ENV_VAR` references before canonicalizing — both template and target paths go through this.

## Best Practices

### Automated Testing

After making functional code changes (adding, removing, or modifying existing behavior) make sure to run tests to ensure no functionality has broken.
If the tests are not consistent with new functionality, update (add, remove, or modify) the tests to account for these changes.

### Formatting

Run `cargo fmt` after making code changes, then verify with `cargo fmt -- --check` before considering the work done.
