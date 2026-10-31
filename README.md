# gloc (git lines of code)

A Rust CLI that reports source, test, comment, and blank line churn between
a PR branch and its base branch. Built for CI gates: fail when a PR adds
too much code.

## Usage

Primary usage is to check how much a branch differs from the default branch:
```
gloc --base origin/main [--head HEAD]
```

## Features
- Filter files with globs or regex.
- `.gitignore` rules apply by default.
- Per project `.gloc.toml` config.
- Return non-zero exit code if too many lines are added.
