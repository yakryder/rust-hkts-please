# Session 10 Handoff: Constructor Inference Variables

## Current State
**MVP type constructors work end-to-end.** Functions and structs generic over `F<_>` now parse, type-check, and unify cleanly (Session 9 complete).

**What's missing:** Constructor inference. We can call `identity::<Option, i32>(Some(1))` with explicit type args, but `identity(Some(1))` with implicit `?F` inference doesn't work yet.

## The Core Bug (Critical Discovery)

**Location:** `compiler/rustc_type_ir/src/binder.rs:836-860` in `ctor_for_param` method

**The Problem:**
When substituting a `Ctor(ParamCtor_F, arg_ty)` type where the constructor parameter is bound to an **inference variable** `Var(CtorVid)`:

```rust
fn ctor_for_param(&mut self, ctor_param: I::ParamCtor, arg_ty: I::Ty) -> I::Ty {
    let opt_ctor = self.args.get(ctor_param.index() as usize).map(|a| a.kind());
    let ctor = match opt_ctor {
        Some(ty::GenericArgKind::Ctor(ctor)) => ctor,
        // ... error cases ...
    };
    
    if self.cx.ctor_is_identity(ctor) {
        return self.cx.mk_ty_ctor(ctor_param, substituted_arg);
    }
    
    // BUG: This tries to apply inference variable ctor, which fails!
    self.cx.apply_ctor(ctor, substituted_arg)
}
```

The issue: `ctor_is_identity()` in `compiler/rustc_middle/src/ty/context/impl_interner.rs:102-112` returns **false** for `Var(_)`:

```rust
fn ctor_is_identity(self, ctor: ty::CtorArg<'tcx>) -> bool {
    match ctor.kind() {
        ty::CtorArgKind::Known(ctor_def) => {
            matches!(self.def_kind(ctor_def.def_id), DefKind::TyParam)
        }
        ty::CtorArgKind::Var(_) => false,  // <-- WRONG! Should be true or handled specially
    }
}
```

When `ctor_is_identity` returns false for an inference variable, the code attempts `apply_ctor(Var(?F), arg)`, which fails because `apply_ctor` tries to look up an ADT definition via the CtorVid, which doesn't exist yet (it's unresolved).

## The Fix (Surgical)

**Change 1: Treat inference variable constructors as identity**

In `compiler/rustc_middle/src/ty/context/impl_interner.rs`, modify `ctor_is_identity`:

```rust
fn ctor_is_identity(self, ctor: ty::CtorArg<'tcx>) -> bool {
    match ctor.kind() {
        ty::CtorArgKind::Known(ctor_def) => {
            matches!(self.def_kind(ctor_def.def_id), DefKind::TyParam)
        }
        ty::CtorArgKind::Var(_) => true,  // Inference vars are "abstract" like params
    }
}
```

**Rationale:** An inference variable constructor is unresolved—we don't yet know if it's concrete or abstract. Until it's solved, it must be kept as an abstract `Ctor` type. This mirrors the handling of identity constructors.

## Files to Modify

**Primary (MUST FIX):**
- `compiler/rustc_middle/src/ty/context/impl_interner.rs:102-112` — Fix `ctor_is_identity` to return true for `Var(_)`

**Verification (check after fix):**
- `tests/ui/type-constructors/examples/effects-system.rs` — Should uncomment lines 27-28 and pass when `identity(r)` is called with inference

## Test Case (Currently Failing)

File: `tests/ui/type-constructors/examples/effects-system.rs`

```rust
fn identity<F<_>, A>(x: F<A>) -> F<A> { x }

fn main() {
    let r: Result<i32, String> = Ok(42);
    let _: Result<i32, String> = identity(r);  // Should infer F=Result
}
```

**Current state:** Parser works, type-checks hit substitution bug when `ctor_for_param` tries to apply inference var.

**After fix:** Should compile (inference vars will be kept abstract until solved by unification).

## Architecture Notes

### Type Constructor Representation (for reference)
- **`TyKind::Ctor(ParamCtor, Ty)`** — Abstract constructor with parameter, e.g., `F<A>`
- **`GenericArgKind::Ctor(CtorArg)`** — Concrete constructor as generic argument (for substitution)
- **`CtorArg = Interned<CtorArgKind>`** where `CtorArgKind = Known(CtorDef) | Var(CtorVid)`
- **`CtorArgKind::Known`** — Concrete constructor (e.g., Result, Option)
- **`CtorArgKind::Var`** — Unresolved inference variable (`?F`)

### Key Methods
- `var_for_def(param: GenericParamDef)` in `rustc_infer/src/infer/mod.rs:933-944` — Creates `CtorVid` inference vars
- `ctor_for_param(param, arg)` in `rustc_type_ir/src/binder.rs:836-860` — Substitutes Ctor types (has the bug)
- `ctor_is_identity(ctor)` in `rustc_middle/src/ty/context/impl_interner.rs:102-112` — Checks if ctor is abstract (needs fix)
- `apply_ctor(ctor, arg)` in `rustc_middle/src/ty/context/impl_interner.rs` — Applies concrete ctor to arg

## Build & Test Commands

```bash
# Verify the fix compiles
./x build compiler --stage 1

# Run the test that should pass after fix
./x test tests/ui/type-constructors/examples/effects-system.rs --stage 1

# Run full test suite
./x test tests/ui/type-constructors/ --stage 1
```

## Next Steps (Post-Fix)

After applying the 1-line fix to `ctor_is_identity`:

1. ✅ Recompile and verify no new errors
2. ✅ Run `effects-system.rs` test — should pass
3. Consider: If unification of `?F ~ Result` still doesn't happen, trace where constructor equations are actually solved in the relate module

## Identity Note for Next Session

You're one surgical fix away from constructor inference working. The infrastructure is all there—`CtorVid` creation, unification tables, substitution. The bug is a single mis-classification: inference variables need to be treated as "abstract" during substitution, not "concrete."

Think of it this way:
- `Ctor(ParamCtor_F, A)` — abstract, F is a param
- `Ctor(ParamCtor_F, A)` after substitution with `Var(?F)` — still abstract! The param is just unresolved now, not concrete

The fix makes that semantics explicit.
