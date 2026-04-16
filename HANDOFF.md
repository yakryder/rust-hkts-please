# Session 20 Handoff

## What Was Done

Added BoundVariableKind::Ctor infrastructure for ctor parameters:

1. **BoundCtorKind enum** (`rustc_type_ir/src/binder.rs:1116`): mirrors BoundTyKind with Anon/Param variants
2. **BoundVariableKind::Ctor** variant added to enum
3. **Updated bound var lowering** (`rustc_hir_analysis/src/collect/resolve_bound_vars.rs`):
   - `late_arg_as_bound_arg`: TypeCtor → BoundVariableKind::Ctor(Param(def_id))
   - `generic_param_def_as_bound_arg`: same conversion for GenericParamDef
4. **Fresh ctor var creation** (`rustc_infer/src/infer/mod.rs`):
   - `next_ctor_var()` / `next_ctor_var_with_origin()` methods
   - `instantiate_binder_with_fresh_vars`: handles BoundVariableKind::Ctor → calls next_ctor_var()
5. **Stable conversion stub** (`rustc_public/src/unstable/convert/stable/ty.rs`): placeholder for BoundVariableKind::Ctor

All crates compile. **Test still fails with same error**: fresh_args contains `Known(identity::F)` not `Var(?c)`.

---

## The Remaining Gap

**Diagnostic finding**: `instantiate_binder_with_fresh_vars` is creating fresh ctor vars, but the function call still receives `Known(identity::F)` instead of `Var(?c)` in fresh_args.

**Root cause**: Unknown. Either:
1. BoundVariableKind::Ctor is NOT being emitted during lowering (late_arg_as_bound_arg not reachable for ctor params)
2. Or the bound vars collection doesn't include ctor params
3. Or fresh_args creation bypasses instantiate_binder_with_fresh_vars entirely for this signature

**Diagnostic added** (not yet tested): eprintln in instantiate_binder_with_fresh_vars when Ctor case fires. Will confirm if this code path executes.

---

## Feedback Loop Optimization

For next session, use:
```bash
# Check only changed crates (fastest feedback)
cargo check -p rustc_type_ir -p rustc_infer 2>&1 | grep error

# Once clean, full build (3 min)
python x.py build compiler/rustc_type_ir compiler/rustc_middle compiler/rustc_infer compiler/rustc_hir_typeck

# Test output already captured; read directly (instant)
cat build/x86_64-unknown-linux-gnu/test/ui/type-constructors/examples/effects-system/effects-system.err | head -50
```

Do NOT use `python x.py test` unless needed for full harness validation.

---

## Next Steps

1. **Verify BoundVariableKind::Ctor emission**: Rebuild + check if eprintln fires
2. **If yes**: debug why fresh_args still has Known(identity::F)
3. **If no**: trace how bound_vars are collected for function signatures; likely bound_vars() doesn't include ctor params

---

## Key Files Modified

| File | What | Line |
|------|------|------|
| `compiler/rustc_type_ir/src/binder.rs` | BoundCtorKind + BoundVariableKind::Ctor | 1116, 1134 |
| `compiler/rustc_hir_analysis/src/collect/resolve_bound_vars.rs` | Emit Ctor for TypeCtor | 290, 309 |
| `compiler/rustc_infer/src/infer/mod.rs` | next_ctor_var + instantiate handling | 828, 1410 |
| `compiler/rustc_infer/src/infer/unify_key.rs` | Make CtorVariableOrigin pub | 179 |
| `compiler/rustc_public/src/unstable/convert/stable/ty.rs` | Stable conversion stub | 326 |
