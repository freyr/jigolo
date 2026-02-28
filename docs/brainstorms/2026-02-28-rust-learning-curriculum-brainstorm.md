# Rust Learning Curriculum — Brainstorm

**Date:** 2026-02-28
**Status:** Decided

## What We're Building

An interactive Rust learning curriculum that uses this codebase (context-manager) as a living textbook. Invoked via a `/rust-learn` slash command in Claude Code. Organized as a **concept spiral** — 10 thematic clusters that pull examples from across all modules, bridge each concept to PHP equivalents, and test comprehension through read-and-explain quizzes.

### Learner Profile

- **Background:** PHP / Laravel / Composer, enterprise SaaS, interpreted languages
- **Gap:** No compiled-language experience (ownership, stack/heap, compilation model are new)
- **Goal:** Build CLI/systems tools in Rust
- **Style:** Quiz & explain — read code, predict behavior, explain reasoning
- **Pace:** Flexible — supports both 15-minute quick bites and 60-90 minute deep dives

## Why This Approach

**Concept Spiral** was chosen over Module Walk and PHP-First Reverse Map because:

1. **Focuses time on what's genuinely new.** A PHP developer already understands structs-as-classes and iterators. The spiral lets us speed through familiar ground and spend real time on ownership, borrowing, and lifetimes — concepts with zero PHP equivalent.
2. **Cross-module examples build richer mental models.** Seeing `&self` in model.rs, discovery.rs, AND app.rs teaches the pattern, not just the instance.
3. **Supports flexible pacing.** Each cluster works as a 30-minute quick bite (core concept + 3 quiz questions) or a 90-minute deep dive (full concept + all examples + edge cases + 10 questions).

## Key Decisions

### 1. Invocation: `/rust-learn` Slash Command

Three subcommands:
- `/rust-learn` — Start or continue the next lesson in sequence
- `/rust-learn quiz` — Quiz mode on the current or most recent cluster
- `/rust-learn status` — Show progress dashboard (clusters completed, current position)

The skill reads a progress file at `docs/journal/rust-learning-progress.toml` to track state.

### 2. The 10 Concept Clusters (in order)

Order rationale: each cluster builds on the previous. Clusters 1-2 establish the "this is not PHP" foundation. Cluster 3 (ownership) is the hardest and most important — placed early so every subsequent cluster reinforces it. Clusters 4-9 are roughly ordered by how much new-to-PHP content they contain. Cluster 10 (testing) is last because it synthesizes everything.

| # | Cluster | New-to-PHP Level | Key Files |
|---|---------|-------------------|-----------|
| 1 | Compilation Model & Cargo | Medium | `Cargo.toml`, `main.rs` |
| 2 | Types, Structs, Enums | Low (familiar shape, new rules) | `model.rs`, `app.rs` |
| 3 | Ownership, Borrowing & Moves | **Critical** (no PHP equivalent) | All files |
| 4 | Option\<T\> & Result\<T,E\> | Medium (replaces nullable + exceptions) | `discovery.rs`, `library.rs` |
| 5 | Pattern Matching & Match Guards | Medium (PHP match is simpler) | `app.rs`, `discovery.rs` |
| 6 | Iterators & Closures | Low-Medium (array_map analog, but lazy) | `discovery.rs`, `main.rs` |
| 7 | Traits & Derives | Medium (interfaces + annotations) | `model.rs`, `library.rs` |
| 8 | Modules & Visibility | Low (namespaces + PSR-4 analog) | `lib.rs`, `main.rs`, `tui/mod.rs` |
| 9 | Error Handling with anyhow | Medium (try/catch analog, but different) | `library.rs`, `app.rs` |
| 10 | Testing Patterns | Medium (PHPUnit analog, new patterns) | All `#[cfg(test)]` blocks, `tests/cli.rs` |

### 3. Session Structure

Each cluster session follows this flow:

**Quick Bite (15-30 min):**
1. **Bridge** (2 min) — "In PHP you do X. In Rust, the equivalent is..."
2. **Show** (5 min) — 2-3 real code snippets from the codebase with annotations
3. **Quiz** (10 min) — 3-5 read-and-explain questions
4. **Capture** (2 min) — Auto-generate journal entry to `docs/journal/`

