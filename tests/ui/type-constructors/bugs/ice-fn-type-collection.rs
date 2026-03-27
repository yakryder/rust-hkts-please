//@ known-bug: unknown
// ICEs at: compiler/rustc_middle/src/ty/context.rs:2313
// bug!("mk_param_from_def: TypeCtor params not yet supported (Step 8)")
//
// When Step 8 is done: move to parse/gated-with-allow.rs or a new
// tests/ui/type-constructors/basic/ subdirectory.
#![feature(type_constructors)]
#![allow(incomplete_features)]

fn foo<F<_>>() {}

fn main() {}
