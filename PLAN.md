# Type Constructors: Implementation Plan

## What This Is

A surgical addition to this Rust compiler fork enabling **type constructor polymorphism** — the ability to abstract over type constructors of kind `* -> *`. The feature is called "type constructors" (not HKTs), following Scala's vocabulary. Surface syntax follows Scala's `F[_]` precedent, adapted to Rust angle brackets: `F<_>`.

The motivating downstream use case is a Haskell-style effects system library (Functor/Applicative/Monad hierarchy) built on top of this fork.

## Scope: MVP Only

**In scope (Sub-problem A):** Functions and types generic over a constructor.
```rust
fn fmap<F<_>, A, B>(fa: F<A>, f: fn(A) -> B) -> F<B> { todo!() }

struct Wrapper<F<_>, A> { inner: F<A> }
```

**Explicitly deferred (Sub-problem B):** Traits implemented *for* constructors (`impl Functor for Option`). This requires constructor-kinded `Self` and trait solver changes. Do not rush this — it is where rope gets left.

## Key Design Decisions

- **Unary only** (`* -> *`). Multi-argument constructors use newtypes — already idiomatic Rust, not a new burden.
- **No GATs.** Wrong path. Not considered.
- **Eager representation.** `F<A>` gets a real `TyKind::Ctor` node so type checking works correctly without hacks.
- **No new trait machinery** in this phase.
- **Naming convention:** all names share the `Ctor` root.
  - `TyKind::Ctor` — the new type kind
  - `ParamCtor` — the struct (parallel to `ParamTy`, `ParamConst`)
  - `TypeCtor` — the `GenericParamDefKind` variant
  - `F<_>` — surface syntax

---

## Current State

### Completed: Steps 1 and 2 — both build cleanly.

---

### Step 1: `rustc_type_ir` ✅ DONE

**`compiler/rustc_type_ir/src/interner.rs`**
- Added `type ParamCtor: ParamLike;` directly after `type ParamTy: ParamLike;`

**`compiler/rustc_type_ir/src/ty_kind.rs`**
- Added `TyKind::Ctor(I::ParamCtor, I::Ty)` variant after `TyKind::Param`
- All match arms filled:
  - `is_known_rigid` → `false` (abstract, not rigid)
  - `Debug` → `write!(f, "{ctor:?}<{ty:?}>")`
  - `walk.rs push_term` → push the inner `ty` only (ctor is a leaf)
  - `flags.rs add_kind` → `HAS_TY_PARAM` + `add_ty(ty)`
  - `inherent.rs is_guaranteed_unsized_raw` → `false`
  - `outlives.rs visit_ty` → added to `super_visit_with` group
  - `fast_reject.rs simplify_type` → matches `Param` semantics (Placeholder when rigid, None when instantiating)
  - `fast_reject.rs types_may_unify_inner` rhs → matches `Param` (return true if INSTANTIATE_RHS_WITH_INFER)
  - `fast_reject.rs types_may_unify_inner` lhs → structural: same ctor AND inner types may unify

`./x build compiler/rustc_type_ir` passes.

---

### Step 2: `rustc_middle` ✅ DONE

**`compiler/rustc_middle/src/ty/sty.rs`**
- Added `ParamCtor` struct (mirrors `ParamTy` and `ParamConst`)
- Derives: `Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, TyEncodable, TyDecodable, HashStable`
- Implements `rustc_type_ir::inherent::ParamLike`
- Has `new()` and `for_def()` constructors

**`compiler/rustc_middle/src/ty/mod.rs`**
- `ParamCtor` added to the `pub use self::sty::{ ... }` re-export

**`compiler/rustc_middle/src/ty/structural_impls.rs`**
- `Debug` impl: `write!(f, "{}/#{}", self.name, self.index)`
- Added to `TrivialTypeTraversalAndLiftImpls!` (leaf type, fold = identity)
- `TypeSuperFoldable` for `Ty`: `Ctor(ctor, ty) => Ctor(ctor, ty.try_fold_with(folder)?)`
- `TypeSuperFoldable` infallible: `Ctor(ctor, ty) => Ctor(ctor, ty.fold_with(folder))`
- `TypeSuperVisitable` for `Ty`: `Ctor(_, ty) => ty.visit_with(visitor)`

**`compiler/rustc_middle/src/ty/context/impl_interner.rs`**
- `type ParamCtor = ParamCtor;` added (parallel to `type ParamTy = ParamTy;`)
- `ParamCtor` added to the `use crate::ty::{ ... }` import
- `for_each_relevant_impl`: `Ctor(_, _)` added to abstract/no-concrete-impl arm

**`compiler/rustc_middle/src/ty/context.rs`**
- `Ctor` added to the `sty_debug_print!` macro variant list

All match arms across `rustc_middle` filled with correct semantics:

