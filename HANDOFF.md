# Session 13 Handoff: Architectural Gap Identified - Phase 3 Blocker

## What Was Done (Session 13)

**Completed: Infrastructure testing and architectural analysis**

The constructor equation solving infrastructure (Phases 1-2) is complete, compiles, and all TypeRelation impls are in place. However, end-to-end testing revealed a fundamental architectural gap that blocks Phase 3.

## The Architectural Gap: Type Representation Problem

### The Issue

When `identity(opt)` is called with `opt: Option<i32>` and the parameter type is `fn<F<_>, A>(x: F<A>)`:

1. **Instantiation:** Generic parameters get fresh variables: `F` → `CtorVid(?0)`, `A` → `TyVar(?A)`
2. **Substitution:** The type `F<A>` is substituted via `ctor_for_param()`:
   - Looks up `GenericArg` at position 0: finds `CtorArg::Var(?0)`
   - Calls `ctor_is_identity(?0)` → returns `true`
   - **Reconstructs type as `Ctor(ParamCtor{index=0}, ?A)`** ← This is the problem
3. **Unification:** Must unify `Ctor(ParamCtor{index=0}, ?A)` against `Adt(Option, [i32])`
4. **Failure:** In `structurally_relate_tys()` at line 525, no arm handles `Ctor` vs `Adt`, so we get:
   ```
   TypeError: sorts mismatch (Ctor vs Adt)
   ```

### Root Cause

**The `CtorVid(?0)` inference variable becomes inaccessible after substitution.**

When we reconstruct with `ParamCtor{index=0}`, we've severed the link to the actual `CtorVid(?0)`. Later, in `TypeRelating::tys`, we only have:
- `ParamCtor{index=0}` (a parameter reference)
- No way to resolve this reference back to `CtorVid(?0)`

The inference variable was created during instantiation, but its location in the type system isn't accessible from the type level.

## Why Current Approaches Don't Work

### Approach A: Handle in `structurally_relate_tys` (lower level)
- **Problem:** `structurally_relate_tys` is in `rustc_type_ir` with no access to `InferCtxt`
- **Can't do:** Resolve `ParamCtor.index` to get the `CtorVid`

### Approach B: Handle in `TypeRelating::tys` (higher level)
- **Problem:** Even with `InferCtxt` access, `ParamCtor` is just `{index: u32, name: Symbol}`
- **Can't do:** Look up which `CtorVid` corresponds to parameter index 0 in the current context
- **Missing:** No mapping from parameter indices to their instantiated `CtorVid`s

### Approach C: Modify `apply_ctor` for inference variables
- **Problem:** `apply_ctor` is called during type substitution (in binder folder), before `InferCtxt` exists
- **Can't do:** Create type variables or record obligations at that point

## The Real Fix: Type Representation Change

**The `Ctor` type node should embed the `CtorArg` directly, not a `ParamCtor` reference.**

Current (broken):
```rust
pub enum TyKind<I: Interner> {
    Ctor(I::ParamCtor, I::Ty),  // ParamCtor is just {index, name}
    ...
}
```

Needed:
```rust
pub enum TyKind<I: Interner> {
    Ctor(I::CtorArg, I::Ty),  // CtorArg is Var(CtorVid) | Known(CtorDef)
    ...
}
```

**Benefits:**
- Inference variables are visible at the type level
- `TypeRelating::tys` can directly access `CtorVid` and call `ctor_args`
- No need for parameter index lookups or hidden mappings
- Unification becomes straightforward

**Cost:**
- Requires changing `TyKind` enum (ripples through entire codebase)
- Affects type printing, encoding, error reporting
- Moderate refactoring (~500-1000 LOC)

## What Remains for Phase 3

### Immediate (1-2 sessions)
1. Change `Ctor` variant to take `CtorArg` instead of `ParamCtor`
2. Update `ctor_for_param()` to work with the new representation
3. Update `mk_ty_ctor()` to work with `CtorArg`
4. Add special case in `TypeRelating::tys` to relate `Ctor` types via `ctor_args`
5. Run full test suite

### Testing
- `tests/ui/type-constructors/examples/effects-system.rs` (currently ignored)
- Full type-constructor test suite for regressions

## Code Locations

| What | File | Lines |
|------|------|-------|
| Problem manifestation | `compiler/rustc_type_ir/src/relate.rs` | 520-525 |
| Architecture decision (reconstruction) | `compiler/rustc_type_ir/src/binder.rs` | 854-855 |
| Decision logic | `compiler/rustc_middle/src/ty/context/impl_interner.rs` | 102-112 |
| Type definition | TyKind enum in rustc_type_ir |
| Solution location | `TypeRelating::tys` would intercept here |

## Session Summary

✅ Phases 1-2 infrastructure complete  
✅ All TypeRelation impls have ctor_args methods  
✅ Constructor unification machinery tested  
⏳ Phase 3: Blocked by `ParamCtor` vs `CtorArg` representation gap  
❌ End-to-end HKT function calls don't work yet

## Key Insight

This isn't a small bug—it's a **design decision that broke down**. Using `ParamCtor` to delay constructor instantiation made sense initially, but it created an information boundary that the type relation machinery can't cross. The solution requires embedding the inference variable directly in the type, which is a bigger refactoring but architecturally clean.

---

## Next Session Prerequisites

- Understand `TyKind` definition and where it's used
- Plan `Ctor(CtorArg)` refactoring carefully to minimize ripple effects
- Consider: does `CtorArg` need additional context (like its "home" context)?
- Test strategy: build incrementally, running type-checker tests after each chunk

