# HKT Project Context

**Goal**: Add higher-kinded type (HKT) support — type constructor parameters `F<_>` where `F: * -> *`.

```rust
fn identity<F<_>, A>(x: F<A>) -> F<A> { x }
identity(Some(42): Option<i32>)  // F = Option, A = i32 → Some(42)
```

**Status (Session 20)**: Bound variable infrastructure added; fresh ctor vars created in instantiate; error persists: fresh_args still shows `Known(identity::F)` instead of `Var(?c)`. **Next**: verify BoundVariableKind::Ctor is emitted during lowering.

---

## Rust Compiler Architecture (Relevant Parts)

### Key Crates

- **`compiler/rustc_type_ir/`** — Generic, interner-agnostic type system traits. Defines `TyKind`, fold/visit machinery, `Interner` trait. This is where new type variants and their structural rules live.
- **`compiler/rustc_middle/`** — The concrete `TyCtxt` (type context), `Ty<'tcx>` (types), `GenericArg`. Implements the `Interner` trait for `TyCtxt`.
- **`compiler/rustc_infer/`** — Type inference: `InferCtxt`, unification tables, the `relate` machinery that checks type equality/subtyping.
- **`compiler/rustc_hir_typeck/`** — HIR-level type checking: function calls, argument checking, writeback.
- **`compiler/rustc_hir_analysis/`** — Type collection: lowers HIR to types, including generics and function signatures.

### Key Types

- **`Ty<'tcx>`**: An interned type. `ty.kind()` returns the `TyKind` enum variant.
- **`ty::Adt(adt_def, args)`**: A concrete ADT like `Option<i32>`. `args` are `GenericArg` slices.
- **`ty::Param(p)`**: A generic type parameter like `T` or `A`.
- **`ty::Infer(TyVar(v))`**: A type inference variable (like `?T`).
- **`GenericArg<'tcx>`**: One of: Type, Lifetime, Const, or (HKT-new) Ctor.
- **`EarlyBinder<T>`**: Wraps a type/signature that has uninstantiated generic params. `instantiate(tcx, args)` substitutes them.

### How Generic Instantiation Works

`EarlyBinder::instantiate(tcx, args)` runs `ArgFolder` over the type:
- `fold_ty` handles `ty::Param(p)` → looks up `args[p.index()]` → returns the substituted type
- `fold_ty` also handles `ty::Ctor(ctor_param, arg_ty)` → calls `ctor_for_param` (HKT-specific)

### How Type Inference Works

When a function is called, fresh inference variables are created for each generic param. The signature is instantiated with those variables. Then argument types are related against the instantiated parameter types via `InferCtxt::relate` → `TypeRelating::tys()` → `super_combine_tys()` → `structurally_relate_tys()`.

---

## HKT Additions (What Has Been Built)

### New Type Variant: `ty::Ctor(ctor_arg, arg_ty)`

A type constructor application: `F<A>` is represented as `Ctor(ctor_arg, arg_ty)` where:
- `ctor_arg: CtorArg<'tcx>` — the constructor (param, inference var, or concrete)
- `arg_ty: Ty<'tcx>` — the applied type argument

### `CtorArgKind` (in `compiler/rustc_middle/src/ty/sty.rs`)

```rust
pub enum CtorArgKind<'tcx> {
    Param(ParamCtor),         // uninstantiated ctor param, e.g. F in fn<F<_>>
    Known(CtorDef<'tcx>),     // concrete ctor, e.g. Option
    Var(CtorVid),             // inference variable for a ctor
}

pub struct CtorDef<'tcx> {
    pub def_id: DefId,        // e.g. Option's DefId
    pub args: GenericArgsRef<'tcx>,  // pre-bound args (for partial application)
}
```

### `GenericArgKind::Ctor(CtorArg)`

A fourth kind of generic argument, alongside Type/Lifetime/Const. When calling `identity(opt)`, the compiler creates fresh `GenericArgKind::Ctor(Var(?CtorVid))` for the `F` parameter.

### Interner Trait Methods (in `compiler/rustc_type_ir/src/interner.rs`)

```rust
fn apply_ctor(self, ctor: CtorArg, arg: Ty) -> Ty;
    // apply_ctor(Known(Option), i32) → Option<i32>
fn ctor_is_identity(self, ctor: CtorArg) -> bool;
    // true for Param and Var (unresolved); false for concrete Known ctors
fn ctor_as_infer_var(self, ctor: CtorArg) -> Option<CtorVid>;
fn ctor_arg_as_param(self, ctor: CtorArg) -> Option<ParamCtor>;
fn ctor_arg_is_param(ctor: CtorArg) -> bool;
fn mk_ty_ctor(self, ctor_arg: CtorArg, arg: Ty) -> Ty;
fn decompose_ctor_application(self, ty: Ty) -> Option<(CtorArg, Ty)>;
    // decompose_ctor_application(Option<i32>) → Some((Known(Option), i32))
    // Only works for 1-ary ADTs (args.len() == 1) currently
```

### Fold Machinery (ArgFolder::ctor_for_param)

`Ctor(Param(F), A)` + args `[Ctor(Var(?v)), i32]`:
1. Lookup Param(F) @ index 0 → Var(?v)
2. ctor_is_identity(Var(?v)) = true
3. Return Ctor(Var(?v), i32)

### Unification (type_relating.rs::ctor_args)

`(Var(?v), Known(Option))` → `instantiate_ctor_var(?v, Known(Option))` → unification table records `?v = Known(Option)`

### Flags (flags.rs)

