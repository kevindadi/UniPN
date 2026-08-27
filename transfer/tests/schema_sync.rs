//! The alarm for the one real cost of mirroring ConcIR's schema.
//!
//! [`concir::ast`] is a **dev-dependency only**. Three things would break if it
//! became a real one:
//!
//! 1. ConcIR marks its structs `deny_unknown_fields`; we must not, or a field
//!    added upstream turns every conversion into a hard error while our
//!    submodule pin lags behind.
//! 2. A `path` dependency on a submodule makes this crate unpublishable and
//!    forces the submodule on everyone who uses it. Right now only these tests
//!    need it.
//! 3. Putting `concir::ast::Program` in the public API would pin *the caller's*
//!    ConcIR revision to ours. Two revisions in one binary are two incompatible
//!    `Program` types. JSON is the interface precisely so that cannot happen.
//!
//! What the mirror does cost is drift, and drift is what this file watches: both
//! ASTs read ConcIR's own example corpus, and the fields the lowering actually
//! uses have to agree.

use std::path::PathBuf;

use unipn_transfer::concir::ast as mine;

fn examples() -> Vec<(String, String)> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("third_party")
        .join("ConcIR")
        .join("examples");
    let entries = std::fs::read_dir(&dir).unwrap_or_else(|e| {
        panic!(
            "cannot read {}: {e}\nrun `git submodule update --init`",
            dir.display()
        )
    });

    let mut found = Vec::new();
    for entry in entries {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|ext| ext == "json") {
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            found.push((name, std::fs::read_to_string(&path).unwrap()));
        }
    }
    assert!(!found.is_empty(), "no examples in the ConcIR submodule");
    found
}

/// ConcIR's `Op` is internally tagged, so its own `kind` string round-trips out
/// of a serialization. Our mirror answers the same question directly.
fn their_kind(op: &concir::ast::Op) -> String {
    serde_json::to_value(op).unwrap()["kind"]
        .as_str()
        .unwrap()
        .to_owned()
}

fn their_bounds(base: &concir::ast::BaseType) -> Option<(i64, i64)> {
    match base {
        concir::ast::BaseType::Complex(concir::ast::ComplexBaseType::BoundedInt { lo, hi }) => {
            Some((*lo, *hi))
        }
        _ => None,
    }
}

#[test]
fn both_asts_read_the_corpus_the_same_way() {
    for (name, json) in examples() {
        let mine: mine::Program = serde_json::from_str(&json).unwrap();
        let theirs: concir::ast::Program = serde_json::from_str(&json).unwrap();

        assert_eq!(mine.program, theirs.program, "{name}: program");
        assert_eq!(mine.version, theirs.version, "{name}: version default");
        assert_eq!(mine.entry, theirs.entry, "{name}: entry");
        assert_eq!(mine.modules.len(), theirs.modules.len(), "{name}: modules");

        for (m, t) in mine.modules.iter().zip(&theirs.modules) {
            let at = format!("{name}:{}", m.name);
            assert_eq!(m.name, t.name, "{at}: module name");

            // Resources: everything the lowering branches on. `kind` picks
            // place-versus-store, `type` picks the place's flavour, and
            // `count` / `capacity` / the bounded-Int domain become numbers in
            // the net.
            assert_eq!(m.resources.len(), t.resources.len(), "{at}: resource count");
            for (r, s) in m.resources.iter().zip(&t.resources) {
                let at = format!("{at}:{}", r.name);
                assert_eq!(r.name, s.name, "{at}: name");
                assert_eq!(r.kind, s.kind, "{at}: kind");
                assert_eq!(r.res_type, s.res_type, "{at}: type");
                assert_eq!(r.mode, s.mode, "{at}: mode");
                assert_eq!(r.count, s.count, "{at}: count");
                assert_eq!(r.capacity, s.capacity, "{at}: capacity");
                assert_eq!(r.init, s.init, "{at}: init");
                assert_eq!(
                    r.base.as_ref().and_then(mine::BaseType::bounded_int),
                    s.base.as_ref().and_then(their_bounds),
                    "{at}: bounded Int domain"
                );
            }

            assert_eq!(m.functions.len(), t.functions.len(), "{at}: function count");
            for (f, g) in m.functions.iter().zip(&t.functions) {
                let at = format!("{at}::{}", f.name);
                assert_eq!(f.name, g.name, "{at}: name");
                assert_eq!(f.kind, g.kind, "{at}: kind");
                assert_eq!(f.form, g.form, "{at}: form default");

                // `modeled` decides what enters the variable store, so a change
                // in its default would silently change the state space.
                let my_slots: Vec<&str> = f.modeled_slots().collect();
                let their_slots: Vec<&str> = g
                    .params
                    .iter()
                    .filter(|p| p.modeled)
                    .map(|p| &*p.name)
                    .chain(g.locals.iter().filter(|l| l.modeled).map(|l| &*l.name))
                    .collect();
                assert_eq!(my_slots, their_slots, "{at}: modeled slots");

                // The statement list is the control flow. Same sids in the same
                // order, same operation at each one.
                assert_eq!(f.body.len(), g.body.len(), "{at}: body length");
                for (a, b) in f.body.iter().zip(&g.body) {
                    assert_eq!(a.sid, b.sid, "{at}: sid order");
                    assert_eq!(
                        a.op.kind_name(),
                        their_kind(&b.op),
                        "{at}@{}: op kind",
                        a.sid
                    );
                }
            }
        }
    }
}

#[test]
fn our_mirror_tolerates_a_field_upstream_would_reject() {
    // This is reason 1 made concrete, and the reason the mirror is not just
    // duplication. A newer ConcIR annotating statements with a source line must
    // not stop us from reading its output.
    let json = r#"{
      "program": "annotated",
      "modules": [{
        "name": "m",
        "functions": [{
          "name": "f",
          "kind": "normal",
          "body": [{"sid": "s1", "kind": "return", "line": 42}]
        }]
      }],
      "entry": "m::f"
    }"#;

    let ours: Result<mine::Program, _> = serde_json::from_str(json);
    assert!(ours.is_ok(), "our AST should ignore `line`: {ours:?}");

    let theirs: Result<concir::ast::Program, _> = serde_json::from_str(json);
    assert!(
        theirs.is_err(),
        "if ConcIR ever drops deny_unknown_fields, reason 1 for the mirror is \
         gone and this file's header should be revisited"
    );

    // And the tolerated program still lowers.
    let lowered = unipn_transfer::cvn_from_concir_json(json).unwrap();
    assert!(lowered.transition_named("m::f@s1").is_some());
}
