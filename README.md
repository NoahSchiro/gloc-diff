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

## Config

`.gloc.toml` at the repo root accepts the same options as the flags, and
flags override it:

```toml
include = ["src/**", "crates/*/src/**"]
exclude = ["**/generated/**"]
ignore-whitespace = true
max-src = 500
max-test = 300
max-total = 1000
```

## Supported languages
- [x] Rust
- [x] Python
- [x] Typescript / Javascript
- [x] HTML / CSS (separate `html` and `css` features)
- [x] C
- [x] C++
- [ ] Java
- [x] Go

Each language is a cargo feature, and only Rust is enabled by default. This
can be removed with `--no-default-features`. Otherwise, specify which language
you want to use via `--features "lang1","lang2"` and so on. Or `--features all`
if you want all languages.

## CI

`.github/workflows/gate.yml` gates this repo's pull requests with the latest
released binary. Copy it into your repo and tune the limits.
