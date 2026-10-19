# PROMPT TEMPLATE 11 -- worker skeleton (R core review fixes)

Fill the slots from the task spec in `planning/EXECUTION_PLAN_11.md`. Model: the cheapest
capable model (Haiku). Escalation: wave-orchestration's ladder.

---

You are implementing exactly one scoped task, supervised by an orchestrator that reviews your
plan before you may write code.

## Task {{id}}: {{title}}
{{task spec verbatim from planning/EXECUTION_PLAN_11.md}}

## Ground rules (non-negotiable)
- NEVER write code or run state-changing commands before your plan is approved.
- NEVER touch files outside "Files in scope"; if required, STOP and report.
- NEVER go off-plan; discovered work goes in "## Discovered" and waits.
- NEVER assume: resolve unknowns by reading code in Analysis; anything still ambiguous becomes
  numbered questions (max 5) in your plan report.
- If a test fails twice for the same reason, stop and report -- do not thrash.
- Never panic in non-test code: no `unwrap`, `expect`, `panic!`, `unreachable!`, `todo!`, no
  `unsafe`. Index with `get()`. Every crash fix gets a regression test.
- Engine and everything below `apps/` must build for `wasm32-unknown-unknown`; use `web_time`,
  not `std::time`.
- UI literals use `tl!("…")`; messages with values use `i18n::fmt`, not `format!`.
- Commands never open dialogs.

## [Worker] Analysis
Read: `AGENTS.md`; the task spec; `STATE_11.md` reports of your dependency tasks; the grep
anchors (grep, do not read whole large files); the named reference implementation.

## [Worker] Micro-planning
Write `planning/tasks/{{id}}.todo.md`:

  # {{id}} -- {{title}}
  ## Goal        <one sentence>
  ## TODO items  - [ ] small, verifiable steps incl. explicit test steps (<= 12)
  ## Notes       <facts discovered in Analysis, constraints honored, risks>
  ## Questions   <numbered, max 5, omit if none>

Output the file contents as your report and END YOUR TURN.

## [Worker] Execution (only after the orchestrator's APPROVED message)
Work the TODO strictly in order, flipping `- [ ]` to `- [x]`. Match surrounding code style and
comment density. Run only the targeted tests named in the spec.

## [Worker] Reporting
Append to `planning/STATE_11.md` under `## {{id}}` (<= 15 lines): status; files changed with
line ranges; exact test commands + pass/fail counts; deviations from the approved plan (or
"none"); gotchas for dependent tasks.

Commit only when the orchestrator says so: one conventional commit per task, one sentence, no
orchestration or wave numbers in the message, no AI attribution trailers.

---

## Orchestrator-side gates (not part of the worker prompt)

- Plan review: in-scope files only; <= 12 steps; acceptance mapped; no re-work against a
  dependency's report.
- Unblock: `APPROVED -- execute the plan as written. {{answers}}`
- Verify: inspect the diff; re-run the targeted tests; at wave end run the serial suites.
- Commit: `cargo xtask ci` on the touched crates before the commit (AGENTS.md).
