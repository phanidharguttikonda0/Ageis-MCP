

<!-- codebase:start -->
## Project Overview

**Ageis-MCP** is a Rust-based application currently in early development stage (alpha). This is a modular Rust project using Cargo as the package manager. The project follows a standard modular architecture pattern with separate directories for core business logic, modules, interfaces, tests, and documentation.

### Project Commands
| Task | Command |
|---|---|
| `dev` | `cargo run` |
| `build` | `cargo build` |
| `test` | `cargo test` |
| `lint` | `cargo clippy` |
| `format` | `cargo fmt` |

## Architecture

The codebase follows a modular architecture pattern with clear separation of concerns:

- **src/main.rs** - Entry point for the application
- **src/core/** - Core business logic (not yet implemented)
- **src/modules/** - Feature modules (not yet implemented)
- **src/interfaces/** - Interface definitions, contracts, and types (not yet implemented)
- **tests/** - Test files (not yet implemented)
- **docs/** - Project documentation including architecture, implementation guides, and product brief

The project uses Cargo as the build system and package manager, with a standard Rust project structure.

## Key Conventions

- **Rust 2024 Edition** - The project uses Rust 2024 edition
- **Modular Architecture** - Clear separation between core business logic, feature modules, and interfaces
- **No External Dependencies** - Currently has zero external dependencies (vanilla Rust)
- **Standard Cargo Commands** - Uses standard Cargo commands for building, testing, and development
- **Documentation-First** - Maintains comprehensive documentation in the `docs/` directory

## Current Status

The project is in **early development (alpha)**. Currently has no open issues or pull requests. The codebase has uncommitted changes with setup scaffolding in place. No active milestone configured yet. The project is ready for feature development and issue tracking.

### Session Start
```
npx codebase brief
```
Returns: project identity, tech stack, commands, structure, current status, next task, blockers, decisions — everything you need in one call.

### Commands (your interface — use these, don't read files)
| Command | What it returns |
|---|---|
| `npx codebase brief` | Full project briefing — **run this first** |
| `npx codebase next` | Highest-priority task + what's in progress |
| `npx codebase status` | Kanban board, priorities, milestones |
| `npx codebase query <path>` | Any data point (e.g. `stack.languages`, `commands.test`) |
| `npx codebase issue create "title"` | Track a bug, feature, or TODO |
| `npx codebase issue close <n> --reason "why"` | Close an issue after fixing it |
| `npx codebase handoff` | Generate HANDOFF.md — session transfer for next agent |
| `npx codebase tokens` | Show token budget report for this project |

### Maintenance
| Command | What it does |
|---|---|
| `codebase doctor` | Health check — diagnose broken setup |
| `codebase fix` | Auto-repair issues found by doctor |
| `codebase skills` | List installed skills and their capabilities |

### Rules
- **Never explore the codebase to discover stack/commands/structure** — it's already in `brief`
- **Check `next` before starting work** — don't duplicate in-progress tasks
- **Create issues for bugs/TODOs you find** — keep the project brain alive
- **Close issues when you fix them** — with a reason so the team knows why
- **If any command fails, run `codebase doctor` then `codebase fix`** — self-heal before asking for help

### Workflow Tips
- **Subagents for isolation**: use Task tool for large refactors — keeps main session clean
- **Session hygiene**: keep sessions focused; start fresh rather than pushing through context limits
- **Commit often**: detailed commit messages serve as memory between sessions
- **End of session**: run `codebase handoff` before closing to save state for the next agent
- **Model selection**: Opus for architecture/security decisions; Sonnet for implementation/iteration
- **Rewind mistakes**: ESC ESC after a bad edit — reverts and lets you try a different approach

### Where to Find More
- Vibekit loop: see `.claude/commands/` for /simulate, /build, /launch
- MCP tools: call `list_commands` or `list_skills` via MCP server
- Browser automation: see `~/.claude/skills/simulate/SKILL.md`
<!-- codebase:end -->
