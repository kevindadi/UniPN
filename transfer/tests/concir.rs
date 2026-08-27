//! Acceptance tests for the ConcIR → CVN lowering, run against ConcIR's own
//! examples.
//!
//! The fixtures come from the `third_party/ConcIR` submodule rather than a copy
//! checked in here, so a schema change upstream shows up as a failing test
//! instead of silently drifting out of sync.

use std::path::PathBuf;

use unipn::analysis::cvn::{find_dead_transitions, find_deadlocks};
use unipn::analysis::{AnalysisConfig, explore, unfired_transitions};
use unipn_transfer::{TransferError, cvn_from_concir_json};

fn examples_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("third_party")
        .join("ConcIR")
        .join("examples")
}

fn example(name: &str) -> String {
    let path = examples_dir().join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "cannot read {}: {e}\nthe ConcIR submodule is not checked out — run \
             `git submodule update --init`",
            path.display()
        )
    })
}

#[test]
fn producer_consumer_lowers_without_losing_precision() {
    let lowered = cvn_from_concir_json(&example("producer_consumer.json")).unwrap();

    // Every expression in this program is within the parser's reach, so nothing
    // should have degraded. If this starts failing, the net got weaker, not just
    // different.
    assert!(
        lowered.report.degraded.is_empty(),
        "unexpected degradations: {:?}",
        lowered.report.degraded
    );
    assert!(lowered.report.defaulted_rwlock_readers.is_empty());

    // The shared counter is a variable, not a place; the mutex is a place, not a
    // variable. That split is the whole point of the resource pass.
    assert_eq!(
        lowered.initial.extra.vars.get("main::count"),
        Some(&unipn::cvn::expr::Val::int(0))
    );
    let mutex = lowered.place_named("main::mtx").expect("mutex place");
    assert_eq!(lowered.initial.marking.tokens(mutex), 1);
    assert!(lowered.place_named("main::count").is_none());

    // A thread is a function: `scope` hands tokens to the two functions' start
    // places and takes them back from their ends.
    assert!(lowered.transition_named("main::main@s1#spawn").is_some());
    assert!(lowered.transition_named("main::main@s1#join").is_some());
    assert!(lowered.place_named("main::producer@end").is_some());
}

#[test]
fn the_lowered_condvar_wait_is_recognized_as_a_wait_point() {
    let lowered = cvn_from_concir_json(&example("producer_consumer.json")).unwrap();
    let ending_in = |suffix: &str| -> Vec<_> {
        lowered
            .net
            .place_ids()
            .filter(|&p| lowered.net.place(p).unwrap().name.ends_with(suffix))
            .collect()
    };

    // The lowering marks neither place: both are plain `BasicBlock`s, and the
    // classification comes from the transitions that can carry the token away.
    // If a future lowering reshapes the wait, this is what notices.
    let waiting = ending_in(":waiting");
    let reacquire = ending_in(":reacquire");
    assert_eq!(waiting.len(), 1, "the example has one condvar_wait");
    assert_eq!(reacquire.len(), waiting.len());

    for p in waiting {
        assert!(lowered.net.is_wait_point(p));
    }
    for p in reacquire {
        assert!(!lowered.net.is_wait_point(p));
    }
}

#[test]
fn producer_consumer_has_no_deadlock_and_no_dead_step() {
    let lowered = cvn_from_concir_json(&example("producer_consumer.json")).unwrap();
    let graph = explore(
        &lowered.net,
        lowered.initial.clone(),
        &AnalysisConfig::default(),
    );

    assert!(!graph.truncated, "state space should be finite here");

    // Blocked states exist — the run ends — but none of them strands a thread.
    assert!(!graph.blocked.is_empty());
    let deadlocks = find_deadlocks(&lowered.net, &graph);
    assert!(
        deadlocks.is_empty(),
        "unexpected deadlock, blocked places: {:?}",
        deadlocks
            .iter()
            .map(
                |cex| unipn::analysis::cvn::blocked_places(&lowered.net, &cex.final_state)
                    .iter()
                    .map(|&p| lowered.net.place_label(p))
                    .collect::<Vec<_>>()
            )
            .collect::<Vec<_>>()
    );

    // The producer's whole path runs: if any of its transitions never fired, the
    // lowering wired the thread up wrongly and the empty deadlock list above
    // would mean nothing.
    let never_fired = unfired_transitions(&lowered.net, &graph);
    let producer: Vec<_> = never_fired
        .iter()
        .filter(|t| lowered.transitions_in("main::producer").contains(t))
        .map(|&t| lowered.net.transition_label(t))
        .collect();
    assert!(producer.is_empty(), "producer never ran: {producer:?}");

    // The consumer reaches both arms of its `count > 0` branch, because the
    // interleaving where it looks before the producer writes is real.
    for arm in ["main::consumer@s3#true", "main::consumer@s3#false"] {
        let t = lowered.transition_named(arm).unwrap();
        assert!(!never_fired.contains(&t), "{arm} never fired");
    }

    // Both notification outcomes are explored: delivered when the consumer is
    // already waiting, lost when it is not.
    for outcome in ["main::producer@s3", "main::producer@s3#lost"] {
        let t = lowered.transition_named(outcome).unwrap();
        assert!(!never_fired.contains(&t), "{outcome} never fired");
    }

    // Nothing is behaviorally dead anywhere in the program.
    let dead = find_dead_transitions(&lowered.net, &graph);
    assert!(dead.is_empty(), "dead steps: {dead:?}");
}

