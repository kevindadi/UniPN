//! ConcIR expression strings into the CVN's [`Expr`] / [`BoolExpr`] trees.
//!
//! ConcIR keeps `expr` and `cond` as opaque source strings, so something has to
//! guess at their structure. This parser is deliberately small: it recognizes
//! literals, bare identifiers, one level of arithmetic, comparisons, and
//! unparenthesized `!` / `&&` / `||`. Anything else **degrades** rather than
//! failing.
//!
//! Degrading is safe in one specific direction. The CVN evaluates guards over
//! three values and treats `Unknown` as satisfied, so a degraded guard leaves
//! *both* arms of a branch enabled and the explored behavior is a superset of
//! the real one — no run is lost, some impossible runs are added. That is why a
//! degraded guard must be an `Unknown`-valued comparison and never
//! [`BoolExpr::True`]: `Not(True)` is `False`, which would silently delete the
//! else-arm.
//!
//! Every degradation is reported (see
//! [`LoweringReport::degraded`](crate::LoweringReport::degraded)), because
//! losing precision quietly is the failure mode that makes a verifier useless.

use unipn::cvn::expr::{BoolExpr, CmpOp, Expr, Op, Val};

/// A parse result: the tree, and whether precision was lost getting there.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Parsed<T> {
    pub tree: T,
    pub degraded: bool,
}

impl<T> Parsed<T> {
    fn exact(tree: T) -> Self {
        Self {
            tree,
            degraded: false,
        }
    }

    fn degraded(tree: T) -> Self {
        Self {
            tree,
            degraded: true,
        }
    }
}

/// A value expression whose value is unknown — the absorbing element of the
/// store's value lattice.
pub fn unknown_expr() -> Expr {
    Expr::Lit(Val::Unknown)
}

/// A guard that evaluates to `Unknown`, and therefore lets both a branch and its
/// negation fire.
///
/// [`BoolExpr::True`] would be wrong here: the else-arm carries `Not(guard)`, and
/// `Not(True)` is `False`, so an unparsed condition would disable one arm
/// instead of leaving both open.
pub fn unknown_guard() -> BoolExpr {
    BoolExpr::Cmp {
        op: CmpOp::Eq,
        lhs: Box::new(unknown_expr()),
        rhs: Box::new(unknown_expr()),
    }
}

/// Parse a value expression: a literal, an identifier, or `<atom> <op> <atom>`.
pub fn parse_expr(src: &str) -> Parsed<Expr> {
    let src = src.trim();
    let Some(tokens) = tokenize(src) else {
        return Parsed::degraded(unknown_expr());
    };
    match parse_arith(&tokens) {
        Some(tree) => Parsed::exact(tree),
        None => Parsed::degraded(unknown_expr()),
    }
}

/// Parse a condition: comparisons joined by unparenthesized `&&` / `||`, with
/// `!` in front of any of them. A bare identifier reads as `<ident> == true`.
pub fn parse_guard(src: &str) -> Parsed<BoolExpr> {
    let src = src.trim();
    if src.contains('(') || src.contains(')') {
        // Precedence with parentheses needs a real parser; over-approximate.
        return Parsed::degraded(unknown_guard());
    }
    match parse_bool(src) {
        Some(tree) => Parsed::exact(tree),
        None => Parsed::degraded(unknown_guard()),
    }
}

// ── Booleans ──

fn parse_bool(src: &str) -> Option<BoolExpr> {
    let src = src.trim();
    if src.is_empty() {
        return None;
    }

    // Lowest precedence first, so the split lands at the top of the tree.
    if let Some((lhs, rhs)) = split_once_outside(src, "||") {
        return Some(BoolExpr::Or(
            Box::new(parse_bool(lhs)?),
            Box::new(parse_bool(rhs)?),
        ));
    }
    if let Some((lhs, rhs)) = split_once_outside(src, "&&") {
        return Some(BoolExpr::And(
            Box::new(parse_bool(lhs)?),
            Box::new(parse_bool(rhs)?),
        ));
    }
    if let Some(rest) = src.strip_prefix('!') {
        return Some(BoolExpr::Not(Box::new(parse_bool(rest)?)));
    }

    parse_comparison(src)
}

