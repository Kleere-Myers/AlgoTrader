---
name: permissions
description: View or reset AlgoTrader tool permissions. Use when permissions need updating or reviewing.
user-invocable: true
disable-model-invocation: true
argument-hint: [reset | show | add <rule>]
allowed-tools: Read, Edit, Bash(ls:*)
---

# AlgoTrader Permissions Manager

Manage tool permissions in `.claude/settings.local.json`.

## Task
$ARGUMENTS

## Actions

### `show` (default if no argument)
Read and display the current `.claude/settings.local.json` permissions, grouped by category.

### `reset`
Overwrite `.claude/settings.local.json` with the canonical permission set defined below.
After writing, display the result.

### `add <rule>`
Append a new permission rule to the `allow` array. Validate it follows the pattern syntax before adding. Do not duplicate existing rules.

## Canonical Permission Set

When resetting, write exactly this to `.claude/settings.local.json`:

```json
{
  "permissions": {
    "allow": [
      "Bash(ls:*)",
      "Bash(cd:*)",
      "Bash(which:*)",
      "Bash(sleep:*)",
      "Bash(find:*)",
      "Bash(grep:*)",
      "Bash(echo:*)",
      "Bash(cat:*)",
      "Bash(kill:*)",
      "Bash(lsof:*)",
      "Bash(ss:*)",
      "Bash(curl:*)",
      "Bash(source:*)",
      "Bash(export:*)",

      "Bash(python3:*)",
      "Bash(.venv/bin/python:*)",
      "Bash(.venv/bin/pip:*)",
      "Bash(pip install:*)",

      "Bash(cargo:*)",
      "Bash(rustup:*)",

      "Bash(npm:*)",
      "Bash(npx:*)",
      "Bash(node:*)",

      "Bash(git:*)",
      "Bash(gh:*)",

      "Read(//home/mmyers/Projects/**)",
      "Read(//tmp/**)",
      "Read(//home/mmyers/.cargo/**)",
      "Read(//home/mmyers/.rustup/**)",

      "WebFetch(domain:)"
    ]
  }
}
```

## Permission Pattern Reference

- `Bash(command:*)` — allow any args to `command`
- `Bash(exact command here)` — allow only that exact command
- `Read(//path/**)` — allow reading files under path (recursive)
- `WebFetch(domain:)` — allow fetching any domain (empty = all)
- `WebFetch(domain:github.com)` — allow only that domain

## Rules
- Never add permissions for `rm -rf`, `git push --force`, or `git reset --hard`
- Keep rules broad enough to avoid one-off approval spam
- Group by category (shell, python, rust, node, git, reads, web)