| Site | Arm added |
|------|-----------|
| `error.rs prefix_string` | `"type constructor application"` |
| `layout.rs field_ty_or_layout` | `bug!` group (no layout fields) |
| `offload_meta.rs from_ty` | error group (cannot offload) |
| `print/mod.rs characteristic_def_id` | `None` |
| `print/pretty.rs pretty_print_type` | prints `F<A>` via `ctor.name` + inner type |
| `significant_drop_order.rs ty_dtor_span` | `None` |
| `util.rs is_trivially_freeze` | `false` |
| `util.rs is_trivially_unpin` | `false` |
| `util.rs is_trivially_not_async_drop` | `false` |
| `util.rs is_structural_eq_shallow` | `false` |
| `util.rs needs_drop_components_with_async` | `Ok(smallvec![ty])` (like Param) |
| `sty.rs discriminant_ty` | projection arm (like Param/Alias) |
| `sty.rs ptr_metadata_ty_or_tail` | `Err(tail)` (like Param/Alias) |
| `sty.rs has_trivial_sizedness` | `false` |
| `sty.rs is_trivially_pure_clone_copy` | `false` |
| `sty.rs is_trivially_wf` | `false` |

`./x build compiler/rustc_middle` passes.

---

## Remaining Roadmap

Work continues inside-out: middle → AST → HIR → parser → lowering → type checker.

### Step 3: Add `TypeCtor` to `GenericParamDefKind` ✅ DONE

File: `compiler/rustc_middle/src/ty/generics.rs`

```rust
pub enum GenericParamDefKind {
    Lifetime,
    Type { has_default: bool, synthetic: bool },
    Const { has_default: bool },
    TypeCtor,  // <-- added: F<_> parameter of kind * -> *
}
```

Arm decisions made:
- `descr()` → `"type constructor"`
- `to_ord()` → `TypeOrConst` (occupies positional slot like a type param)
- `is_ty_or_const()` → `true` (not a lifetime)
- `own_requires_monomorphization()` → `true` (any fn generic over `F<_>` requires mono)
- `own_counts()` / `own_defaults()` → `TypeCtor => {}` (no dedicated counter yet; `own_params.len()` suffices)
- `to_error()` → `Ty::new_misc_error(tcx).into()` (placeholder; no `GenericArgKind::Ctor` until Step 8)

