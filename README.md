# Issues Watcher

A bot watch issues from GitHub repos & projects.

Tells changes between two time points.

## Quick Start

Copy `config.example.toml` to `config.toml`, update your GitHub token and watched repos.

GitHub token can be generated from here [https://github.com/settings/tokens/new](https://github.com/settings/tokens/new).

Slack token can be generated from here [https://api.slack.com/apps](https://api.slack.com/apps). Make sure using the OAuth access token and user token scopes are required.

```sh
cargo build --release
./target/release/issues-watcher -c config.toml
```

Build musl with static link, `musl-tools`, `pkg-config`, `libssl-dev` are required for this.

```sh
cargo build --release --target x86_64-unknown-linux-musl
./target/x86_64-unknown-linux-musl/release/issues-watcher -c config.toml
```
