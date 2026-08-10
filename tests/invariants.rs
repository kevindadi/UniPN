//! 库位/变迁不变量（feature `invariants`）。

mod common;

#[cfg(feature = "invariants")]
#[test]
fn cycle_has_place_invariant() {
    use num_bigint::BigInt;
    use num_traits::Signed;
    use unipn::analysis::invariants::place_invariants;

    let net = common::cycle();
    let pis = place_invariants(&net);
    let found = pis.iter().any(|v| {
        let abs: Vec<BigInt> = v.iter().map(|x| x.abs()).collect();
        abs == vec![BigInt::from(1), BigInt::from(1)]
    });
    assert!(found, "p0 + p1 = 1 must be a place invariant: {pis:?}");
}

#[cfg(feature = "invariants")]
#[test]
fn cycle_has_transition_invariant() {
    use num_bigint::BigInt;
    use num_traits::Signed;
    use unipn::analysis::invariants::transition_invariants;

    let net = common::cycle();
    let tis = transition_invariants(&net);
    let found = tis.iter().any(|v| {
        let abs: Vec<BigInt> = v.iter().map(|x| x.abs()).collect();
        abs == vec![BigInt::from(1), BigInt::from(1)]
    });
    assert!(found, "t0 + t1 = 0 must be a transition invariant: {tis:?}");
}
