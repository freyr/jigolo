---
date: 2026-03-03
topic: refactoring-review
---

# Refactoring Review — Issues #25-#32

## What We Evaluated

Eight refactoring issues proposed after a code structure analysis. Each claimed duplication, excessive method length, or God-struct concerns in the TUI modules.

## Outcome

**1 issue kept open (#25), 7 closed.** Most proposals failed the "is the abstraction actually simpler than the duplication?" test.

## Key Decisions

- **#25 TextInput struct: DO IT** — 3 byte-for-byte identical text input handlers across files.rs, compose.rs, library.rs. Natural abstraction boundary, also removes 2 fields from App.
- **Method length issues (#26, #31): CLOSED** — 128 and 75 lines of straight-line rendering code. Reads top-to-bottom. Splitting would create multi-parameter functions called from one place and add borrow-checker friction.
- **Scrollbar helper (#27): CLOSED** — 4 occurrences of a 3-line idiomatic ratatui pattern. Not worth the indirection for ~8 lines saved.
- **Library path helper (#28): CLOSED** — 6 occurrences but None branches differ (some reset state, some don't). Closure-based helper would be leaky. Better fix: cache path on App at init.
- **CursorState (#29): CLOSED** — Superficial similarity. SettingsState has fundamentally different logic (skips collapsed lines). Only 2 truly identical occurrences.
- **App field reduction (#30): CLOSED** — 18 fields is not egregious for 4 screens. Flat struct avoids borrow-checker friction. High effort, low payoff.
- **Tilde expansion (#32): CLOSED** — Single occurrence. Premature abstraction.

## Principles Applied

1. **Don't split straight-line rendering code.** If a method reads top-to-bottom without complex branching, length alone is not a problem.
2. **Verify duplication is actually identical.** Similar-looking code often has meaningful differences (CursorState, library_path dispatch).
3. **Consider borrow-checker cost.** Rust's ownership model makes some abstractions (screen-specific sub-structs on App) more expensive than in GC languages.
4. **YAGNI on single occurrences.** Extract when the second use appears, not before.

## Next Steps

Implement #25 (TextInput extraction) as the only surviving refactoring issue.
