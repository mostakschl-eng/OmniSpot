# AGENTS.md - OmniSpot

OmniSpot is a Windows desktop application built with Rust and Slint. The Windows runtime and storage code live in `src/`. The desktop host, interface, and assets live in `desktop/`.

## Git & Version Control Rules

- **NEVER stage, commit, or push:** Do NOT run `git add`, `git commit`, or `git push`.
- Git staging, committing, and pushing are strictly and personally managed by the user.
- Agents must NEVER modify git tracking, create commits, or push to remotes.

## Build Commands

```powershell
# Build
cargo build --locked --manifest-path desktop/Cargo.toml --target-dir target/desktop

# Build (Release)
cargo build --locked --release --manifest-path desktop/Cargo.toml --target-dir target/desktop

# Test the runtime
cargo test --locked

# Build Inno Setup Installer
powershell -ExecutionPolicy Bypass -File .\build-installer.ps1
```
