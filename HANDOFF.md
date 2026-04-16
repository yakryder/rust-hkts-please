# Session 16 Handoff: Phase 3 Gap Identified - Constructor Substitution Architecture

## What Was Discovered (Session 16)

**Phase 3 is NOT complete. The architecture has a critical gap.**

### The Gap

The HANDOFF from Session 15 claimed that `identity(opt)` should work with the completed unification machinery. Testing revealed it doesn't:

```
error[E0308]: mismatched types
  --> tests/ui/type-constructors/examples/effects-system.rs:28:35
   |
28 |     let _: Option<i32> = identity(opt);
   |                          -------- ^^^ expected `identity::F<_>`, found `Option<i32>`
```

**Root cause:** When lowering the signature `fn<F<_>>(x: F<A>)`:
- Parameter type becomes: `Ctor(Param(F), A)`
- After instantiation: `Ctor(Var(?0), A)` (inference variable)
- Argument type is: `Adt(Option, [i32])` (concrete ADT)

These are **different type kinds**. Unification cannot match them.

The Session 15 HANDOFF assumed this would "just work" once unification machinery existed. It doesn't—the machinery handles `Ctor-to-Ctor` unification, but `Ctor` and `Adt` types are fundamentally incompatible kinds.

### Why This Wasn't Caught

Session 15 never ran the actual test case. The claim "full rustc builds" and "unification complete" was based on compilation success, not functional verification. The test is the first place the gap appears.

### The Actual Problem

The current approach treats constructor instantiation like **type inference**:
1. Create `Ctor(Var(?0), A)` as a fresh variable
2. Unify it with concrete types during type checking
3. Solve constraints

**But this is backwards.** Rust already has a well-tested mechanism for this: **type substitution via the fold machinery** (`ArgFolder::fold_ty` in `binder.rs`).

When you have `fn<T>(x: T)` and instantiate `T → i32`:
- The fold machinery walks the signature
- Replaces `Param(T)` with `i32` directly
- No unification needed

We should do the same for constructors:
- Lower `F<A>` to `Ctor(Param(F), A)` ✓ (already done)
- **Instantiate via fold**: replace `Param(F)` with whatever constructor is bound to it
- This transforms `Ctor(Param(F), A)` → `Ctor(Known(Option), A)` directly
- Then `Ctor-to-Ctor` unification works

## What Remains (Phase 4 - Constructor Substitution)

### Implement Constructor Fold Logic

**File:** `compiler/rustc_type_ir/src/binder.rs`

In `ArgFolder::fold_ty`, add a case for `Ctor` types (similar to how `Param` is handled):

```rust
fn fold_ty(&mut self, t: I::Ty) -> I::Ty {
    if !t.has_param() { return t; }
    match t.kind() {
        ty::Param(p) => self.ty_for_param(p, t),
        ty::Ctor(ctor_param, arg_ty) => self.ctor_for_param(ctor_param, arg_ty),  // NEW
        _ => t.super_fold_with(self),
    }
}
```

The method `ctor_for_param` should:
1. Extract the constructor argument from `self.args` at the param index
2. Verify it's a `GenericArgKind::Ctor(_)` 
3. Apply the constructor to the folded argument type via `interner.apply_ctor(ctor, folded_arg)`
4. This produces a concrete type (e.g., `Option<folded_arg>`)

This leverages existing infrastructure:
- `apply_ctor` already exists (defined in PLAN.md Step 8)
- The fold machinery already exists
- No new unification logic needed

### Test the Fix

Once fold logic is implemented:
```bash
./build/x86_64-unknown-linux-gnu/stage1/bin/rustc --edition 2021 \
  tests/ui/type-constructors/examples/effects-system.rs -o /tmp/test
```

Should produce no errors.

### Verification Checklist

- [ ] `Ctor(Param(F), A)` folds to `Ctor(Known(Option), A)` during instantiation
- [ ] `identity(opt)` type-checks without error
- [ ] Error messages don't show `Param` or unresolved `Var` for constructors
- [ ] Full compiler builds
- [ ] effects-system.rs test passes

## Architecture Summary (Corrected)

```
User Code: fn<F<_>>(x: F<A>) → F<A>
    ↓
Lowering (hir_analysis)
    ↓ creates Ctor(Param(F), A)
Type Signature (cached in tcx)
    ↓
Type Checking (hir_typeck)
    ↓
TypeCheckRootCtxt::new()
    ↓ instantiate_value_path() → fresh generic args
Generic Args (includes F → ?)
    ↓
ArgFolder::fold_ty() → NEW CASE: Ctor
    ↓ ctor_for_param() substitutes F with bound constructor
Folded Signature: Ctor(Known(Option), i32)
    ↓
Argument Type Checking
    ↓ unify: Ctor(Known(Option), i32) ⊑ Adt(Option, [i32])
    ↓ (converted to Ctor(Known(Option), i32) via coercion/normalization)
Type-Checked Code ✓
```

## Key Insights

1. **Inference is wrong model** - Constructors aren't unknowns to be solved; they're parameters to be substituted.

2. **Use existing machinery** - The fold/substitution system already handles this pattern correctly for types. Reuse it for constructors.

3. **Ctor types reach argument checking as abstract** - `Ctor(Param(F), A)` in the lowered signature needs to become concrete (`Ctor(Known(...), ...)`) before argument matching. This happens via fold, not inference.

4. **Gap was architectural, not implementational** - Session 15 completed the infrastructure (unification, representation) but missed that instantiation needed a different approach.

## Next Session

Start with `ArgFolder::fold_ty` in `binder.rs`. Implement the `Ctor` arm and the `ctor_for_param` method. This is the keystone—once it's in place, everything else should fall into place via existing machinery.

---

**Status:** Phase 3 has correctness issues. Phase 4 is the real implementation work. The test case is the spec.
