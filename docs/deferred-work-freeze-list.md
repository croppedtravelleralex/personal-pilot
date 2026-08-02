# Deferred Work Freeze List (2026-04-02)

## Frozen for the current stage

### 1. providerRegion implementation
Status: **frozen**
Reason: entry threshold not met yet.

### 2. selection ranking redesign around version semantics
Status: **frozen**
Reason: no evidence yet that current behavior creates ranking mistakes worth redesigning.

### 3. broad trust-score semantics rewrite
Status: **frozen**
Reason: current phase goal was refresh-scope stabilization, not semantics expansion.

### 4. broad explainability rewrite
Status: **frozen**
Reason: explain visibility fields are sufficient for the current stage.

## Reopen conditions

- providerRegion: only after entry-condition threshold is met
- selection redesign: only after evidence-backed ranking correctness issues
- trust semantics rewrite: only after a new mainline explicitly adopts it
- explain rewrite: only after readability/usage evidence shows current output is inadequate
