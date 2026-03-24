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

### Step 7: HIR Lowering — AST → HIR ← NEXT

File: `compiler/rustc_ast_lowering/src/item.rs` (or `lib.rs`)

Two lowering sites:

**Parameter lowering:** `ast::GenericParamKind::TypeCtor` → `hir::GenericParamKind::TypeCtor`

**Use-site lowering:** When resolving a path `F<A>` where `F` resolves to a `TypeCtor` generic param, lower the type to `TyKind::Ctor(param_ctor, ty_arg)`. This happens in the type lowering path — find where `hir::TyKind::Path` is lowered for generic params and add the ctor case.

The index for `ParamCtor` in `GenericArgs` corresponds to the param's index in the generic param list. Use `ParamCtor::for_def(def)` to construct it.

### Step 8: Type Checker

File: `compiler/rustc_hir_analysis`

**Constructor to `TyCtxt`:** Add `Ty::new_ctor(tcx, param_ctor, arg_ty)` to the `Ty` inherent impl in `compiler/rustc_type_ir/src/inherent.rs` (alongside `Ty::new_param`). This is needed by lowering.

```rust
fn new_ctor(interner: I, param: I::ParamCtor, ty: I::Ty) -> Self;
```

Implement in `rustc_middle` similarly to `new_param`.

**Substitution** — the critical piece: when a `TypeCtor` param `F` is substituted with a concrete constructor `Option`, `Ctor(F, A)` must become `Option<A>`. This happens in the generic arg substitution machinery (`EarlyBinder::instantiate`).

The substitution for `Ctor(F, A)` works as:
1. Look up what `F` substitutes to in `args` — this should be a `GenericArg` that encodes a type constructor (a bare `DefId` + `GenericArgs` prefix, not a full type)
2. Apply that constructor to the substituted `A`

This is the hardest part. One approach: encode the substitution value for a `TypeCtor` param as a special `GenericArg` variant, or reuse `Ty` with a sentinel (e.g., a `ty::Adt` with a placeholder arg). Design decision needed here.

**Arity and kind errors:** In `rustc_hir_analysis`, when lowering `T<A>` where `T` is a `Type` param (not `TypeCtor`), emit: "T is not a type constructor". When lowering `F<A, B>` where `F` is `TypeCtor`, emit: "type constructor takes exactly 1 argument".

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

**The method:** let the compiler's exhaustive match errors be the roadmap. Each non-exhaustive match is a question: "what does `Ctor(F, A)` mean in this context?" Answer that question from kind theory, not from pattern-matching what other variants do. The arms you write are theorems.

**The temptation to resist:** filling arms with `todo!()` or `unreachable!()` to silence errors and move on. Don't. Every `todo!()` is a timebomb. Reason through each site.

**The identity to activate:** not an explainer, not a code monkey. A peer who has opinions about representation, who finds premature generalization offensive, who knows the difference between `* -> *` and `*` and why it matters. Engage from that place. Deepen it through dialogue at the start of each session.

**Build commands:**
- `./x build compiler/rustc_type_ir` — fastest check, type IR only
- `./x build compiler/rustc_middle` — middle layer
- `./x build compiler` — full compiler (slow, do at milestones)
- `./x test tests/ui/type-constructors/` — end-to-end verification
