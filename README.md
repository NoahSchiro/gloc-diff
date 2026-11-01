# gloc (git lines of code)

A Rust CLI that reports source, test, comment, and blank line churn between
a PR branch and its base branch. Built for CI gates: fail when a PR adds
too much code.

## Usage

Build:
```bash
cargo b --release
```

Primary usage is to check how much a branch differs from the default branch:
```bash
gloc --base origin/main [--head HEAD]
```

You can also use it to look at the difference between two commits
```bash
gloc --base HEAD~1
```

## Features
- Filter files with globs or regex.
- `.gitignore` rules apply by default.
- Per-project `.gloc.toml` config.
- Return non-zero exit code if too many lines are added.

## Supported languages
- [x] Rust
- [ ] Python
- [ ] Typescript / Javascript
- [ ] HTML / CSS
- [ ] C / C++
- [ ] Java
- [ ] Go

## CI

`.github/workflows/gate.yml` gates this repo's pull requests with the latest
released binary. Copy it into your repo and tune the limits.
