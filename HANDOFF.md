# Session 25 Handoff

## Discovery: Control Flow Broken for Ctor Param Lowering

**Status**: Fresh ctor vars still not being created. Root cause identified but not yet fixed.

### The Problem

Test still fails with:
```
expected `identity::F<_>`, found `Option<i32>`
```

Fresh args contain `Known(CtorDef { def_id: identity::F, ... })` instead of `Var(?c)`.

### Root Cause Analysis

The `Known(CtorDef)` with param's own def_id is created by error recovery in `GenericParamDef::to_error()` (rustc_middle/src/ty/generics.rs:108). This happens when `extend_with_error()` is called to fill missing ctor args in fresh args.

**Why are ctor args missing?**
- `instantiate_binder_with_fresh_vars` creates fresh vars for late-bound variables
- If ctor params are early-bound (stored as `Param(F)` not `Bound(BoundCtor)`), no fresh ctor var is created
- When extending args, ctor param slot is missing, triggering error recovery

**Why are ctor params early-bound?**
- `try_lower_ctor_param_use` is supposed to create `Ctor(Bound(BoundCtor), arg_ty)` types during signature lowering
- But this function is **never being called** (verified: no output when added eprintln)
- Either the function doesn't detect it should handle ctor params, or parameter types aren't going through that code path

### Control Flow Investigation

Traced function signature lowering:
1. `fn_sig` query (collect.rs:970) → calls `lower_fn_sig_recovering_infer_ret_ty`
2. → calls `icx.lowerer().lower_fn_ty(...)`
3. → calls `lower_fn_sig(...)` (line 3544)
4. → impl in collect.rs:528 iterates over `decl.inputs` and calls `self.lowerer().lower_ty(a)` for each

**Added eprintln to confirm:**
- `HirTyLowerer::lower_ty` (line 3039) — **NOT CALLED** (0 times)
- `lower_resolved_ty_path` (line 2175) — **NOT CALLED**  
- `Res::Def(DefKind::TyParam, ...)` path — **NOT CALLED**
- `try_lower_ctor_param_use` — **NOT CALLED**

**Conclusion**: Signature parameter types are **not** being lowered through the normal `lower_ty` pipeline during signature collection.

### Hypothesis

Either:
1. Signature types are NOT lowered at collection time (just stored as HIR), lowering happens later during type-checking
2. There's a different lowering path for function signatures that bypasses `lower_ty`
3. The `ItemCtxt::lower_ty` (collect.rs:246) that delegates to `self.lowerer().lower_ty()` is somehow not actually calling the trait method

### Next Steps

1. **Verify when signature types are lowered**: Add eprintln to `collect.rs:552` (where `self.lowerer().lower_ty(a)` is called) to confirm if signature lowering is even being executed
2. **Check type-checking phase**: Signatures might be lowered during type-checking (`rustc_hir_typeck`), not collection
3. **Trace the actual impl**: Determine which `lower_ty` method is actually being called by the `self.lowerer().lower_ty()` delegatio chain

### Files Modified (Session 25)

- `compiler/rustc_hir_analysis/src/hir_ty_lowering/mod.rs`: 
  - Fixed `try_lower_ctor_param_use` signature to accept `hir_id` parameter
  - Changed from `last_seg.hir_id` back to correct HIR ID source
  - Added diagnostic output (now removed)

### Key Insight

The Session 24 infrastructure (bound ctor instantiation in `instantiate_binder_with_fresh_vars`) is correct. The blocker is **not** in fresh var creation — it's in whether ctor params ever reach the code that marks them as bound in the first place.

If `try_lower_ctor_param_use` is never called, ctor params are never converted to `Ctor` types with bound/param ctor args. They stay as regular `Param(F)` types. Then when fresh args are created, there's nothing to instantiate, and error recovery creates dummy `Known(CtorDef)` values.

The fix is to ensure ctor param application syntax (`F<A>`) goes through `try_lower_ctor_param_use` during signature lowering.
