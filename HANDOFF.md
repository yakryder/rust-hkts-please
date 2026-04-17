# Session 21 Handoff

## Discovery

**Root cause identified**: TypeCtor params lack a match arm in the generic args lowering pipeline.

### The Bug

In `compiler/rustc_hir_analysis/src/hir_ty_lowering/generics.rs:253`, the inner match statement that handles generic argument lowering has arms for:
- Lifetime params
- Type params  
- Const params

**Missing**: TypeCtor params

When `lower_generic_args` encounters a TypeCtor parameter (e.g., `F` in `fn<F<_>, A>`), it falls through to the error case (line 289) instead of calling `ctx.inferred_kind()` to create a fresh ctor var.

### Why This Matters

The flow for function calls is:
1. `instantiate_value_path` → `lower_generic_args` → match on param.kind
2. For Type/Const with no user args → calls `inferred_kind` → calls `var_for_def` → creates fresh var
3. For TypeCtor with no user args → **no match arm** → falls through → error path

This prevents `var_for_def(TypeCtor)` from being called, so fresh ctor vars are never created. The args array ends up with the signature's Param ctors instead of fresh Vars.

---

## The Fix (Ready to Apply)

Add a match arm in `compiler/rustc_hir_analysis/src/hir_ty_lowering/generics.rs` after line 263:

```rust
(
    GenericArg::Infer(_) | GenericArg::Type(_) | GenericArg::Const(_),
    GenericParamDefKind::TypeCtor,
    _,
) => {
    // TypeCtor param with mismatched user arg: infer the ctor param
    args.push(ctx.inferred_kind(&args, param, infer_args));
    args_iter.next();
    params.next();
}
```

This matches the pattern for other param kinds: when the user provides something that doesn't match, or nothing at all, we call `inferred_kind` to create the appropriate fresh variable.

---

## Expected Result

After this fix:
1. `lower_generic_args` will call `ctx.inferred_kind()` for TypeCtor params
2. `inferred_kind` calls `fcx.var_for_def(span, param)`
3. `var_for_def` hits the `GenericParamDefKind::TypeCtor` case (line 947 in mod.rs)
4. Creates fresh ctor var via `self.tcx.mk_ctor_var_arg(ctor_var_id)`
5. Fresh args contain `Var(?c)` instead of `Known(identity::F)`
6. Test should pass: `Option<i32>` argument unifies correctly with fresh `?c<i32>`

---

## Files Modified

| File | Change | Impact |
|------|--------|--------|
| `compiler/rustc_hir_analysis/src/hir_ty_lowering/generics.rs` | Add TypeCtor match arm (after line 263) | Enables fresh ctor var creation |

## Next Steps

1. Apply the match arm addition
2. Run quick check: `cargo check -p rustc_hir_analysis`
3. Full build: `python x.py build compiler/rustc_hir_analysis compiler/rustc_type_ir compiler/rustc_infer`
4. Test: `python x.py test tests/ui/type-constructors/examples/effects-system.rs`
5. Commit as: `WIP: Session 21 - Add TypeCtor match arm to lower_generic_args`

---

## Key Insight

The compiler infrastructure for HKTs is **structurally complete**:
- BoundVariableKind::Ctor exists ✓
- fresh_args_for_item creates Var(?c) for TypeCtor ✓
- var_for_def handles TypeCtor correctly ✓
- ctor_for_param folds signatures correctly ✓

The bug was a **single missing match arm** preventing the generic lowering pipeline from routing TypeCtor params to the inference machinery. This is a pattern completion issue, not a design gap.