#[test]
fn a_returning_function_drops_its_locals_not_the_shared_state() {
    let lowered = cvn_from_concir_json(&example("producer_consumer.json")).unwrap();
    let graph = explore(
        &lowered.net,
        lowered.initial.clone(),
        &AnalysisConfig::default(),
    );

    // `count` is a module resource, so it survives every return; no function in
    // this program declares a modeled local, so there is nothing to drop.
    for &index in &graph.blocked {
        assert!(graph.states[index].extra.vars.contains_key("main::count"));
    }
}

#[test]
fn an_operation_we_do_not_lower_yet_is_an_error_not_a_silent_skip() {
    // Dropping a `channel_send` would delete exactly the blocking behavior the
    // analysis exists to find, so it has to fail loudly.
    let json = r#"{
      "program": "unsupported",
      "modules": [{
        "name": "m",
        "resources": [
          {"name": "tx", "kind": "sync", "type": "Channel", "mode": "Async",
           "base": "Int", "capacity": 1}
        ],
        "functions": [{
          "name": "f",
          "kind": "normal",
          "body": [
            {"sid": "s1", "kind": "channel_send", "channel": "tx", "value": "1"},
            {"sid": "s2", "kind": "return"}
          ]
        }]
      }],
      "entry": "m::f"
    }"#;

    match cvn_from_concir_json(json) {
        Err(TransferError::UnsupportedOp { scope, sid, kind }) => {
            assert_eq!(
                (scope.as_str(), sid.as_str(), kind),
                ("m::f", "s1", "channel_send")
            );
        }
        other => panic!("expected UnsupportedOp, got {other:?}"),
    }
}

#[test]
fn an_unreadable_condition_leaves_both_arms_open() {
    // The parser cannot read `lookup(k) > 0`, so the guard degrades to Unknown
    // and both branches stay reachable. Losing the else-arm here would be the
    // dangerous failure: an unexplored path proves nothing.
    let json = r#"{
      "program": "degraded",
      "modules": [{
        "name": "m",
        "functions": [{
          "name": "f",
          "kind": "normal",
          "body": [
            {"sid": "s1", "kind": "branch", "cond": "lookup(k) > 0", "then": "s2", "else": "s3"},
            {"sid": "s2", "kind": "nop"},
            {"sid": "s3", "kind": "return"}
          ]
        }]
      }],
      "entry": "m::f"
    }"#;

    let lowered = cvn_from_concir_json(json).unwrap();
    assert_eq!(lowered.report.degraded.len(), 1);
    assert_eq!(lowered.report.degraded[0].role, "cond");
    assert_eq!(lowered.report.degraded[0].sid, "s1");

    let graph = explore(
        &lowered.net,
        lowered.initial.clone(),
        &AnalysisConfig::default(),
    );
    let never_fired = unfired_transitions(&lowered.net, &graph);
    for arm in ["m::f@s1#true", "m::f@s1#false"] {
        let t = lowered.transition_named(arm).unwrap();
        assert!(!never_fired.contains(&t), "{arm} was pruned by a guess");
    }
}

#[test]
fn every_bundled_example_parses_even_when_it_cannot_be_lowered() {
    // The lenient AST is the claim under test: a program this crate cannot
    // lower yet must still deserialize, so the failure names the missing
    // operation instead of blaming the file.
    for entry in std::fs::read_dir(examples_dir()).expect("ConcIR submodule checked out") {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|ext| ext != "json") {
            continue;
        }
        let json = std::fs::read_to_string(&path).unwrap();
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        match cvn_from_concir_json(&json) {
            Ok(_) | Err(TransferError::UnsupportedOp { .. }) => {}
            Err(other) => panic!("{name}: {other}"),
        }
    }
}
