# Contributing

Thanks for helping. Titi is AGPL-3.0: contributions are accepted under the same licence.

## Branches

- `dev` — integration. Open PRs against `dev`.
- `main` — production. Every push deploys the relay (Cloud Run) and the web app
  (<https://titi.dragoscatalin.ro>) after CI passes. Tags `vX.Y.Z` on `main` publish
  Android (Play internal) and desktop builds to GitHub Releases.

## Setup (Windows, macOS or Linux)

```sh
pnpm install            # also installs the git hooks (lefthook)
pnpm gates              # everything CI runs, locally, in parallel lanes
```

Rust ≥ 1.85, Node 24, pnpm 10, JDK 22 and the Android SDK/NDK (for `android/`).

## Fast loops

| What | Command |
| --- | --- |
| relay unit tests (+coverage) | `pnpm --filter @titi/backend exec vitest --coverage` |
| shared UI logic tests | `pnpm --filter @titi/app-ui exec vitest` |
| Rust core | `cargo nextest run -p titi-core --manifest-path core/Cargo.toml` |
| E2E, only the flow you touch | `pwsh scripts/e2e.ps1 -Grep chat` (starts only relay + web, reuses them) |
| E2E with hot reload | `pwsh scripts/e2e.ps1 -Dev` |
| E2E against production | `pwsh scripts/e2e.ps1 -BaseUrl https://titi.dragoscatalin.ro -Smoke` |

Failed E2E tests keep a trace, video and every browser console error:
`pnpm --filter @titi/e2e exec playwright show-report`.

## Hooks and gates

- **pre-commit** (seconds): gitleaks on staged changes, oxlint, rustfmt, no large/forbidden files.
- **commit-msg**: Conventional Commits (`feat(web): …`, `fix(core): …`).
- **pre-push**: JS lane always; the native lane only when `core/`, `android/` or the desktop crate changed.
- **CI**: the same gates on a clean checkout, plus E2E, coverage thresholds, cargo-deny,
  CodeQL, dependency review and a secret scan of the full history.

Coverage thresholds only go up. If a change lowers one, explain why in the commit message.

## Performance and size budgets

`scripts/size-budgets.ps1` fails when a shipped artifact (APKs, wasm, desktop exe/installer,
Tizen bundle) grows past its budget. `scripts/gates.ps1` fails when the warm gate run exceeds
its time budget. Raise a budget only with a measured reason.
