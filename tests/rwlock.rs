//! Test 5: RwLock read/write mutual exclusion.
//!
//! RwLock with N=3:
//! - Readers consume 1 token each (can run concurrently up to N)
//! - Writer consumes all N tokens (exclusive access)
//! - No deadlock expected.

use cvn::analysis::{explore, AnalysisConfig};
use cvn::builder::CvnNetBuilder;
use cvn::model::*;

fn build_rwlock_net() -> cvn::net::CvnNet {
    let n = 3u32;

    CvnNetBuilder::new()
        // Resource: RwLock with N=3
        .add_resource_place("rw", "rw", ResourceType::RwLock { max_readers: n })
        // main
        .add_control_place("main_start", "main", "ms0")
        .add_control_place("main_s1", "main", "ms1")
        .add_control_place("main_s2", "main", "ms2")
        .add_control_place("main_s3", "main", "ms3")
        .add_control_place("main_s4", "main", "ms4")
        .add_control_place("main_done", "main", "ms5")
        .set_return("main_done")
        // reader1
        .add_control_place("r1_start", "reader1", "r1s0")
        .add_control_place("r1_reading", "reader1", "r1s1")
        .add_control_place("r1_done", "reader1", "r1s2")
        .set_return("r1_done")
        // reader2
        .add_control_place("r2_start", "reader2", "r2s0")
        .add_control_place("r2_reading", "reader2", "r2s1")
        .add_control_place("r2_done", "reader2", "r2s2")
        .set_return("r2_done")
        // writer
        .add_control_place("wr_start", "writer", "ws0")
        .add_control_place("wr_writing", "writer", "ws1")
        .add_control_place("wr_done", "writer", "ws2")
        .set_return("wr_done")
        // main: spawn reader1, reader2, writer then join all
        .add_transition("t_spawn_r1", TransitionKind::Spawn, &["ms0"])
        .add_input_arc("main_start", "t_spawn_r1", 1, BoolExpr::True)
        .add_output_arc("t_spawn_r1", "main_s1", 1, None)
        .add_output_arc("t_spawn_r1", "r1_start", 1, None)
        .add_transition("t_spawn_r2", TransitionKind::Spawn, &["ms1"])
        .add_input_arc("main_s1", "t_spawn_r2", 1, BoolExpr::True)
        .add_output_arc("t_spawn_r2", "main_s2", 1, None)
        .add_output_arc("t_spawn_r2", "r2_start", 1, None)
        .add_transition("t_spawn_wr", TransitionKind::Spawn, &["ms2"])
        .add_input_arc("main_s2", "t_spawn_wr", 1, BoolExpr::True)
        .add_output_arc("t_spawn_wr", "main_s3", 1, None)
        .add_output_arc("t_spawn_wr", "wr_start", 1, None)
        .add_transition("t_join_r1", TransitionKind::Join, &["ms3"])
        .add_input_arc("main_s3", "t_join_r1", 1, BoolExpr::True)
        .add_input_arc("r1_done", "t_join_r1", 1, BoolExpr::True)
        .add_output_arc("t_join_r1", "main_s4", 1, None)
        .add_transition("t_join_r2", TransitionKind::Join, &["ms4"])
        .add_input_arc("main_s4", "t_join_r2", 1, BoolExpr::True)
        .add_input_arc("r2_done", "t_join_r2", 1, BoolExpr::True)
        .add_output_arc("t_join_r2", "main_done", 1, None)
        // Note: we join r1+r2 but not writer separately. Writer just runs.
        // Actually let's join writer too to keep it clean.
        // ... Simplification: writer auto-completes, main waits on readers only.
        // The writer return place is still terminal so is_terminal works.
        // reader1: read(rw) → drop_read(rw)
        .add_transition("t_r1_read", TransitionKind::Lock, &["r1s0"])
        .add_input_arc("r1_start", "t_r1_read", 1, BoolExpr::True)
        .add_input_arc("rw", "t_r1_read", 1, BoolExpr::True) // consume 1 token
        .add_output_arc("t_r1_read", "r1_reading", 1, None)
        .add_transition("t_r1_drop", TransitionKind::Unlock, &["r1s1"])
        .add_input_arc("r1_reading", "t_r1_drop", 1, BoolExpr::True)
        .add_output_arc("t_r1_drop", "r1_done", 1, None)
        .add_output_arc("t_r1_drop", "rw", 1, None) // return 1 token
        // reader2: read(rw) → drop_read(rw)
        .add_transition("t_r2_read", TransitionKind::Lock, &["r2s0"])
        .add_input_arc("r2_start", "t_r2_read", 1, BoolExpr::True)
        .add_input_arc("rw", "t_r2_read", 1, BoolExpr::True) // consume 1 token
        .add_output_arc("t_r2_read", "r2_reading", 1, None)
        .add_transition("t_r2_drop", TransitionKind::Unlock, &["r2s1"])
        .add_input_arc("r2_reading", "t_r2_drop", 1, BoolExpr::True)
        .add_output_arc("t_r2_drop", "r2_done", 1, None)
        .add_output_arc("t_r2_drop", "rw", 1, None) // return 1 token
        // writer: lock(rw) consumes ALL N=3 tokens → drop_write returns N tokens
        .add_transition("t_wr_lock", TransitionKind::Lock, &["ws0"])
        .add_input_arc("wr_start", "t_wr_lock", 1, BoolExpr::True)
        .add_input_arc("rw", "t_wr_lock", n, BoolExpr::True) // consume ALL N tokens
        .add_output_arc("t_wr_lock", "wr_writing", 1, None)
        .add_transition("t_wr_drop", TransitionKind::Unlock, &["ws1"])
        .add_input_arc("wr_writing", "t_wr_drop", 1, BoolExpr::True)
        .add_output_arc("t_wr_drop", "wr_done", 1, None)
        .add_output_arc("t_wr_drop", "rw", n, None) // return ALL N tokens
        // Initial tokens
        .set_initial_tokens("main_start", 1)
        .set_initial_tokens("rw", n)
        .build()
        .expect("valid rwlock net")
}

#[test]
fn rwlock_no_deadlock() {
    let net = build_rwlock_net();
    let result = explore(&net, &AnalysisConfig::default()).unwrap();
    assert!(
        result.deadlocks.is_empty(),
        "RwLock scenario should not deadlock, found {} deadlocks",
        result.deadlocks.len()
    );
}

#[test]
fn readers_can_run_concurrently() {
    let net = build_rwlock_net();
    let result = explore(&net, &AnalysisConfig::default()).unwrap();

    // There should be a reachable state where both readers are in their "reading" places
    let both_reading = result
        .reachability_graph
        .node_indices()
        .any(|idx| {
            let state = &result.reachability_graph[idx];
            state.tokens(&PlaceId::new("r1_reading")) >= 1
                && state.tokens(&PlaceId::new("r2_reading")) >= 1
        });

    assert!(
        both_reading,
        "there should be a state where both readers are reading concurrently"
    );
}

#[test]
fn writer_excludes_readers() {
    let net = build_rwlock_net();
    let result = explore(&net, &AnalysisConfig::default()).unwrap();

    // No reachable state should have the writer writing AND a reader reading
    let writer_with_reader = result
        .reachability_graph
        .node_indices()
        .any(|idx| {
            let state = &result.reachability_graph[idx];
            state.tokens(&PlaceId::new("wr_writing")) >= 1
                && (state.tokens(&PlaceId::new("r1_reading")) >= 1
                    || state.tokens(&PlaceId::new("r2_reading")) >= 1)
        });

    assert!(
        !writer_with_reader,
        "writer and readers should be mutually exclusive"
    );
}
