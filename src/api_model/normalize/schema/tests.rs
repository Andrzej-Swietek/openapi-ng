//! Property tests for the schema walk.

use std::rc::Rc;

use proptest::prelude::*;

use super::normalize_named_schema;
use crate::api_model::normalize::MAX_NORMALIZE_DEPTH;
use crate::error::{DiagnosticCode, Reporter};
use crate::parse::openapi_model::Schema;

fn arb_schema(max_depth: u32) -> impl Strategy<Value = Schema> {
  let leaf = Just(Schema::default_string());
  leaf.prop_recursive(max_depth, 32, 4, |inner| {
    prop_oneof![
      inner.clone().prop_map(Schema::wrap_array),
      proptest::collection::vec(inner.clone(), 0..3).prop_map(Schema::wrap_one_of),
      inner.prop_map(Schema::wrap_nullable),
    ]
  })
}

proptest! {
  #![proptest_config(ProptestConfig {
    cases: 128,
    ..ProptestConfig::default()
  })]

  #[test]
  fn normalize_named_schema_never_panics(schema in arb_schema(40)) {
    let path: Rc<str> = Rc::from("test");
    let reporter = Reporter::new(path);
    let result = normalize_named_schema("Root", &schema, &reporter);

    if let Err(diag) = result {
      prop_assert!(
        matches!(
          diag.code,
          DiagnosticCode::UnsupportedSemantic | DiagnosticCode::PolicyViolation,
        ),
        "unexpected diagnostic code: {:?}", diag.code,
      );
    }
  }
}

#[test]
fn nested_inline_objects_are_charged_one_level_each() {
  // Arrays already covered the depth guard; inline objects descend through
  // `normalize_properties`, which is where a double charge would hide.
  let depth = usize::from(MAX_NORMALIZE_DEPTH) - 2;
  let schema = (0..depth).fold(Schema::default_string(), |inner, _| {
    Schema::wrap_object("child", inner)
  });

  let path: Rc<str> = Rc::from("test");
  let reporter = Reporter::new(path);
  assert!(
    normalize_named_schema("Root", &schema, &reporter).is_ok(),
    "{depth} nested inline objects must fit under a cap of {MAX_NORMALIZE_DEPTH}"
  );
}

#[test]
fn depth_exceeded_diagnostic_includes_breadcrumb_chain() {
  // Build a 40-level-deep schema by wrapping in array; MAX_NORMALIZE_DEPTH is 32.
  let schema = (0..40).fold(Schema::default_string(), |inner, _| {
    Schema::wrap_array(inner)
  });

  let path: Rc<str> = Rc::from("test");
  let reporter = Reporter::new(path);
  let err = normalize_named_schema("Root", &schema, &reporter)
    .expect_err("should fail with depth exceeded");

  assert!(
    err.message.contains("32"),
    "expected depth limit in message: {}",
    err.message,
  );
  assert!(
    err.message.contains("Root"),
    "expected root breadcrumb in message: {}",
    err.message,
  );
}