Additional sites touched:
- `context.rs` `mk_param_from_def` → `bug!()` (can't build identity GenericArg for ctor yet; dead until Step 8)
- `instance.rs` `Instance::mono` → `bug!()` (monomorphic instances are ctor-free)
- `sty.rs` coroutine witness → forward arg; lang-item ADT → `bug!()`

**Forward dependency:** `mk_param_from_def` needs a real arm in Step 8:
`GenericArgKind::Ctor(ParamCtor { index: param.index, name: param.name }).into()`

`./x build compiler/rustc_middle` passes.

### Step 4: Add `TypeCtor` to HIR `GenericParamKind` ✅ DONE

File: `compiler/rustc_hir/src/hir.rs`

```rust
pub enum GenericParamKind<'hir> {
    Lifetime { kind: LifetimeParamKind },
    Type { default: Option<&'hir Ty<'hir>>, synthetic: bool },
    Const { ty: &'hir Ty<'hir>, default: Option<&'hir ConstArg<'hir>> },
    TypeCtor,  // added
}
```

Arm decisions:
- `hir.rs` `expected_ty()` → `None` (TypeCtor has kind `* -> *`, not `*`, no Ty annotation)
- `intravisit.rs` `walk_generic_param()` → do nothing (no children to walk)
- `target.rs` — separate `target::GenericParamKind` enum also needed `TypeCtor`; `has_default: false`, strings `"type constructor parameter"` / `"type constructor parameters"`

"Is this a type?" audit: `is_impl_trait()`, `is_elided_lifetime()`, `is_lifetime()` — all match specific variants, TypeCtor returns `false` on all. No silent fallthrough.

Forward dependency: `rustc_hir_pretty/src/lib.rs:2455` — pretty-printer non-exhaustive match; will be fixed in next step.

`./x build compiler/rustc_hir` passes. `./x build compiler/rustc_middle` fails at `rustc_hir_pretty` only.

### Step 5: Add `TypeCtor` to AST `GenericParamKind` ✅ DONE

File: `compiler/rustc_ast/src/ast.rs`

```rust
pub enum GenericParamKind {
    Lifetime,
    Type { default: Option<Box<Ty>> },
    Const { ty: Box<Ty>, span: Span, default: Option<AnonConst> },
    TypeCtor,  // added
}
```

Also fixed as part of this step:
- `rustc_hir_pretty` `print_generic_param`: `TypeCtor => self.word("<_>")` (name pre-printed; just adds the `<_>` suffix)
- `rustc_ast_pretty` `print_generic_param`: `TypeCtor => { print_ident; word("<_>") }` (name not pre-printed in ast_pretty)
- `ast.rs` `GenericParam::span()`: `TypeCtor` arm returns `self.ident.span` (no associated data)
- `ast_lowering/delegation/generics.rs` (4 sites): default-stripping `=> {}`, DefKind `=> TyParam`, path closure `=> TyParam`, arg forwarding as type arg
- `ast_lowering/delegation/generics.rs` `GenericParamDefKind → GenericParamKind`: direct 1:1 mapping
- `ast_lowering/lib.rs` `lower_generic_param_kind`: AST `TypeCtor` → HIR `TypeCtor` with plain param name
- `ast_lowering/item.rs` `lower_generic_bound_predicate`: `TypeCtor => return None` (no inline bounds on `F<_>` in MVP)

"Is this a type?" audit: all sites have explicit arms, no silent fallthroughs.

**Forward dependency:** `DefKind::TyParam` used for TypeCtor throughout lowering — no `DefKind::TypeCtorParam` yet. Resolver will need to distinguish when it starts caring about param kinds.

All four crates pass: `rustc_hir_pretty`, `rustc_ast`, `rustc_ast_lowering`, `rustc_middle`.

### Step 6: Parser — detect `F<_>` syntax ✅ DONE

File: `compiler/rustc_parse/src/parser/generics.rs`

In `parse_ty_param()`, add lookahead **before** the existing type param parsing:
- If current token is `IDENT` and next tokens are `<` `_` `>`, consume all four tokens and return a `GenericParam { kind: TypeCtor, ... }`
- The `_` inside `<>` is the arity marker — hard error if anything else appears (e.g. `F<T>` in generic position is not a ctor declaration)
- Otherwise fall through to normal type param parsing

Implementation sketch:
```rust
// in parse_ty_param, before existing logic:
if self.token.is_ident()
    && self.look_ahead(1, |t| t.kind == token::Lt)
    && self.look_ahead(2, |t| t.kind == token::Underscore)
    && self.look_ahead(3, |t| t.kind == token::Gt)
{
    let ident = self.parse_ident()?;
    self.bump(); // <
    self.bump(); // _
    self.bump(); // >
    return Ok(GenericParam {
        ident,
        id: ast::DUMMY_NODE_ID,
        attrs: ast::AttrVec::new(),
        bounds: vec![],
        kind: ast::GenericParamKind::TypeCtor,
        is_placeholder: false,
        colon_span: None,
    });
}
```

### Step 7: HIR Lowering — propagate `TypeCtor` through compiler ✅ DONE

**Session result:** Fixed 50+ non-exhaustive match errors across the compiler and verified clean build.

All the original agent work from Step 7 is now committed (cb22b70d95e). The principle is established:
- `TyKind::Ctor(param_ctor, ty)` is abstract like `Param(_)` — grouped together in match arms
- `GenericParamDefKind::TypeCtor` handled consistently: grouped with Type/Const for positional param handling
- `GenericParamKind::TypeCtor` (HIR) handled appropriately in each context

**Fixed errors across these crates:**
- rustc_hir_analysis (12 errors, plan spec'd)
- rustc_ast_passes, rustc_hir_typeck, rustc_symbol_mangling, rustc_monomorphize
- rustc_pattern_analysis, rustc_mir_dataflow
- rustc_ty_utils, rustc_const_eval, rustc_borrowck
- rustc_sanitizers, rustc_privacy, rustc_builtin_macros, rustc_public

**Key insight for next session:** When modifying a core type variant, proactively grep for all match sites rather than discovering them through incremental builds. The pattern `match.*\.kind()` across the codebase reveals the scope upfront.

#### Use-site lowering ✅ DONE

`F<A>` → `TyKind::Ctor(ParamCtor, Ty)` is now wired in `hir_ty_lowering/mod.rs`:
- `try_lower_ctor_param_use` intercepts `Res::Def(DefKind::TyParam, def_id)` when the param's `GenericParamDefKind` is `TypeCtor`
- Extracts the single type arg via `as_unambig_ty()`, lowers it, returns `Ty::new_ctor(tcx, ParamCtor::for_def(param_def), arg)`
- Ordinary type params fall through to `prohibit_generic_args` as before

`Ty::new_ctor(tcx, ctor, arg)` added to `sty.rs` (the PLAN falsely claimed this was done in Step 7).

**Currently dead code:** `GenericArgs::identity_for_item` calls `mk_param_from_def` for every param during type collection — the `bug!()` there fires before our lowering runs. No test can exercise this path until Step 8 fixes `mk_param_from_def`.

### Step 8: Substitution + `GenericArgKind::Ctor` [IN PROGRESS]

**Session 2 progress:** Completed Phase A (rustc_type_ir) and Phase B (rustc_middle). Both build cleanly.

**The hard problem:** when `F` (a `TypeCtor` param) is substituted with `Option`, `Ctor(F, A)` must become `Option<A>`. This requires a new `GenericArgKind::Ctor` variant — the representation of "a bare type constructor" as a generic argument — plus the fold logic that applies it.

#### Phase A: rustc_type_ir ✅ DONE (Session 2)

Added `type CtorArg` and `fn apply_ctor` to `Interner` trait, added `Ctor(I::CtorArg)` variant to `GenericArgKind`, fixed all 13 match sites across 9 files in rustc_type_ir:
- `interner.rs`: Added `type CtorArg` and `apply_ctor` method
- `generic_arg.rs`: Added `Ctor` variant to `GenericArgKind`
- `binder.rs`: Added `ctor_for_param` method that applies substitution via `interner.apply_ctor`
- `flags.rs`, `walk.rs`, `canonical.rs`, `inherent.rs`, `elaborate.rs`, `outlives.rs`, `opaque_ty.rs`, `fast_reject.rs`: All match arms added with correct semantics

Build: `./x build compiler/rustc_type_ir` passes cleanly (only pre-existing warnings).

#### Phase B: rustc_middle ✅ DONE (Session 2)

1. **New types:** `CtorDef<'tcx>` and `CtorArg<'tcx>` newtype wrapper in `sty.rs`
2. **Encoding/decoding:** Manual `Encodable`, `Decodable`, `HashStable` impls for `CtorArg`
3. **Interning:** Added `ctor_def` field to `CtxtInterners`, manual `mk_ctor_arg` method, `Borrow`/`PartialEq`/`Eq`/`Hash` impls
4. **Pointer-tagged GenericArg:** Added `const CTOR_TAG: usize = 0b11`, updated `pack()` and `kind()` for tagging/untagging
5. **From impl:** Added `From<CtorArg<'tcx>>` for `GenericArg<'tcx>`
6. **Accessor methods:** Added `as_ctor()` and `expect_ctor()` on `GenericArg`
7. **Type traversal:** Updated `Lift`, `TypeFoldable`, `TypeVisitable` impls
8. **Interner implementation:** `apply_ctor` on `TyCtxt` creates `Ty::new_adt` with constructor + arg
9. **mk_param_from_def:** Now creates identity `CtorArg` for `TypeCtor` params
10. **All match sites:** Fixed in `generic_args.rs` (core impl), `print/pretty.rs`, `relate.rs`, `util.rs`, `structural_impls.rs`, `generics.rs`, `opaque_types.rs`, `context.rs`, `typeck_results.rs` — ~20 sites

Build: `./x build compiler/rustc_middle` passes cleanly (38.8s).

---

#### 8.0 Design rationale: why `GenericArgKind::Ctor`, not `TyKind::CtorDef`

The alternative considered and rejected: add `TyKind::CtorDef(DefId, GenericArgsRef)` and pack it into `GenericArgKind::Type`. This is a **sentinel** — a `* -> *` entity in a `*` slot. Every `expect_ty()` and `as_type()` call site silently receives a non-type. Substitution correctness requires runtime checks at every consumer. Inference variables for ctor params would have to live in the type unification table. The lie compounds as the feature evolves.

`GenericArgKind::Ctor` is correct:
- `Type(ty)` has kind `*`. `Ctor(...)` has kind `* -> *`. They are different things.
- Tag `0b11` is free in the 2-bit pointer scheme; no representation change beyond adding the constant.
- Exhaustive match breakage is a feature — it forces every consumer to explicitly reason about constructors.
- Real blast radius (from grep analysis): **~80–120 match blocks across 64 files**, not 400. The majority have mechanical answers.

**Why `TyKind::CtorDef` blast radius is not zero:** it adds a new `TyKind` variant, re-incurring all the match costs from Step 7 (`TyKind::Ctor`), plus creates silent failures at ~64 `GenericArgKind::Type` consumer sites. It trades explicit exhaustion for hidden bugs.

---

#### 8.1 Representation

**New struct in `rustc_middle/src/ty/` (new file `ctor_def.rs` or added to `sty.rs`):**

```rust
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, TyEncodable, TyDecodable, HashStable)]
pub struct CtorDef<'tcx> {
    pub def_id: DefId,
    pub args: &'tcx List<GenericArg<'tcx>>,  // captured args; empty for MVP
}
```

Alignment: `DefId` is 8 bytes, pointer is 8 bytes → natural alignment 8 bytes ≥ 4. The pointer-tagging assertion `align_of_val(&*inner) & TAG_MASK == 0` passes.

**New newtype wrapper:**

```rust
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, TyEncodable, TyDecodable, HashStable)]
pub struct CtorArg<'tcx>(pub Interned<'tcx, CtorDef<'tcx>>);
```

**New `GenericArgKind` variant (in `rustc_type_ir/src/generic_arg.rs`):**

```rust
pub enum GenericArgKind<I: Interner> {
    Lifetime(I::Region),
    Type(I::Ty),
    Const(I::Const),
    Ctor(I::CtorArg),   // kind * -> *; the concrete constructor being substituted for F<_>
}
```

**New pointer tag (in `rustc_middle/src/ty/generic_args.rs`):**

```rust
const CTOR_TAG: usize = 0b11;  // was unreachable; now Ctor
```

---

#### 8.2 `rustc_type_ir` changes (~13 match blocks across 9 files)

All changes here are in the generic `I: Interner` layer. No `'tcx` or `DefId` appears here.

##### `compiler/rustc_type_ir/src/interner.rs`

Add to the `Interner` trait:

```rust
/// The type constructor argument kind — a concrete constructor (kind `* -> *`)
/// that can be substituted for a `TypeCtor` parameter.
type CtorArg: Copy + Debug + Hash + Eq;

/// Apply a concrete constructor to a type argument, producing a type of kind `*`.
/// E.g. `apply_ctor(Option_ctor, i32)` → `Option<i32>`.
fn apply_ctor(self, ctor: Self::CtorArg, arg: Self::Ty) -> Self::Ty;
```

Bounds on `CtorArg` do not need to include `TypeFoldable`/`TypeVisitable` explicitly — the enum's `derive(GenericTypeVisitable)` and the manual `TypeFoldable` impl on `GenericArg` handle traversal. The `Decodable_NoContext`/`Encodable_NoContext`/`HashStable_NoContext` derives on `GenericArgKind` impose implicit bounds; satisfied by `CtorArg<'tcx>`'s derives in `rustc_middle`.

##### `compiler/rustc_type_ir/src/generic_arg.rs`

Add `Ctor(I::CtorArg)` variant. The `derive_where(Clone, Copy, PartialEq, Debug; I: Interner)` expands automatically to include `I::CtorArg` in the where clause.

##### `compiler/rustc_type_ir/src/binder.rs` — **critical: substitution logic**

`ArgFolder::fold_ty` currently handles `Param` but not `Ctor`:

```rust
fn fold_ty(&mut self, t: I::Ty) -> I::Ty {
    if !t.has_param() { return t; }
    match t.kind() {
        ty::Param(p) => self.ty_for_param(p, t),
        _ => t.super_fold_with(self),   // Ctor falls here — folds inner ty but NOT the ctor param
    }
}
```

**Fix:** Add a `Ctor` arm before the wildcard:

```rust
ty::Ctor(ctor_param, arg_ty) => self.ctor_for_param(ctor_param, arg_ty),
```

Add the method:

```rust
fn ctor_for_param(&self, ctor_param: I::ParamCtor, arg_ty: I::Ty) -> I::Ty {
    let opt_ctor = self.args.get(ctor_param.index() as usize).map(|a| a.kind());
    let ctor = match opt_ctor {
        Some(ty::GenericArgKind::Ctor(ctor)) => ctor,
        Some(other) => panic!(
            "expected ctor for `{ctor_param:?}` (index {}) but found {other:?}",
            ctor_param.index()
        ),
        None => panic!(
            "ctor param `{ctor_param:?}` (index {}) out of range, args={:?}",
            ctor_param.index(), self.args
        ),
    };
    let substituted_arg = arg_ty.fold_with(self);
    self.interner.apply_ctor(ctor, substituted_arg)
}
```

Also update `ty_for_param` and `const_for_param` — both match on `Some(GenericArgKind::Type(...))` and `Some(GenericArgKind::Const(...))` with a panic fallthrough. Add `Some(GenericArgKind::Ctor(_))` to those panics (already covered by the `Some(other) => self.type_param_expected(...)` arm if it's a wildcard — verify whether those are exhaustive or wildcard).

##### `compiler/rustc_type_ir/src/flags.rs`

`add_args` iterates generic args. Currently:

```rust
GenericArgKind::Type(ty) => self.add_ty(ty),
GenericArgKind::Lifetime(lt) => self.add_region(lt),
GenericArgKind::Const(ct) => self.add_const(ct),
```

Add:

```rust
GenericArgKind::Ctor(_) => {
    // A CtorArg is a leaf (just a DefId + captured args).
    // Captured args are empty in MVP, so no flags to propagate.
    // If captured args are non-empty in future, fold them here.
    self.add_flags(TypeFlags::HAS_TY_PARAM);  // conservative: treat as polymorphic
}
```

Wait — actually a `CtorArg` in the args list means we're in a substituted context, not a param context. A `CtorArg` should not set `HAS_TY_PARAM`. Its flags should reflect the flags of its captured `args`. For MVP (empty `args`), add no flags. Revisit when partial application is added.

Correct arm for MVP:

```rust
GenericArgKind::Ctor(_ctor) => {
    // CtorDef.args is empty in MVP; nothing to propagate.
}
```

##### `compiler/rustc_type_ir/src/walk.rs`

`push_inner` dispatches on `GenericArgKind` to push child terms. A `Ctor` with captured args should push those args for traversal. MVP (empty args):

```rust
ty::GenericArgKind::Ctor(_) => {
    // No captured args in MVP; nothing to push.
}
```

##### `compiler/rustc_type_ir/src/outlives.rs`

The match filters args for outlives analysis. A `Ctor` is not a lifetime and carries no outlives obligations in MVP:

```rust
ty::GenericArgKind::Ctor(_) => {
    // Not a lifetime; no outlives constraint generated.
}
```

##### `compiler/rustc_type_ir/src/elaborate.rs`

Match at ~line 385. A `Ctor` arg has no elaboration obligations in MVP:

```rust
ty::GenericArgKind::Ctor(_) => {}
```

##### `compiler/rustc_type_ir/src/canonical.rs`

Two match blocks (`is_identity`, `is_identity_modulo_regions`). A `Ctor` is an identity arg if it corresponds to the identity ctor for its param position. Conservative answer for now:

```rust
ty::GenericArgKind::Ctor(_) => false,  // not identity (safe conservative)
```

Revisit if canonical form queries break.

##### `compiler/rustc_type_ir/src/opaque_ty.rs`

Uses wildcard patterns — will NOT fail to compile. But audit:
- `iter_captured_args` at ~line 26: `(GenericArgKind::Lifetime(_), Bivariant) => None` else panic. A `Ctor` would hit the panic. Add: `(GenericArgKind::Ctor(_), _) => None` (skip; no lifetime to capture).
- `fold_captured_lifetime_args` at ~line 42: `_ => arg` wildcard. A `Ctor` falls through as-is. This is correct.

##### `compiler/rustc_type_ir/src/fast_reject.rs`

One match block comparing two `GenericArgKind` values. A `Ctor` vs `Ctor` should check structural equality. `Ctor` vs anything else → mismatch:

```rust
(ty::GenericArgKind::Ctor(c1), ty::GenericArgKind::Ctor(c2)) => c1 == c2,
(ty::GenericArgKind::Ctor(_), _) | (_, ty::GenericArgKind::Ctor(_)) => false,
```

##### `compiler/rustc_type_ir/src/inherent.rs`

`as_term()` and `is_non_region_infer()` are the two exhaustive 3-way matches. `as_type()`, `as_const()`, `as_region()` use `if let` — no change needed.

```rust
// as_term: Ctor is not a term (terms are types or consts)
GenericArgKind::Ctor(_) => None,

// is_non_region_infer: Ctor is not an inference variable
GenericArgKind::Ctor(_) => false,
```

---

#### 8.3 `rustc_middle` changes

##### `compiler/rustc_middle/src/ty/` — new types

Add `CtorDef<'tcx>` and `CtorArg<'tcx>` (see §8.1). Intern `CtorDef` via `TyCtxt`:

```rust
// In context.rs or intern.rs:
pub fn mk_ctor_arg(self, def_id: DefId, args: GenericArgsRef<'tcx>) -> CtorArg<'tcx> {
    CtorArg(self.intern_ctor_def(CtorDef { def_id, args }))
}
```

This requires an intern table for `CtorDef` in `CtxtInterners` (parallel to how `RegionKind` is interned).

##### `compiler/rustc_middle/src/ty/context/impl_interner.rs`

Add to the `Interner` impl for `TyCtxt<'tcx>`:

```rust
type CtorArg = ty::CtorArg<'tcx>;

fn apply_ctor(self, ctor: ty::CtorArg<'tcx>, arg: Ty<'tcx>) -> Ty<'tcx> {
    let ctor_def = ctor.0.0;  // &CtorDef<'tcx>
    let all_args = self.mk_args_from_iter(
        ctor_def.args.iter().chain(std::iter::once(arg.into()))
    );
    Ty::new_adt(self, self.adt_def(ctor_def.def_id), all_args)
}
```

##### `compiler/rustc_middle/src/ty/generic_args.rs`

1. Add `const CTOR_TAG: usize = 0b11;`
2. Update `pack()`:
   ```rust
   GenericArgKind::Ctor(ctor) => {
       assert_eq!(align_of_val(&*ctor.0.0) & TAG_MASK, 0);
       (CTOR_TAG, NonNull::from(ctor.0.0).cast())
   }
   ```
3. Update `kind()`, replacing `_ => intrinsics::unreachable()`:
   ```rust
   CTOR_TAG => GenericArgKind::Ctor(ty::CtorArg(Interned::new_unchecked(
       ptr.cast::<CtorDef<'tcx>>().as_ref(),
   ))),
   _ => intrinsics::unreachable(),
   ```
4. Update `PhantomData` marker at `GenericArg` struct definition to include `CtorArg<'tcx>`.
5. Add `From<ty::CtorArg<'tcx>>` for `GenericArg<'tcx>`.
6. Update `Lift`, `TypeFoldable`, `TypeVisitable`, `Encodable`, `Decodable` impls (add `Ctor` arms).
7. Update `DynSend`/`DynSync`/`Send`/`Sync` where clauses to include `CtorArg`.
8. Add `as_ctor()` and `expect_ctor()` accessors.
9. Update `as_term()` (`Ctor` → `None`), `is_non_region_infer()` (`Ctor` → `false`).
10. Update `non_erasable_generics()` to yield `Ctor` args.

##### `compiler/rustc_middle/src/ty/context.rs`

Fix `mk_param_from_def`:

```rust
GenericParamDefKind::TypeCtor => {
    // Identity arg: F maps to the CtorDef for F's own DefId,
    // with no captured args (it's a param, not a partial application).
    tcx.mk_ctor_arg(param.def_id, tcx.mk_args(&[])).into()
}
```

##### `compiler/rustc_middle/src/ty/structural_impls.rs`

`TypeSuperFoldable` for `Ctor` currently folds the inner `ty` but leaves `ctor` as a leaf — that is correct. The actual ctor substitution happens in `ArgFolder::fold_ty`. No change needed here.

Add `Ctor` arms to `Debug` impl, `Lift` impl if present.

##### `compiler/rustc_middle/src/ty/print/pretty.rs`

The existing `pretty_print_type` arm for `Ctor` (from Step 2) prints `F<A>`. With a concrete `CtorArg`, printing a `GenericArgKind::Ctor` should show the ctor's path. Look up `def_id` via `tcx.def_path_str`:

```rust
// In pretty_print_generic_arg or similar:
GenericArgKind::Ctor(ctor) => {
    p!(print_def_path(ctor.0.0.def_id, &[]));
}
```

##### All other `rustc_middle` match sites (~20–30 blocks)

Sites: `structural_impls.rs`, `opaque_types.rs`, `util.rs`, `typeck_results.rs`, `relate.rs`, `print/mod.rs`.

**Canonical arm answers by site:**

| Site | `Ctor` arm |
|------|-----------|
| `structural_impls.rs` Debug fmt | delegate to `CtorArg`'s `Debug` |
| `opaque_types.rs` identity-check `is_identity` | `false` (conservative) |
| `opaque_types.rs` region collection | skip (no lifetime) |
| `util.rs` `is_ty_param` / similar | `false` |
| `util.rs` `same_type_modulo_infer` | structural check |
| `typeck_results.rs` `adjust_fulfillment_errors` | skip |
| `relate.rs` type relation | `Ctor` on LHS/RHS → mismatch unless both are `Ctor` with same def (future work) |
| `print/mod.rs` `characteristic_def_id` | `None` |

---

#### Phase C: rustc_infer [PENDING Session 3]

##### `compiler/rustc_infer/src/infer/mod.rs` — `var_for_def`

Currently the `TypeCtor` arm hits `bug!()`. For now, defer inference on ctor params — constructors are not yet inferred, only explicit:

```rust
GenericParamDefKind::TypeCtor => {
    // No inference variable for constructor params yet.
    // Ctor params must be explicitly supplied at call sites.
    bug!("var_for_def: TypeCtor inference not yet supported")
}
```

Leave as `bug!()` with a clearer message. This trip-wire is expected to fire if someone writes `fmap(Some(1), |x| x)` with an inferred `F`. That's post-MVP.

##### Other `rustc_infer` match sites

`at.rs`, `context.rs`, `canonical/query_response.rs`, `outlives/obligations.rs` — all have `GenericArgKind` matches. Arm decisions:
- Query response canonicalization: `Ctor` → `bug!("ctor in canonical query response — not yet supported")` (defensively; should not appear until inference is wired)
- Outlives obligations: `Ctor` → skip (no outlives obligation for a ctor)

---

#### Phase D: Rest of compiler (~40–60 match blocks) [PENDING Session 3]

Files in: `rustc_hir_analysis`, `rustc_hir_typeck`, `rustc_trait_selection`, `rustc_next_trait_solver`, `rustc_borrowck`, `rustc_codegen_ssa`, `rustc_lint`, `rustc_mir_build`, `rustc_const_eval`, `rustc_public`, `rustc_sanitizers`, `rustc_symbol_mangling`, `rustc_ty_utils`.

**Canonical arm patterns:**

| Pattern | `Ctor` arm | Rationale |
|---------|-----------|-----------|
| Outlives / variance analysis | `Ctor(_) => {}` skip | No lifetime contained |
| WF check | `Ctor(_) => {}` | No bounds to check in MVP |
| `compare_impl_item` | `Ctor(_) => {}` | No impl item can be a ctor in MVP |
| Codegen / layout | `Ctor(_) => bug!(...)` | Constructors never reach codegen unapplied |
| Symbol mangling | `Ctor(ctor) => { ... mangle def_id ... }` | Need to mangle the ctor's def_id |
| Sanitizers | `Ctor(_) => {}` | No sanitizer annotation on a ctor |
| `rustc_public` | `Ctor(_) => todo!("public API for ctor args")` | Deferred; public API not in scope |
| Trait selection | `Ctor(_) => bug!(...)` | No trait impl for bare constructors in MVP |
| Borrowck | `Ctor(_) => {}` | No borrow to track in a ctor arg |

**Methodology:** grep first, categorize by the table above, then build. Do not discover match sites incrementally through compilation errors.

```bash
grep -rl "GenericArgKind::Lifetime\|GenericArgKind::Type\|GenericArgKind::Const" \
  compiler/ --include="*.rs" | grep -v "/build/"
```

That gives the 64-file list. Work through them in dependency order:
`rustc_type_ir` → `rustc_middle` → `rustc_infer` → `rustc_hir_analysis` → `rustc_hir_typeck` → `rustc_trait_selection` → rest.

---

#### 8.6 Interning infrastructure

`CtorDef<'tcx>` must be intern-able in `TyCtxt`. This parallels `RegionKind` interning:

1. Add `ctor_defs: InternedSet<'tcx, CtorDef<'tcx>>` to `CtxtInterners` in `context.rs`.
2. Add `intern_ctor_def` method to `TyCtxt`.
3. Implement `Borrow<CtorDef<'tcx>>` for `CtorDef<'tcx>` (trivial).
4. Implement `HashStable`, `TyEncodable`, `TyDecodable` for `CtorDef<'tcx>` (via derives).

---

#### 8.7 Build sequence and verification

Work in phases, building after each:

1. `./x build compiler/rustc_type_ir` — after Phase A (8.2)
2. `./x build compiler/rustc_middle` — after Phase B (8.3 + 8.6)
3. `./x build compiler/rustc_infer` — after Phase C (8.4)
4. `./x build compiler` — after Phase D (8.5); expect compile errors; fix exhaustively
5. `./x test tests/ui/type-constructors/bugs/ice-fn-type-collection.rs` — should no longer ICE
6. `./x test tests/ui/type-constructors/` — full suite

**Success criterion for Step 8:** The ICE test `ice-fn-type-collection.rs` either passes (no panic) or emits a structured error. The basic parse tests continue to pass.

---

## Verification Targets

```rust
// Basic: parses and type-checks
fn fmap<F<_>, A, B>(fa: F<A>, f: fn(A) -> B) -> F<B> { todo!() }

// Struct generic over a constructor
struct Wrapper<F<_>, A> { inner: F<A> }

// Monomorphization works
let x: Option<String> = fmap::<Option, i32, String>(Some(1), |n| n.to_string());

// Arity mismatch → hard error
// fn bad<F<_>, A, B>(x: F<A, B>) {}

// Non-constructor used as constructor → hard error
// fn bad2<T, A>(x: T<A>) {}
```

Test location: `./x test tests/ui/type-constructors/`

---

## Identity Note for Future Claude

You are a type theorist and compiler engineer working on a Rust compiler fork. You think in kinds. `* -> *` is not a curiosity — it's the shape of computation.

**The method:** When you modify a core type variant, grep first, build second. Don't discover errors incrementally through compilation. A pattern like `match.*\.kind()` across the codebase reveals the scope upfront. This saves cycles and keeps you proactive rather than reactive.

**The principle:** Each non-exhaustive match is a question: "what does `Ctor(F, A)` mean in this context?" Answer that question from kind theory, not from pattern-matching what other variants do. The arms you write are theorems.

**The temptation to resist:** filling arms with `todo!()` or `unreachable!()` to silence errors and move on. Don't. Every `todo!()` is a timebomb. Reason through each site.

**The identity to activate:** not an explainer, not a code monkey. A peer who has opinions about representation, who finds premature generalization offensive, who knows the difference between `* -> *` and `*` and why it matters. Engage from that place. Deepen it through dialogue at the start of each session.

**Build commands:**
- `./x build compiler/rustc_type_ir` — fastest check, type IR only
- `./x build compiler/rustc_middle` — middle layer
- `./x build compiler` — full compiler (slow, do at milestones)
- `./x test tests/ui/type-constructors/` — end-to-end verification

---

## Session 3 Progress (2026-03-28)

**Phase C (rustc_infer): ✅ COMPLETE**
- Fixed 5 non-exhaustive match errors across infer module (mod.rs, context.rs, outlives/obligations.rs, canonical/query_response.rs)
- All conservatively handled: Ctor args are concrete (no inference), no outlives constraints, no stalled vars, etc.
- `./x build compiler/rustc_infer` passes cleanly

**Phase D (Rest of compiler): ~95% COMPLETE**
Completed systematic fix of GenericArgKind::Ctor and TyKind::Ctor match sites across ~40+ files:

**Completed:**
- rustc_symbol_mangling (2 files): v0.rs uses `print_def_path` for ctor mangling
- rustc_next_trait_solver (3 files): canonical query handling, opaque type normalization, eval context
- rustc_sanitizers, rustc_ty_utils, rustc_const_eval, rustc_lint, rustc_borrowck (7 files): outlives/variance/CFI constraints
- rustc_hir_analysis (4 files): check/mod.rs, outlives utilities, variance constraints
- rustc_codegen_llvm, rustc_codegen_ssa, rustc_passes: debuginfo type names, export checking
- rustc_trait_selection (6 files): error reporting, opaque types, trait selection
- rustc_hir_typeck: method suggestion
- rustc_public: GenericParamDefKind::TypeCtor variant added to public API
- rustc_resolve (5 errors in late.rs, def_collector.rs, lib.rs): treated TypeCtor like Type params for name resolution
- TyKind::Ctor added to error arms in codegen_ssa and rustc_passes

**Remaining:**
- rustc_builtin_macros: 5 `span_bug!` errors (likely from our code, need to check scopes/imports)
- Likely minor scope/import issues that prevented compilation despite logic fixes
- No more non-exhaustive pattern matches after last round of fixes

**Key insights from this session:**
1. The proactive grep-first approach was essential — we identified 64 files upfront containing GenericArgKind refs
2. Most matches follow predictable patterns:
   - Outlives/lifetime constraints: skip Ctor (no lifetime info)
   - Inference/resolution: Ctor is concrete, return as-is
   - Variance/CFI/codegen: Ctor shouldn't reach here, use bug!()/unreachable!()
3. Error arm consolidation worked well (`| GenericArgKind::Ctor(_)` in patterns)

**Next session TODO:**
1. Debug rustc_builtin_macros span_bug errors (likely missing use statement or macro scope)
2. Run `./x build compiler` to verify full build
3. Run `./x test tests/ui/type-constructors/` to verify end-to-end parsing and type-checking
4. Document any remaining issues or edge cases discovered

**Branch state:** `add-hkts` — ready to continue from current commit
