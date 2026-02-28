---
title: "Build /rust-learn slash command skill"
type: feat
date: 2026-02-28
brainstorm: docs/brainstorms/2026-02-28-rust-learning-curriculum-brainstorm.md
---

# Build `/rust-learn` Slash Command Skill

## Overview

Create a single-file Claude Code skill at `~/.claude/skills/rust-learn/SKILL.md` that teaches Rust to a PHP developer using the jigolo codebase as a living textbook. Claude reads the source files directly and teaches dynamically — no static question banks, no separate reference files.

The learner is an experienced PHP/Laravel developer with no compiled-language background, targeting CLI/systems tool development in Rust.

## Design Decisions

| Decision | Choice | Rationale |
|---|---|---|
| Skill location | `~/.claude/skills/rust-learn/SKILL.md` | Follows existing skill convention |
| Deliverable | Single file, ~120-160 lines | All reviewers agreed: Claude generates content dynamically from source files |
| Cluster content | Inline table in SKILL.md | No separate reference files. Claude reads source directly. |
| Quiz generation | Dynamic, not pre-written | Claude generates contextual questions from the actual code each session |
| Progress tracking | Journal-file-as-progress | If `docs/journal/cluster-01-compilation-model.md` exists, cluster 1 is done |
| Subcommands | None. Just `/rust-learn` | Lesson flow includes quiz. Progress reported at session start. |
| Session modes | None. One flexible format | User says "go deeper" or "keep it short" conversationally |
| Completion trigger | Journal entry written = cluster done | Simple, artifact-based, no state machine |

### What Was Cut (and Why)

| Cut | Reason |
|---|---|
| 10 cluster reference files | Claude reads source files directly. Static references rot when code changes. |
| 120 pre-written quiz questions | Claude generates better questions dynamically from the actual code. |
| TOML progress file | Journal file existence IS the progress indicator. No separate state file needed. |
| `/rust-learn quiz` subcommand | The lesson flow already includes quizzes. |
| `/rust-learn status` subcommand | Progress is reported inline at the start of each `/rust-learn` invocation. |
| Quick Bite vs Deep Dive | One flexible session. User controls depth conversationally. |
| Codebase validation check | If source files are missing, Claude notices when it tries to read them. |

## Implementation

### Single Phase: Create SKILL.md

Create `~/.claude/skills/rust-learn/SKILL.md` containing:

**1. Frontmatter**

```yaml
---
name: rust-learn
description: Interactive Rust learning curriculum using the jigolo codebase.
  Teaches Rust to PHP developers through read-and-explain quizzes with PHP analogies.
  Invoke /rust-learn to start or continue the next lesson.
---
```

**2. Learner Profile**

- Experienced PHP/Laravel developer, enterprise SaaS background
- No compiled-language experience
- Goal: build CLI/systems tools in Rust
- Learning style: read code, explain reasoning, predict behavior (no code writing)

**3. Curriculum Outline (inline table)**

The 10 concept clusters, in order. Each row: number, name, key concepts (3-5 bullets), PHP analogy (1 line), source files to read.

| # | Cluster | Key Concepts | PHP Bridge | Source Files |
|---|---|---|---|---|
| 1 | Compilation Model & Cargo | Binary compilation, Cargo.toml, cargo commands, target dir, editions | `composer.json` / `vendor/` | `Cargo.toml`, `src/main.rs` |
| 2 | Types, Structs, Enums | Struct definition, impl blocks, data-free enums, derive macros, primitive types | PHP classes, backed enums | `src/model.rs`, `src/tui/app.rs` (Pane, Mode enums) |
| 3 | Ownership, Borrowing & Moves | Stack vs heap, move semantics, `&self`/`&mut self`/`self`, `'static` lifetime, no GC | No PHP equivalent — the big shift | All files, esp. `src/discovery.rs` (`into_path`), `src/tui/app.rs` (`TreeItem<'static, TreeId>`) |
| 4 | Option\<T\> & Result\<T,E\> | `Option` as enum, `.ok()`, `.then_some()`, `Result` with `?`, `unwrap_or` | `?string` / nullable, try-catch | `src/discovery.rs`, `src/library.rs` |
| 5 | Pattern Matching | Exhaustive match, match guards, OR patterns, if-let chains | PHP `match` expression (simpler) | `src/tui/app.rs` (key handling), `src/discovery.rs` |
| 6 | Iterators & Closures | `.iter()`, `.map()`, `.filter()`, `.collect()`, lazy evaluation, `filter_entry` vs `filter` | `array_map`/`array_filter` | `src/discovery.rs` (the full iterator chain), `src/main.rs` |
| 7 | Traits & Derives | `Display`, `Debug`, `Clone`, `Default`, `Serialize`/`Deserialize`, derive macros | Interfaces + PHP attributes | `src/model.rs` (Display impl), `src/library.rs` (serde) |
| 8 | Modules & Visibility | `mod`, `pub`, `use crate::`, re-exports, `lib.rs` pattern | Namespaces + PSR-4 autoloading | `src/lib.rs`, `src/main.rs`, `src/tui/mod.rs` |
| 9 | Error Handling with anyhow | `anyhow::Result`, `.with_context()`, `anyhow!()`, `ErrorKind` matching | try-catch + exception hierarchy | `src/library.rs`, `src/tui/app.rs` |
| 10 | Testing Patterns | `#[cfg(test)]`, `use super::*`, tempfile, assert_cmd, TestBackend, buffer inspection | PHPUnit test classes | All `#[cfg(test)]` blocks, `tests/cli.rs` |

