# Session 11 Handoff: Constructor Equation Solving

## What Was Done (Session 10)

**Completed: ctor_is_identity bug fix**
- Changed `ctor_is_identity` in `impl_interner.rs:110` to return `true` for `Var(_)` 
- Rationale: Inference variables are abstract like type parameters; must be preserved during substitution
- Result: No more crashes when `ctor_for_param` encounters `Var(?F)`
- Commit: `38d00d8b378` (WIP: Step 8 Part 1)

**What still doesn't work:** Constructor equations `?F ~ Result` aren't being **solved**.

## The Remaining Problem

**Test case:** `identity(r)` where `r: Result<i32, String>`
- Function signature: `fn identity<F<_>, A>(x: F<A>) -> F<A>`
- After instantiation: fresh variables `?F` and `?A`
- Expected param type: `?F<?A>`
- Actual arg type: `Result<i32, String>`
- Need to unify: `?F<?A> = Ctor(?F, ?A)` with `Result<i32, String>`

**Error:** Type mismatch still reported; `?F` never gets bound to `Result`.

## Root Cause

When `GenericArgKind::Ctor` values are related in `rustc_middle/src/ty/relate.rs`:
```rust
(ty::GenericArgKind::Ctor(a_ctor), ty::GenericArgKind::Ctor(b_ctor)) => {
    if a_ctor == b_ctor { Ok(a) } else { bug!("ctor mismatch") }  // <-- structural equality only
}
```

This treats all ctor arguments as structurally equal or not. **No unification of inference variables happens.**

### Why the Relate impl is limited:

The `Relate` trait in `rustc_type_ir` is the generic relate impl. It doesn't have access to `InferCtxt` where unification tables live. When it encounters:
- `Var(?F)` vs `Known(Result)` → still bugs, no way to call unification

## Solution Architecture

### Phase 1: Add constructor unification to `TypeRelation`

**File:** `compiler/rustc_infer/src/infer/relate/type_relating.rs`

Add handling for `GenericArgKind::Ctor` in the main type relating logic (mirror `TyVar` handling):

```rust
// In the `relate` method or a new handler:
// When relating GenericArgs that contain Ctor args with inference vars,
// delegate to constructor-specific unification.

// Pattern (from TyVar handling, lines 173-183):
// (&ty::Infer(TyVar(a_vid)), _) => infcx.instantiate_ty_var(...)
// (_, &ty::Infer(TyVar(b_vid))) => infcx.instantiate_ty_var(...)

// Analogous for constructors:
// (Ctor(Var(a_vid)), other_ctor) => instantiate_ctor_var(...)
// (other_ctor, Ctor(Var(b_vid))) => instantiate_ctor_var(...)
```

### Phase 2: Implement `instantiate_ctor_var`

**File:** `compiler/rustc_infer/src/infer/relate/generalize.rs` or new method on `InferCtxt`

Parallel to `instantiate_ty_var` (lines 54-217 in generalize.rs):
- Takes `ctor_var_id: CtorVid` and `other_ctor: CtorArg`
- Unifies the variable with the concrete constructor
- Updates the `ctor_unification_table()`

Pseudo-code:
```rust
pub fn instantiate_ctor_var<R: PredicateEmittingRelation<Self>>(
    &self,
    relation: &mut R,
    var_is_expected: bool,
    ctor_vid: CtorVid,
    other_ctor: ty::CtorArg<'tcx>,
) -> RelateResult<'tcx, ty::CtorArg<'tcx>> {
    // 1. If other_ctor is also a Var, call equate() on the unification table
    // 2. If other_ctor is Known(_), unify the var with the ctor
    // 3. Handle universes/generalization as needed
    
    match other_ctor.kind() {
        ty::CtorArgKind::Var(other_vid) => {
            self.inner.borrow_mut().ctor_unification_table().equate(...)?;
            Ok(other_ctor)
        }
        ty::CtorArgKind::Known(ctor_def) => {
            // Assign the var to this concrete ctor
            let _ = self.inner.borrow_mut()
                .ctor_unification_table()
                .unify_var_var(ctor_vid, ...)?;  // or direct assignment
            Ok(other_ctor)
        }
    }
}
```

### Phase 3: Wire into Generic Arg relating

**File:** `compiler/rustc_infer/src/infer/relate/type_relating.rs`

In the `relate` method or as part of generic arg combining:
- When a `GenericArg` with a `Ctor` variant reaches relating logic
- Check if either side is an inference variable
- Call `instantiate_ctor_var` instead of just checking equality

## Key Files to Modify

1. **Primary (must implement):**
   - `compiler/rustc_infer/src/infer/relate/generalize.rs` — add `instantiate_ctor_var` method

2. **Secondary (wire in):**
   - `compiler/rustc_infer/src/infer/relate/type_relating.rs` — handle Ctor inference vars in `relate`/`relate_ty_args`
   - May need to modify how `combine_ty_args` delegates to relating logic for Ctor args

3. **Reference (for guidance):**
   - `compiler/rustc_infer/src/infer/relate/generalize.rs:54-217` — `instantiate_ty_var` (the TyVar analogue)
   - `compiler/rustc_infer/src/infer/unify_key.rs:217-263` — `UnifyKey` impl for `CtorVidKey` (unification table contract)

## Test & Verification

After implementing:
```bash
# Uncomment lines 27-28 in effects-system.rs and verify it passes:
./x test tests/ui/type-constructors/examples/effects-system.rs --stage 1

# Run full type-constructor suite:
./x test tests/ui/type-constructors/ --stage 1
```

## Notes for Next Session

- **Complexity:** This mirrors type variable unification but for a new kind. The `TyVar` code is the template.
- **Blast radius:** Likely ~3-4 match arms across 2-3 files. Minimal.
- **Critical:** The unification table machinery already exists (`ctor_unification_table()`, `CtorVidKey`, `CtorVariableValue`). We're just wiring in the solver.
- **Semantic:** Once `?F ~ Result` is solved, substitution of `Ctor(?F, A)` will become `Result<A, _>` (pending second-arg inference for Result).

---

## State Summary

✅ MVP type constructor syntax works (Steps 1-7)  
✅ Parser recognizes `F<_>` (Step 6)  
✅ Type checker instantiates functions with `F<_>` params (Step 7)  
✅ Substitution avoids crash for inference vars (Session 10 - Part 1)  
⏳ **Constructor equation solving** (Session 11 - Part 2, in progress)  
❌ Inference var unification completed  
❌ Full `identity(r)` call works end-to-end

