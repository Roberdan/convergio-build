# ADR-002: Security Audit & Fixes

**Date:** 2025-07-16
**Status:** Accepted

## Context

Full security audit of convergio-build (self-build daemon crate).

## Checklist Results

| Check | Result | Notes |
|-------|--------|-------|
| SQL injection | ✅ PASS | All queries use parameterized `?N` bindings |
| Path traversal | ✅ PASS | No user-controlled paths in file ops |
| Command injection | ✅ PASS | All `Command::new` use hardcoded args only |
| SSRF | ✅ PASS | No HTTP clients or user-controlled URLs |
| Secret exposure | ✅ PASS | No tokens/credentials in code or logs |
| Race conditions | ⚠️ FIXED | Concurrent builds could race (see below) |
| Unsafe blocks | ✅ PASS | Zero `unsafe` blocks |
| Input validation | ⚠️ FIXED | Unbounded `limit` parameter (see below) |
| Auth/AuthZ bypass | ✅ PASS | Auth handled by SDK middleware layer |

## Findings & Fixes

### 1. Concurrent Build Race Condition (Medium)

**Problem:** Multiple POST `/api/build/self` calls could spawn concurrent
builds writing to the same `target/release/convergio` binary simultaneously,
causing data corruption or partial binaries.

**Fix:** Added `Arc<Mutex<()>>` build lock in `BuildState`. The
`run_build_pipeline` acquires the lock before executing, serializing builds.

### 2. Unbounded History Limit (Low)

**Problem:** `GET /api/build/history?limit=999999999` would attempt to
return all records. While parameterized (no SQL injection), it could cause
excessive memory use.

**Fix:** Clamped `limit` to `1..=100` via `i64::clamp()`.

## Decision

Both fixes applied with zero breaking changes. All 21 tests pass.
