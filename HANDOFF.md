# Session 18 Handoff: Path B Implemented, Three Gaps Remain

## What Was Done This Session

**Established Path B as the correct architectural approach:**
- Rejected adding `(Ctor, Adt)` unification to `structurally_relate_tys` (wrong level)
- Chose: decompose `Option<i32>` → `Ctor(Known(Option), i32)` *before* relating, so both sides are `Ctor` and the existing `ctor_args(Var, Known)` machinery fires

**Implemented:**
1. `decompose_ctor_application` on `Interner` trait (`compiler/rustc_type_ir/src/interner.rs`)
2. Implementation for 1-ary ADTs (`compiler/rustc_middle/src/ty/context/impl_interner.rs`):
   - `Option<i32>` → `Some((Known(CtorDef { def_id: Option, args: [] }), i32))`
   - Only handles `args.len() == 1` for now (covers Option, Vec, Box)
3. New match arm in `type_relating.rs::tys()` (`compiler/rustc_infer/src/infer/relate/type_relating.rs:229`):
   ```rust
   (&ty::Ctor(ctor_arg, a_arg), _)
       if ctor_as_infer_var_helper(infcx.tcx, ctor_arg).is_some() =>
   {
       if let Some((b_ctor, b_arg)) = decompose_ctor_application_helper(infcx.tcx, b) {
           self.ctor_args(ctor_arg, b_ctor)?;
           self.relate(a_arg, b_arg)?;
       } else {
           super_combine_tys(infcx, self, a, b)?;
       }
   }
   ```
   With helpers at lines ~15-35 using `#[allow(rustc::usage_of_type_ir_traits)]`.

**Compiles clean. Test still fails with same error.**

---

## Why The Test Still Fails: Three Gaps

### Gap 1: Symmetric arm missing (most likely cause of arm not firing)

The current arm matches `(Ctor(?v, _), _)` — i.e., when `a` is the `Ctor` type. But when checking `identity(opt)`, rustc may pass `a = Option<i32>` (actual) and `b = Ctor(?v, ?A)` (expected param type). Need the mirror:

```rust
(_, &ty::Ctor(ctor_arg, b_arg))
    if ctor_as_infer_var_helper(infcx.tcx, ctor_arg).is_some() =>
{
    if let Some((a_ctor, a_arg)) = decompose_ctor_application_helper(infcx.tcx, a) {
        self.ctor_args(a_ctor, ctor_arg)?;
        self.relate(a_arg, b_arg)?;
    } else {
        super_combine_tys(infcx, self, a, b)?;
    }
}
```

**File:** `compiler/rustc_infer/src/infer/relate/type_relating.rs` — add immediately after the existing `(&ty::Ctor(...), _)` arm (around line 238).

### Gap 2: `shallow_resolve` does not resolve Ctor types

`InferCtxt::shallow_resolve` (`compiler/rustc_infer/src/infer/mod.rs:1108`) only resolves `ty::Infer(TyVar(...))`. A `Ctor(Var(?v), i32)` type is NOT a `ty::Infer` — it's a `ty::Ctor`. So even after `?v = Known(Option)` is recorded in the ctor unification table, `shallow_resolve(Ctor(Var(?v), i32))` returns `Ctor(Var(?v), i32)` unchanged.

**Fix needed:** Extend `shallow_resolve` to handle `ty::Ctor`:

```rust
// In InferCtxt::shallow_resolve, after the existing ty::Infer match:
pub fn shallow_resolve(&self, ty: Ty<'tcx>) -> Ty<'tcx> {
    if let ty::Infer(v) = *ty.kind() {
        // ... existing TyVar/IntVar/FloatVar handling ...
    } else if let ty::Ctor(ctor_arg, arg_ty) = *ty.kind() {
        // If ctor arg is a solved inference variable, apply it
        if let ty::CtorArgKind::Var(vid) = ctor_arg.kind() {
            let value = self.inner.borrow_mut().ctor_unification_table().probe_value(vid);
            if let crate::infer::CtorVariableValue::Known { value: ctor_def } = value {
                let known_ctor = self.tcx.mk_ctor_arg(ty::CtorArgKind::Known(ctor_def));
                return self.tcx.apply_ctor(known_ctor, arg_ty);
            }
        }
        ty
    } else {
        ty
    }
}
```

Check `CtorVariableValue` in `compiler/rustc_infer/src/infer/unify_key.rs` for the exact enum variant names.

### Gap 3: Writeback may need Ctor resolution

The writeback pass (`compiler/rustc_hir_typeck/src/writeback.rs`) resolves all inference variables in the final types. It uses `fully_resolve` which walks types recursively. If it encounters `Ctor(Var(?v), T)` and doesn't know to resolve it via the ctor unification table, the final type will still be abstract.

Search for `resolve_vars_if_possible` or `fully_resolve` in `rustc_hir_typeck/src/writeback.rs` and check if `ty::Ctor` is handled. It likely walks through it structurally via `TypeFoldable` but doesn't call `apply_ctor`. 

The writeback folder in `rustc_infer` (`compiler/rustc_infer/src/infer/resolve.rs` or similar) handles `Infer` types — it likely also needs a `Ctor(Var(?v), T)` → `apply_ctor(Known(X), T)` case.

---

## Recommended Order of Fixes

1. **Add symmetric arm** — 5 lines, same file, likely makes our arm fire
2. **Extend `shallow_resolve`** — needed so `Ctor(?v, T)` normalizes to `X<T>` when `?v` is solved
3. **Check writeback** — verify final types are resolved correctly; may work via `shallow_resolve` propagation

After each step: `python x.py test tests/ui/type-constructors/examples/effects-system.rs`

---

## Key File Locations

| File | Purpose |
|------|---------|
| `compiler/rustc_infer/src/infer/relate/type_relating.rs` | The `tys()` method where our new arm lives |
| `compiler/rustc_infer/src/infer/mod.rs:1108` | `shallow_resolve` — needs Ctor extension |
| `compiler/rustc_infer/src/infer/relate/generalize.rs:231` | `instantiate_ctor_var` — records `?v = Known(X)` |
| `compiler/rustc_infer/src/infer/unify_key.rs` | `CtorVariableValue`, `CtorVidKey` |
| `compiler/rustc_middle/src/ty/context/impl_interner.rs` | `apply_ctor`, `decompose_ctor_application` |
| `compiler/rustc_type_ir/src/binder.rs:836` | `ctor_for_param` — fold machinery |
| `tests/ui/type-constructors/examples/effects-system.rs` | The target test |

---

## The Full Expected Flow (When Working)

```
identity(opt) where opt: Option<i32>

1. Fresh vars: [Ctor(Var(?c)), TyVar(?a)] for [F, A]
2. Instantiate sig: Ctor(Param(F), Param(A)) → ctor_for_param → Ctor(Var(?c), TyVar(?a))
3. Argument check: relate(Option<i32>, Ctor(Var(?c), TyVar(?a)))
   → symmetric arm fires
   → decompose_ctor_application(Option<i32>) → (Known(Option), i32)
   → ctor_args(Known(Option), Var(?c)) → instantiate_ctor_var(?c, Known(Option))
   → relate(i32, TyVar(?a)) → ?a = i32
4. shallow_resolve(Ctor(Var(?c), TyVar(?a)))
   → ?c is solved → apply_ctor(Known(Option), i32) → Option<i32>
5. Return type check: Option<i32> == Option<i32> ✓
```