**Deep Dive (60-90 min):**
1. **Bridge** (5 min) — Full PHP-to-Rust concept mapping with nuance
2. **Show** (15 min) — 5-8 code snippets, covering edge cases and variations
3. **Explain** (15 min) — "Why does Rust do it this way?" — compiler reasoning, safety guarantees, performance implications
4. **Quiz** (20 min) — 8-12 questions, including "what would the compiler say if..." predictions
5. **Connections** (10 min) — How this cluster connects to previous ones
6. **Capture** (5 min) — Detailed journal entry with quiz results

### 4. Quiz Question Types (Read-and-Explain Only)

- **Predict:** "What does this expression evaluate to?"
- **Explain:** "Why does this function take `&self` instead of `self`?"
- **Compiler:** "Would this code compile? If not, what error would you expect?"
- **Compare:** "How does this differ from the PHP equivalent?"
- **Find:** "Which line in this snippet demonstrates [concept]?"
- **Consequence:** "What would happen at runtime if this `Option` were `None` here?"

### 5. Progress Tracking

A TOML file at `docs/journal/rust-learning-progress.toml`:

```toml
[meta]
started = "2026-02-28"
last_session = "2026-02-28"

[clusters.compilation_model]
status = "not_started"  # not_started | in_progress | completed
sessions = 0
last_quiz_score = ""

[clusters.types_structs_enums]
status = "not_started"
sessions = 0
last_quiz_score = ""

# ... etc for all 10
```

### 6. PHP Analogy Quick Reference (per cluster)

| Rust Concept | PHP Equivalent | Key Difference |
|---|---|---|
| `Cargo.toml` | `composer.json` | Also controls compilation targets, features, edition |
| `cargo build` | No direct equivalent | PHP is interpreted; Rust produces a single binary |
| `struct Foo { ... }` | `class Foo { ... }` | No inheritance, no constructor magic, fields are data-only |
| `enum ExitOutcome { Ok, Error }` | `enum ExitOutcome: int { ... }` backed enums | Rust enums can carry data (tagged unions), PHP enums cannot |
| Ownership / move | `$x = $y` (always copy or refcount) | Values are MOVED by default in Rust, not copied |
| `&self` / `&mut self` | `$this` | Explicit shared vs exclusive access; compiler-enforced |
| `Option<String>` | `?string` | Not a type modifier — it's a real enum you must handle |
| `Result<T, E>` | `try { } catch { }` | Errors are values, not thrown; `?` propagates them |
| `match` | `match ($x) { ... }` | Exhaustive (compiler forces all cases), supports destructuring |
| `.iter().map().filter()` | `array_map()` / `array_filter()` | Lazy evaluation, chaining, zero-cost abstraction |
| `trait Display` | `interface Stringable` | Also used for operator overloading, derive macros, generics bounds |
| `mod` / `pub` / `use` | `namespace` / PSR-4 / `use` | Module tree is explicit in code, not derived from file paths |
| `anyhow::Result` | Exception hierarchy | No stack unwinding; errors flow through return values |
| `#[cfg(test)]` | PHPUnit test classes | Tests live in the same file, conditionally compiled out of production |

### 7. Journal Entry Format

Auto-generated after each session to `docs/journal/cluster-NN-name.md`:

```markdown
# Cluster N: [Name]

**Date:** YYYY-MM-DD
**Mode:** Quick bite / Deep dive
**Quiz score:** X/Y

## Key Concepts
- [Concept 1]: [one-line explanation in your own words]
- [Concept 2]: ...

## PHP Bridge
- [PHP pattern] -> [Rust equivalent]: [key insight]

## Quiz Review
- Q: [question]  A: [your answer]  Correct: [yes/no + correction if needed]

## Open Questions
- [Things still unclear, to revisit]
```

## Open Questions

1. **Should we add a "spaced repetition" mechanism?** Revisiting earlier clusters after N sessions to reinforce retention. Could be as simple as mixing 1-2 review questions from past clusters into each new session.

2. **Should the skill support "concept lookup" mode?** E.g., `/rust-learn explain ownership` for on-demand reference outside the curriculum sequence.

3. **Should quiz results persist in memory (MEMORY.md)?** This would let future sessions adapt difficulty based on past performance, but might clutter memory.

## What's NOT in Scope

- Writing Rust code (learner preference: read-and-explain only)
- External resources or book references (the codebase IS the textbook)
- Covering async/await, threads, unsafe (not used in this codebase)
- Covering generics beyond what appears in the code (`TreeItem<'static, TreeId>`)
