## test: add tests for zero-deadline and no-deadline bounties

Closes #503

---

### What this PR does

Adds a dedicated test module `test_deadline_variants` that exercises the three meaningful deadline configurations the escrow contract can receive:

| Configuration    | Value                 | Refund behaviour                                                 |
| ---------------- | --------------------- | ---------------------------------------------------------------- |
| Zero deadline    | `deadline = 0`        | Immediately refundable — `now < 0` is always false for `u64`     |
| Future timestamp | `deadline = now + n`  | Blocked until the deadline elapses; admin approval bypasses it   |
| No deadline      | `deadline = u64::MAX` | Permanently blocked without admin approval; release always works |

---

### Changes

**New file**

- `contracts/bounty_escrow/contracts/escrow/src/test_deadline_variants.rs` — 17 focused tests grouped into three sections and two cross-configuration comparisons.

**Modified file**

- `contracts/bounty_escrow/contracts/escrow/src/lib.rs` — registered the new module with `#[cfg(test)] mod test_deadline_variants;`.

---

### Test breakdown

**Zero deadline (`deadline = 0`)**

- `test_zero_deadline_stored_correctly` — verifies the value is persisted as-is
- `test_zero_deadline_refund_succeeds_immediately` — refund without any clock advance
- `test_zero_deadline_refund_succeeds_after_time_advance` — refund remains eligible after clock advances
- `test_zero_deadline_release_succeeds` — admin release path is unaffected

**Future timestamp (`deadline = now + n`)**

- `test_future_deadline_stored_correctly`
- `test_future_deadline_refund_blocked_before_expiry` — expects `DeadlineNotPassed`
- `test_future_deadline_refund_succeeds_after_expiry` — clock advanced past deadline
- `test_future_deadline_early_refund_with_admin_approval` — `approve_refund` bypasses the check
- `test_future_deadline_release_unaffected_by_deadline` — release works regardless of deadline

**No deadline (`deadline = u64::MAX`)**

- `test_no_deadline_stored_correctly`
- `test_no_deadline_refund_blocked_without_approval` — expects `DeadlineNotPassed`
- `test_no_deadline_refund_blocked_even_after_large_time_advance` — 100-year advance, still blocked
- `test_no_deadline_refund_succeeds_with_admin_approval` — full refund via approval
- `test_no_deadline_partial_refund_with_admin_approval` — partial refund, status → `PartiallyRefunded`
- `test_no_deadline_release_succeeds` — release always available

**Cross-configuration comparisons**

- `test_deadline_zero_vs_future_refund_eligibility`
- `test_deadline_future_vs_no_deadline_after_expiry`

---

### Proof of passing build

<!-- Attach a screenshot of the terminal output here showing all 17 tests passing.
     See the instructions below for how to capture it. -->

![Test results](<!-- replace this comment with the image you attach -->)

---

### How to get the attachment

1. Open a terminal in the `contracts/bounty_escrow` directory:
   ```
   cd contracts/bounty_escrow
   ```
2. Run only the new tests so the output is concise:
   ```
   cargo test test_deadline_variants -- --nocapture
   ```
3. You should see output ending with:
   ```
   test result: ok. 17 passed; 0 failed; 0 ignored
   ```
4. Take a screenshot of the terminal showing that final line (or the full test list).  
   On Linux you can use `gnome-screenshot`, `scrot`, or the built-in screenshot shortcut (`PrtSc`/`Shift+PrtSc`).
5. Upload the screenshot when creating the PR on GitHub (drag-and-drop into the description text box), then replace the placeholder comment above with the resulting image URL.

---

### Deadline semantics (documented for reviewers)

The `deadline` field in `Escrow` is a plain `u64` UNIX timestamp. The refund gate is:

```rust
if now < escrow.deadline && approval.is_none() {
    return Err(Error::DeadlineNotPassed);
}
```

- **`0`** — The inequality `now < 0` can never be true for `u64`, so the gate is always open. This makes `deadline = 0` equivalent to "already expired / no waiting required".
- **`now + n`** — Standard timed bounty. Refund is gated until the ledger timestamp exceeds the stored deadline.
- **`u64::MAX`** — The inequality is always true for any realistic clock value, so the gate is permanently closed unless an admin approval record exists. Use this to represent a bounty with no natural expiry.