**4. Session Flow Instructions**

Tell Claude to do this on each `/rust-learn` invocation:

1. **Check progress:** List `docs/journal/cluster-*.md` files using Glob. Count completed clusters. Report: "You have completed N of 10 clusters. Next: [cluster name]."
2. **If all 10 done:** Congratulate. Offer to quiz on any cluster by name.
3. **Read source files** listed for the next cluster.
4. **PHP Bridge** (~2 min): "In PHP you do X. In Rust, the equivalent concept is Y. The key difference is Z."
5. **Code Walkthrough** (~5-10 min): Show 3-5 real snippets from the source files. Annotate what each line does. Highlight the cluster's key concepts in context.
6. **Quiz** (~10 min): Generate 5 questions dynamically from the code just discussed. Mix three types:
   - **Predict:** "What does this expression evaluate to?"
   - **Explain:** "Why does this function take `&self` instead of `self`?"
   - **Compiler:** "Would this compile? What error would you expect?"
7. **Capture:** After the quiz, write a journal entry to `docs/journal/cluster-NN-name.md` (e.g., `cluster-03-ownership-borrowing.md`).

**5. Quiz Calibration Examples**

Include 2-3 example questions to set the difficulty level, not a full bank. Example:

> **Predict:** In `discovery.rs`, the iterator chain calls `.into_path()` on a `DirEntry`. What happens to the `DirEntry` after this call? Could you use `entry` again on the next line?
>
> **Explain:** In `model.rs`, `SourceRoot` derives `Clone` but `ExitOutcome` does not. Why might the author have made this choice?
>
> **Compiler:** If you removed the `&` from `fn display(&self, ...)` in the `Display` impl, what would the compiler tell you?

**6. Journal Entry Template**

```markdown
# Cluster N: [Name]

**Date:** YYYY-MM-DD
**Quiz score:** X/5

## Key Concepts
- [Concept]: [one-line explanation in learner's own words from quiz answers]

## PHP Bridge
- [PHP pattern] -> [Rust equivalent]: [key insight]

## Quiz Review
- Q: [question]  A: [learner's answer]  [correct/incorrect + note]

## Open Questions
- [Anything still unclear, to revisit later]
```

**7. Tone and Approach**

- Treat the learner as a senior developer learning a new paradigm, not a beginner
- Use PHP analogies as a bridge, then let go — don't force every concept through a PHP lens
- Focus extra time on cluster 3 (ownership) — this is the paradigm shift
- Be honest about what Rust makes harder and what it makes better
- Quiz answers are AI-evaluated conversationally — explain why an answer is right or wrong

## File Inventory

| Path | Type | Purpose |
|---|---|---|
| `~/.claude/skills/rust-learn/SKILL.md` | Create | The entire skill (single file) |
| `docs/journal/cluster-NN-name.md` | Auto-created by skill | One per completed cluster |

## Acceptance Criteria

- [x] `/rust-learn` reports progress and starts the next cluster on each invocation
- [x] Skill reads actual source files to generate teaching content (no static references)
- [x] Each session includes a PHP bridge, code walkthrough, and 5-question quiz
- [x] Quiz questions are generated dynamically from the code, not pre-written
- [x] Capture step writes a journal entry to `docs/journal/`
- [x] Progress is derived from existing journal files (no separate state file)
- [x] When all 10 clusters are done, the skill congratulates and offers review

## Smoke Test

After creating SKILL.md, invoke `/rust-learn` once and verify:
1. It detects zero completed clusters
2. It reads source files for cluster 1
3. It delivers a PHP bridge, code walkthrough, and quiz
4. It writes `docs/journal/cluster-01-compilation-model.md`
5. A second invocation detects cluster 1 as complete and moves to cluster 2

## References

- Brainstorm: `docs/brainstorms/2026-02-28-rust-learning-curriculum-brainstorm.md`
- Skill convention: `~/.claude/skills/context-doctor/SKILL.md`
- Existing journal: `docs/journal/step-01-hello-world.md`
- Concept inventory: `docs/journal/rust-concepts.md`
