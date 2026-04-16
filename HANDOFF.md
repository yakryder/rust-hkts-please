# Session 14 Handoff: Type Representation Refactor Complete - Design Question Identified

## What Was Done (Session 14)

**Completed: Core type representation change from `ParamCtor` to `CtorArg`**

### The Transformation

Changed `TyKind::Ctor` from:
```rust
Ctor(I::ParamCtor, I::Ty)  // Just a parameter reference
```

To:
```rust
Ctor(I::CtorArg, I::Ty)  // The actual argument: parameter, inference var, or concrete
```

Where `CtorArgKind` is now:
```rust
pub enum CtorArgKind<'tcx> {
    Param(ParamCtor),        // Uninstantiated parameter (e.g., F in fn<F<_>>)
    Known(CtorDef<'tcx>),    // Concrete constructor (e.g., Option, Result<i32, _>)
    Var(ty::CtorVid),        // Inference variable (e.g., ?0ctor)
}
```

### What Changed

**Files modified:**
- `compiler/rustc_type_ir/src/ty_kind.rs` - Changed `Ctor` variant signature
- `compiler/rustc_type_ir/src/inherent.rs` - Updated `new_ctor` trait method
- `compiler/rustc_type_ir/src/interner.rs` - Updated `mk_ty_ctor` signature, added `ctor_arg_as_param` method
- `compiler/rustc_type_ir/src/binder.rs` - Rewrote `ctor_for_param` to handle `Param` lookup
- `compiler/rustc_middle/src/ty/sty.rs` - Updated `new_ctor` impl, added `Param` variant
- `compiler/rustc_middle/src/ty/context/impl_interner.rs` - Implemented new interner methods
- `compiler/rustc_hir_analysis/src/hir_ty_lowering/mod.rs` - Create `Param` variant during lowering
- `compiler/rustc_hir_analysis/src/variance/constraints.rs` - Handle `Param` in variance analysis
- `compiler/rustc_middle/src/ty/print/pretty.rs` - Pretty-print all three variants
- `compiler/rustc_middle/src/ty/generic_args.rs` - Handle `Param` in generic arg lifting
- `compiler/rustc_infer/src/infer/relate/generalize.rs` - Handle `Param` in unification
- `compiler/rustc_infer/src/infer/relate/type_relating.rs` - Handle `Param` in type relation

**Compilation status:** rustc_middle and rustc_infer both build successfully.

### Architecture Achieved

**Inference variables are now visible at the type level.** When you have `Ctor(Var(?0ctor), i32)`, the `?0ctor` is directly accessible and can be unified. No more information boundary.

### The Design Question: Should `Param` Reach Unification?

A critical decision point emerged: what should happen if an uninstantiated `Param` reaches the unification/relating machinery?

**Current implementation:** `bug!` macros in two places:
1. `instantiate_ctor_var()` - treats `Param` as a compiler internal error
2. `relate_ctor_args()` - treats `Param` in comparison as an ICE

**The tension:**

| Perspective | Position | Reasoning |
|---|---|---|
| **Conservative (Niko)** | `bug!` is right | Params should be instantiated before unification. Fail loudly to catch real bugs. |
| **Pragmatist (Error Recovery)** | Handle gracefully | Real error paths hit weird states. Better to fail on the *real* error later than ICE now. |
| **Architect (Felix)** | Question design | Why do we have `Param` in the type at all? This suggests a scoping/lifetime issue. |
| **Type Theorist (Ralf)** | `bug!` is justified | Unifying uninstantiated parameters is semantically undefined. |
| **Reliability Engineer (Carol)** | `bug!` + prevent | Use bug, but add validation early in the pipeline to prevent this case. |

**Consensus:** 3/5 lean toward bug with early prevention; 2/5 lean toward graceful handling.

## What Remains for Phase 3

### Immediate (this session or next)

1. ✅ **Type representation change complete** - `CtorArg` embedded in `Ctor`
2. ✅ **Substitution updated** - `ctor_for_param` looks up `Param` and converts to actual `CtorArg`
3. ✅ **Core compilation clean** - type_ir, middle, infer all build
4. **DECISION NEEDED** - Resolve the `Param` handling question:
   - Option A: Keep `bug!` and add early validation to prevent uninstantiated params from reaching unification
   - Option B: Replace `bug!` with graceful handling (treat as incomparable or unknown)
   - Option C: Investigate whether `Param` shouldn't exist at this level at all (design rethink)

5. **Finish rustc_hir_analysis compilation** - likely 2-5 more exhaustiveness errors to fix
6. **Run full type-constructor test suite** - ensure end-to-end HKT type checking works
7. **Add `Ctor` arm to `TypeRelating::tys`** - implement actual unification for `Ctor(Var, _)` types

### Testing

- `tests/ui/type-constructors/examples/effects-system.rs` (currently ignored)
- Full type-constructor test suite
- Particularly: `identity(opt)` should now work end-to-end

### Open Questions

1. **`Param` semantics**: Should uninstantiated parameters ever reach unification? How confident are we?
2. **Error messages**: If we hit the `bug!` cases, what is the user actually doing wrong?
3. **Canonical forms**: Do canonical queries preserve or strip `Param`?
4. **Inference order**: When instantiation happens, are we creating the right fresh variables at the right time?

## Code Locations (Key References)

| What | File | Lines |
|------|------|-------|
| Type definition | `compiler/rustc_type_ir/src/ty_kind.rs` | 233 |
| `CtorArgKind` enum | `compiler/rustc_middle/src/ty/sty.rs` | 343-348 |
| Substitution logic | `compiler/rustc_type_ir/src/binder.rs` | 836-860 |
| Type lowering | `compiler/rustc_hir_analysis/src/hir_ty_lowering/mod.rs` | 2360-2363 |
| Unification `bug!` | `compiler/rustc_infer/src/infer/relate/generalize.rs` | 243 |
| Relating `bug!` | `compiler/rustc_infer/src/infer/relate/type_relating.rs` | 272 |

## Session Summary

✅ Phases 1-2 infrastructure complete  
✅ Type representation refactored - `CtorArg` embedded in `Ctor`  
✅ `Param` variant added to handle uninstantiated parameters  
✅ Substitution machinery updated to convert `Param` → actual `CtorArg`  
✅ Core compiler crates (type_ir, middle, infer) compile  
⏳ Phase 3 unblocked but requires decision on `Param` handling  
⏳ rustc_hir_analysis compilation in progress (minor exhaustiveness fixes)  
❌ End-to-end HKT function calls not yet tested

## Key Insight

**The representation change was the right move.** Embedding `CtorArg` directly makes inference variables visible and accessible to the type relation machinery. The system is now architecturally sound—we just need to decide how to handle the edge case of uninstantiated parameters in the unification layer.

## Next Session Prerequisites

- Decide on `Param` handling approach (bug vs graceful) - this will guide the remaining work
- Finish rustc_hir_analysis compilation by handling remaining exhaustiveness errors
- Test end-to-end: run the effects-system example and verify unification works
- Consider: does `Param` need additional validation earlier in the pipeline?

---