`Ctor(Param(F), A)`: HAS_TY_PARAM=true (visits on fold). `Ctor(Var(?v), i32)`: HAS_TY_PARAM=false (fold done).

---

## Phase 3: Fresh Constructor Variables

**Target test**: `tests/ui/type-constructors/examples/effects-system.rs`
- `identity(opt: Option<i32>)` must infer `F=Option, A=i32`
- Current error: `expected Option<i32>, found identity::F<_>`

**Blocker**: `instantiate_binder_with_fresh_vars` must create fresh `Var(?c)` for ctor params, not `Known(identity::F)`.

**Expected flow**:
1. BoundVariableKind::Ctor emitted during signature lowering
2. instantiate_binder_with_fresh_vars: Ctor → `next_ctor_var()` → `Var(?c)`
3. ArgFolder substitutes signature with fresh args
4. Unification solves `?c = Known(Option)`
5. shallow_resolve applies solved ctor → type checks ✓

---

## Feedback Loops

**Quick compile check** (10s): `cargo check -p rustc_type_ir -p rustc_infer`

**Full build** (3 min): `python x.py build compiler/rustc_type_ir compiler/rustc_middle compiler/rustc_infer compiler/rustc_hir_typeck`

**Test output** (instant post-build): `cat build/x86_64-unknown-linux-gnu/test/ui/type-constructors/examples/effects-system/effects-system.err`

Never use `python x.py test` for iteration — captures output live; read directly instead.

**Commits**: WIP only — just title, no body. Format: `WIP: Session N - <short description>`

## Important Lint

`rustc_infer` calls to `rustc_type_ir::Interner` need:
```rust
#[allow(rustc::usage_of_type_ir_traits)]
fn helper<'tcx>(tcx: TyCtxt<'tcx>, ...) {
    use rustc_type_ir::Interner as _;
    tcx.method(...)
}
```

---

## Validation Strategy

### Test Layers (ordered by what they catch)

| Layer | Test Pattern | Catches |
|-------|-------------|---------|
| **Parse** | `tests/ui/type-constructors/parse/` | Syntax errors in `F<_>` |
| **Gate** | `tests/ui/type-constructors/gate/` | Feature gate enforcement |
| **Inference** | `//@ check-pass` + explicit return type annotation | Wrong/missing inference (silent failures) |
| **Rejection** | `//~ ERROR` on mismatched lines | Missing constraint enforcement |

### Falsifiable Ctor Var Tests

**A. Fresh var creation** (current blocker):
```rust
//@ check-pass
fn identity<F<_>, A>(x: F<A>) -> F<A> { x }
fn main() { let _: Option<i32> = identity(Some(42)); }
```
Fails with `expected Option<i32>, found F<_>` if Var(?c) never created.

**B. Substitution (fold replaces Param with Var)**:
```rust
//@ check-pass
fn apply_twice<F<_>, A>(x: F<A>, y: F<A>) -> F<A> { x }
fn main() { let _: Option<i32> = apply_twice(Some(1), Some(2)); }
```
Fails if the two F<A> positions don't share the same ctor var.

**C. Ctor conflict rejection**:
```rust
fn identity<F<_>, A>(x: F<A>) -> F<A> { x }
fn main() { let _: Vec<i32> = identity(Some(42)); } //~ ERROR mismatched types
```
If this passes, unification is not constraining ctor vars.

**D. Distinct ctor params**:
```rust
//@ check-pass
fn pair<F<_>, G<_>, A>(x: F<A>, y: G<A>) -> F<A> { x }
fn main() { let _: Option<i32> = pair(Some(1), vec![2]); }
```

### Debugging (instead of eprintln!)

**Tracing** (preferred): Use `tracing::debug!` with `?` formatting:
```rust
use tracing::debug;
debug!(?fresh_args, ?def_id, "fresh_args_for_item");
```
Run with: `RUSTC_LOG=rustc_infer::infer=debug ./build/.../stage1/bin/rustc test.rs 2>&1 | grep fresh_args`

**Targeted filter examples**:
```bash
RUSTC_LOG=rustc_infer::infer=debug,rustc_hir_typeck=debug
RUSTC_LOG=rustc_hir_analysis::hir_ty_lowering=debug
```

**Invariant enforcement** (loudness hierarchy):
1. `span_bug!(span, "msg")` -- ICE with backtrace. For impossible states.
2. `debug_assert!(cond, "msg")` -- Panics in debug builds. For invariants.
3. `//~ ERROR` in tests -- CI catches regressions. For expected errors.
4. `//@ check-pass` -- CI catches regressions. For must-compile cases.
5. `tracing::debug!` -- Silent unless RUSTC_LOG set. For flow tracing.

**Rule**: Never use `eprintln!` in compiler code. It bypasses tracing filters and will be rejected by CI. Use `debug!()` or `span_bug!()` instead.

### Blessing stderr Files

To capture current (broken) error output as a baseline:
```bash
python x.py test tests/ui/type-constructors/examples/effects-system.rs --bless
```
This creates/updates the `.stderr` file. When the fix lands, the test either passes (`.stderr` deleted) or produces a different error (`.stderr` mismatch = test failure). Both are informative.

### Anti-pattern: "Wrong But Passes"

The most dangerous failure mode is silent wrong inference. Guard against it by:
1. Always annotate expected return types: `let _: Option<i32> = ...` not `let _ = ...`
2. Test rejection (mismatched ctors must error)
3. Test that distinct ctor params stay distinct
4. Never write `//@ check-pass` without at least one explicit type annotation that would break under wrong inference