fn parse_comparison(src: &str) -> Option<BoolExpr> {
    // Two-character operators first, so `<=` never reads as `<`.
    for (text, op) in [
        ("==", CmpOp::Eq),
        ("!=", CmpOp::Ne),
        ("<=", CmpOp::Le),
        (">=", CmpOp::Ge),
        ("<", CmpOp::Lt),
        (">", CmpOp::Gt),
    ] {
        if let Some((lhs, rhs)) = src.split_once(text) {
            let lhs = parse_arith(&tokenize(lhs.trim())?)?;
            let rhs = parse_arith(&tokenize(rhs.trim())?)?;
            return Some(BoolExpr::Cmp {
                op,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
            });
        }
    }

    // A bare boolean: `flag` means `flag == true`.
    let tokens = tokenize(src)?;
    let [Token::Ident(name)] = tokens.as_slice() else {
        return None;
    };
    Some(BoolExpr::Cmp {
        op: CmpOp::Eq,
        lhs: Box::new(Expr::Ref(name.clone())),
        rhs: Box::new(Expr::Lit(Val::bool(true))),
    })
}

/// Split on the first occurrence of `sep` that is not part of a longer operator.
/// There are no parentheses to track — [`parse_guard`] rejects those up front.
fn split_once_outside<'a>(src: &'a str, sep: &str) -> Option<(&'a str, &'a str)> {
    let at = src.find(sep)?;
    Some((&src[..at], &src[at + sep.len()..]))
}

// ── Arithmetic ──

fn parse_arith(tokens: &[Token]) -> Option<Expr> {
    match tokens {
        [atom] => atom.as_expr(),
        [lhs, Token::Sym(sym), rhs] => Some(Expr::BinOp {
            op: arith_op(*sym)?,
            lhs: Box::new(lhs.as_expr()?),
            rhs: Box::new(rhs.as_expr()?),
        }),
        _ => None,
    }
}

fn arith_op(sym: char) -> Option<Op> {
    match sym {
        '+' => Some(Op::Add),
        '-' => Some(Op::Sub),
        '*' => Some(Op::Mul),
        '/' => Some(Op::Div),
        '%' => Some(Op::Mod),
        _ => None,
    }
}

// ── Tokens ──

#[derive(Clone, Debug, PartialEq)]
enum Token {
    Ident(String),
    Lit(Val),
    Sym(char),
}

impl Token {
    fn as_expr(&self) -> Option<Expr> {
        match self {
            Self::Ident(name) => Some(Expr::Ref(name.clone())),
            Self::Lit(value) => Some(Expr::Lit(value.clone())),
            Self::Sym(_) => None,
        }
    }
}

/// Split into identifiers, literals, and arithmetic symbols. `None` on any
/// character the caller should not pretend to understand.
///
/// A `-` directly in front of a number and not preceded by an operand is folded
/// into the literal, so `-1` is one token and `count - 1` is three.
fn tokenize(src: &str) -> Option<Vec<Token>> {
    let mut tokens: Vec<Token> = Vec::new();
    let chars: Vec<char> = src.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            i += 1;
        } else if c == '"' {
            let end = chars[i + 1..].iter().position(|&c| c == '"')? + i + 1;
            tokens.push(Token::Lit(Val::string(
                chars[i + 1..end].iter().collect::<String>(),
            )));
            i = end + 1;
        } else if c.is_ascii_digit()
            || (c == '-'
                && !matches!(tokens.last(), Some(Token::Ident(_) | Token::Lit(_)))
                && chars.get(i + 1).is_some_and(char::is_ascii_digit))
        {
            let start = i;
            i += 1;
            while chars
                .get(i)
                .is_some_and(|c| c.is_ascii_digit() || *c == '.')
            {
                i += 1;
            }
            let text: String = chars[start..i].iter().collect();
            tokens.push(Token::Lit(number(&text)?));
        } else if c.is_alphabetic() || c == '_' {
            let start = i;
            while chars
                .get(i)
                .is_some_and(|c| c.is_alphanumeric() || *c == '_' || *c == '.' || *c == ':')
            {
                i += 1;
            }
            let text: String = chars[start..i].iter().collect();
            tokens.push(keyword_or_ident(text));
        } else if matches!(c, '+' | '-' | '*' | '/' | '%') {
            tokens.push(Token::Sym(c));
            i += 1;
        } else {
            return None;
        }
    }

    (!tokens.is_empty()).then_some(tokens)
}

