# Session 26 Handoff

## Discovery: Generic Arg Lowering Doesn't Create Fresh Ctor Vars

**Status**: Identified the exact code path where fresh ctor vars should be created, but they're not being created. Root cause still narrowing down.

### The Blocker

Test still fails with `expected Option<i32>, found identity::F<_>`. Fresh args contain `Known(CtorDef { def_id: identity::F })` instead of `Var(?c)`.

### What We Know

**The code path for generic arg instantiation:**
1. `check_expr_call` (callee.rs:65) type-checks the function call
2. `check_expr_path` (expr.rs:555) type-checks the callee path (e.g., `identity`)
3. `instantiate_value_path` (fn_ctxt/_impl.rs:968) creates fresh args for the function's generics
4. Calls `probe_generic_path_segments` to identify which items have generics
5. Calls `lower_generic_args` with a callback `inferred_kind`
6. `inferred_kind` (line 1302) should call `self.fcx.var_for_def(self.span, param)` for each param

**The var_for_def implementation exists** (mod.rs:948-961):
```rust
GenericParamDefKind::TypeCtor => {
    let ctor_var_id = self.inner.borrow_mut()
        .ctor_unification_table()
        .new_key(CtorVariableValue::Unknown { ... })
        .vid;
    let arg = self.tcx.mk_ctor_var_arg(ctor_var_id).into();
    debug!(?param.name, ?arg, "var_for_def: created fresh ctor var");
    arg
}
```

**BUT**: The debug log "var_for_def: created fresh ctor var" **never appears** in compiler output. This means `var_for_def` is never called.

### Why var_for_def Isn't Called

Three hypotheses:

**1. `probe_generic_path_segments` doesn't identify the function as having generics**
   - If generic_segments is empty, lower_generic_args won't iterate over params
   - Need to check: does probe_generic_path_segments correctly return GenericPathSegment for a bare function path like `identity`?

**2. `lower_generic_args` exists but inferred_kind callback isn't triggered**
   - If the path has explicit generic args `identity::<T, A>()`, those go through `provided_kind`, not `inferred_kind`
   - For bare paths, should use `inferred_kind`, but might be skipping it somehow

**3. The function def_id isn't being treated as having TypeCtor params**
   - Even if generic segments are identified, if TypeCtor params aren't in the generics_of() result, they won't be iterated over
   - Or if has_generics check filters them out

### Evidence Trail

From test output (effects-system.stderr):
```
ctor_for_param: ctor=CtorArg(Param(F/#0)), arg=A/#1
  -> ctor is param: F/#0, args=[CtorArg(Known(CtorDef { def_id: DefId(0:4 ~ effects_system[9a03]::identity::F), args: [] })), ?2t]
```

The signature has `Param(F/#0)`. When it's looked up in fresh args at index 0, we get `Known(CtorDef { def_id: identity::F })`. This comes from `GenericArgs::extend_with_error` (generic_args.rs:545-557):

```rust
pub fn extend_with_error(...) -> GenericArgsRef<'tcx> {
    ty::GenericArgs::for_item(tcx, def_id, |def, _| {
        if let Some(arg) = original_args.get(def.index as usize) {
            *arg
        } else {
            def.to_error(tcx)  // <-- Creates dummy Known(CtorDef)
        }
    })
}
```

So the ctor param slot is **missing from original_args**, which means `instantiate_value_path` didn't create a fresh ctor var for it.

### Next Steps

1. **Check probe_generic_path_segments**: Does it return a GenericPathSegment for bare function paths? Trace the resolution to see if `identity` is being recognized as having generic parameters.

2. **Check lower_generic_args call site**: At fn_ctxt/_impl.rs:1323-1337, verify that:
   - `generic_segments` is non-empty
   - The segments include all function generics, including TypeCtor params
   - `inferred_kind` is being called for at least some params

3. **Check if TypeCtor params are being filtered**: Look at has_generics or similar checks that might exclude TypeCtor params from being considered "generic".

4. **Instrument lower_generic_args**: Add eprintln to see which params are being iterated and which branch (provided_kind vs inferred_kind) is taken for each.

### Files to Check Next Session

- `compiler/rustc_hir_analysis/src/hir_ty_lowering/mod.rs:2046` - `probe_generic_path_segments`
- `compiler/rustc_hir_analysis/src/hir_ty_lowering/generics.rs` - Generic arg lowering logic
- `compiler/rustc_hir_typeck/src/fn_ctxt/_impl.rs:1236-1320` - `CtorGenericArgsCtxt` and callback impl
