---
name: dev-install
description: Build and install zelligent (CLI + Zellij plugin) locally for development
disable-model-invocation: true
allowed-tools: Bash
---

Run the dev install script, using the PATH workaround for Homebrew Rust:

```bash
PATH="$HOME/.rustup/toolchains/stable-$(rustc -vV | grep host | cut -d' ' -f2)/bin:$PATH" bash dev-install.sh
```