fn number(text: &str) -> Option<Val> {
    if text.contains('.') {
        text.parse::<f64>().ok().map(Val::float)
    } else {
        text.parse::<i64>().ok().map(Val::int)
    }
}

/// `true` / `false` are literals; everything else is a store reference. An
/// unquoted `Ready`-style enum variant is indistinguishable from a variable
/// name here, so it stays a reference and evaluates to `Unknown` if absent —
/// which is again the safe direction.
fn keyword_or_ident(text: String) -> Token {
    match text.as_str() {
        "true" => Token::Lit(Val::bool(true)),
        "false" => Token::Lit(Val::bool(false)),
        _ => Token::Ident(text),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn int(i: i64) -> Box<Expr> {
        Box::new(Expr::Lit(Val::int(i)))
    }

    fn r(name: &str) -> Box<Expr> {
        Box::new(Expr::Ref(name.into()))
    }

    #[test]
    fn parses_the_shapes_concir_actually_writes() {
        assert_eq!(
            parse_expr("count + 1"),
            Parsed::exact(Expr::BinOp {
                op: Op::Add,
                lhs: r("count"),
                rhs: int(1),
            })
        );
        assert_eq!(
            parse_expr("count - 1"),
            Parsed::exact(Expr::BinOp {
                op: Op::Sub,
                lhs: r("count"),
                rhs: int(1),
            })
        );
        assert_eq!(parse_expr("0"), Parsed::exact(Expr::Lit(Val::int(0))));
        assert_eq!(parse_expr("-3"), Parsed::exact(Expr::Lit(Val::int(-3))));
        assert_eq!(parse_expr("ret"), Parsed::exact(Expr::Ref("ret".into())));

        assert_eq!(
            parse_guard("count > 0"),
            Parsed::exact(BoolExpr::Cmp {
                op: CmpOp::Gt,
                lhs: r("count"),
                rhs: int(0),
            })
        );
        assert_eq!(
            parse_guard("flag == true"),
            Parsed::exact(BoolExpr::Cmp {
                op: CmpOp::Eq,
                lhs: r("flag"),
                rhs: Box::new(Expr::Lit(Val::bool(true))),
            })
        );
        assert_eq!(
            parse_guard("flag"),
            Parsed::exact(BoolExpr::Cmp {
                op: CmpOp::Eq,
                lhs: r("flag"),
                rhs: Box::new(Expr::Lit(Val::bool(true))),
            })
        );
    }

    #[test]
    fn a_condition_it_cannot_read_leaves_both_arms_open() {
        // Not `True`: the else-arm is `Not(guard)`, and `Not(True)` would delete
        // it. `Unknown` keeps both.
        let parsed = parse_guard("compute(x) && y[0]");
        assert!(parsed.degraded);

        let store = Default::default();
        let guard = parsed.tree;
        assert!(unipn::cvn::expr::eval_guard(&guard, &store).is_not_false());
        assert!(
            unipn::cvn::expr::eval_guard(&BoolExpr::Not(Box::new(guard)), &store).is_not_false()
        );
    }

    #[test]
    fn nesting_beyond_one_level_degrades_rather_than_guessing() {
        assert!(parse_expr("a + b + c").degraded);
        assert!(parse_expr("(a + b) * c").degraded);
        assert!(parse_guard("(a || b) && c").degraded);
    }

    #[test]
    fn conjunctions_without_parentheses_stay_exact() {
        let parsed = parse_guard("count > 0 && flag == false");
        assert!(!parsed.degraded);
        assert!(matches!(parsed.tree, BoolExpr::And(_, _)));

        let parsed = parse_guard("!done");
        assert!(!parsed.degraded);
        assert!(matches!(parsed.tree, BoolExpr::Not(_)));
    }
}
