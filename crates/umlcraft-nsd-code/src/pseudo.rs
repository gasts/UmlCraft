//! Code in the notation of structograms: no types, assignments as `x := value`, no `;`, and
//! loops as `for i := a to b by s` and `foreach x in c`. Java and C# share the shapes of the
//! nodes used here (assignments, binary expressions, for loops).

use tree_sitter::Node;

use crate::text::{lines_at, node_text, one_line};

/// `name := value`, the value's further lines as they are.
pub fn assign(name: &str, mut value: Vec<String>) -> Vec<String> {
    match value.first_mut() {
        Some(first) => first.insert_str(0, &format!("{name} := ")),
        None => value.push(format!("{name} :=")),
    }
    value
}

/// An assignment or increment as `x := value`: `x = 1` → `x := 1`, `total += a + b` →
/// `total := total + (a + b)`, `i++` → `i := i + 1`. Other expressions give `None`.
pub fn assignment(expr: Node, src: &str) -> Option<Vec<String>> {
    match expr.kind() {
        "assignment_expression" => {
            let left = node_text(expr.child_by_field_name("left")?, src);
            let op = node_text(expr.child_by_field_name("operator")?, src);
            let right = expr.child_by_field_name("right")?;
            let column = expr.start_position().column;
            let value = match op.strip_suffix('=').filter(|o| !o.is_empty()) {
                None => lines_at(right, src, column),
                Some(op) => {
                    let mut value = operand(right, src, column, precedence(op));
                    value[0].insert_str(0, &format!("{left} {op} "));
                    value
                }
            };
            Some(assign(left, value))
        }
        "update_expression" => {
            let operand = expr.named_child(0)?;
            let text = node_text(expr, src);
            let op = if text.contains("++") { "+" } else { "-" };
            let name = one_line(node_text(operand, src));
            Some(vec![format!("{name} := {name} {op} 1")])
        }
        _ => None,
    }
}

/// `for i := a to b by s` for a loop that counts `var` from `start` by a constant step:
/// `for (int i = 0; i < n; i++)` → `for i := 0 to n - 1`. Other for loops give `None`.
pub fn counting(node: Node, var: Node, start: Node, src: &str) -> Option<String> {
    let var = node_text(var, src);

    let condition = node.child_by_field_name("condition")?;
    let op = node_text(condition.child_by_field_name("operator")?, src);
    let left = condition.child_by_field_name("left")?;
    let bound = condition.child_by_field_name("right")?;
    if condition.kind() != "binary_expression" || node_text(left, src) != var {
        return None;
    }

    let mut cursor = node.walk();
    let updates: Vec<Node> =
        node.children_by_field_name("update", &mut cursor).filter(|n| n.is_named()).collect();
    let [update] = updates.as_slice() else { return None };
    let step = step(*update, var, src)?;

    let end = match (op, step > 0) {
        ("<", true) => offset(bound, src, -1),
        (">", false) => offset(bound, src, 1),
        ("<=", true) | (">=", false) => one_line(node_text(bound, src)),
        _ => return None,
    };
    let start = one_line(node_text(start, src));
    Some(match step {
        1 => format!("for {var} := {start} to {end}"),
        _ => format!("for {var} := {start} to {end} by {step}"),
    })
}

/// `foreach v in values` for `for (T v : values)`.
pub fn traversal(name: &str, mut value: Vec<String>) -> Vec<String> {
    match value.first_mut() {
        Some(first) => first.insert_str(0, &format!("foreach {name} in ")),
        None => value.push(format!("foreach {name} in")),
    }
    value
}

/// The constant a counting loop's update adds to `var`: `i++` 1, `i -= 2` -2.
fn step(update: Node, var: &str, src: &str) -> Option<i64> {
    match update.kind() {
        "update_expression" => {
            let operand = update.named_child(0)?;
            if node_text(operand, src) != var {
                return None;
            }
            Some(if node_text(update, src).contains("++") { 1 } else { -1 })
        }
        "assignment_expression" => {
            if node_text(update.child_by_field_name("left")?, src) != var {
                return None;
            }
            let right = update.child_by_field_name("right")?;
            let n: i64 = node_text(right, src).parse().ok().filter(|&n| n > 0)?;
            match node_text(update.child_by_field_name("operator")?, src) {
                "+=" => Some(n),
                "-=" => Some(-n),
                _ => None,
            }
        }
        _ => None,
    }
}

/// `bound + by` on one line: a number is computed, an expression gets `- 1` or `+ 1`.
fn offset(bound: Node, src: &str, by: i64) -> String {
    if let Ok(n) = node_text(bound, src).parse::<i64>() {
        return (n + by).to_string();
    }
    // As the left operand of `-`/`+`, a sum needs no parentheses.
    let text = operand(bound, src, 0, precedence("+") - 1).join(" ");
    let text = one_line(&text);
    if by < 0 { format!("{text} - {}", -by) } else { format!("{text} + {by}") }
}

/// An expression's lines (see [`lines_at`]) as the operand of an operator of `outer`
/// precedence, in parentheses where it would otherwise bind differently.
fn operand(expr: Node, src: &str, column: usize, outer: u8) -> Vec<String> {
    let inner = match expr.kind() {
        "binary_expression" => {
            expr.child_by_field_name("operator").map_or(0, |o| precedence(node_text(o, src)))
        }
        "instanceof_expression" => precedence("<"),
        "ternary_expression" | "assignment_expression" | "lambda_expression" => 0,
        _ => u8::MAX,
    };
    let mut text = lines_at(expr, src, column);
    if inner <= outer {
        text[0].insert(0, '(');
        if let Some(last) = text.last_mut() {
            last.push(')');
        }
    }
    text
}

/// How tightly a binary operator binds (Java's order).
fn precedence(op: &str) -> u8 {
    match op {
        "*" | "/" | "%" => 12,
        "+" | "-" => 11,
        "<<" | ">>" | ">>>" => 10,
        "<" | ">" | "<=" | ">=" => 9,
        "==" | "!=" => 8,
        "&" => 7,
        "^" => 6,
        "|" => 5,
        "&&" => 4,
        "||" => 3,
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use crate::Source;

    /// The first statement of `body` as the notation gives it.
    fn first(body: &str) -> Vec<String> {
        let source = Source::parse(&format!("class A {{ void f(int n) {{ {body} }} }}"));
        let m = &source.methods()[0];
        let made = crate::Made { author: "t", date: "2026-10-30", origin: "test" };
        let (d, _) = source.diagram(m, &made).unwrap();
        d.children.elements[0].text()
    }

    #[test]
    fn assignments_lose_types_and_semicolons() {
        assert_eq!(first("int y = n + 1;"), ["y := n + 1"]);
        assert_eq!(first("var s = \"x\";"), ["s := \"x\""]);
        assert_eq!(first("n = 2;"), ["n := 2"]);
        assert_eq!(first("n++;"), ["n := n + 1"]);
        assert_eq!(first("--n;"), ["n := n - 1"]);
        assert_eq!(first("n += 3;"), ["n := n + 3"]);
        assert_eq!(first("n -= a - b;"), ["n := n - (a - b)"]);
        assert_eq!(first("n *= a + b;"), ["n := n * (a + b)"]);
        assert_eq!(first("n += a * b;"), ["n := n + a * b"]);
        assert_eq!(first("a[n] <<= 1;"), ["a[n] := a[n] << 1"]);
        assert_eq!(first("list.clear();"), ["list.clear()"]);
    }

    #[test]
    fn values_over_several_lines_keep_their_indentation() {
        let body =
            "\n        Runnable r = new Runnable() {\n            void run() { }\n        };\n";
        assert_eq!(first(body), ["r := new Runnable() {", "    void run() { }", "}"]);
    }

    #[test]
    fn declarations_without_a_value_vanish() {
        assert_eq!(first("int x; int a = 1, b, c = 2;"), ["a := 1"]);
    }

    #[test]
    fn counting_loops() {
        assert_eq!(first("for (int i = 0; i < n; i++) { }"), ["for i := 0 to n - 1"]);
        assert_eq!(first("for (int i = 0; i < 10; i++) { }"), ["for i := 0 to 9"]);
        assert_eq!(first("for (int i = 0; i < n + 1; i++) { }"), ["for i := 0 to n + 1 - 1"]);
        assert_eq!(first("for (int i = 1; i <= n; i += 2) { }"), ["for i := 1 to n by 2"]);
        assert_eq!(first("for (int i = n; i > 0; i--) { }"), ["for i := n to 1 by -1"]);
        assert_eq!(first("for (i = n; i >= a; i -= 3) { }"), ["for i := n to a by -3"]);
        assert_eq!(first("for (int i = 0; i < n << 1; i++) { }"), ["for i := 0 to (n << 1) - 1"]);
        assert_eq!(first("for (int v : values) { }"), ["foreach v in values"]);
    }

    #[test]
    fn other_for_loops_become_while_loops() {
        // Wrong direction, a step that is no constant, two variables.
        assert_eq!(first("for (int i = 0; i < n; i--) { }"), ["i := 0"]);
        assert_eq!(first("for (int i = 0; i < n; i += k) { }"), ["i := 0"]);
        assert_eq!(first("for (int i = 0, j = n; i < j; i++, j--) { }"), ["i := 0"]);
    }

    #[test]
    fn signatures_name_only_the_parameters() {
        let source = Source::parse(
            "class A { <T> int f(final java.util.List<T> xs, int[] a, String... rest) { return 0; }
                       void g(A this, int x) { } record P(int x) { P { } } }",
        );
        let made = crate::Made { author: "t", date: "2026-10-30", origin: "test" };
        let texts: Vec<Vec<String>> =
            source.methods().iter().map(|m| source.diagram(m, &made).unwrap().0.text()).collect();
        assert_eq!(texts, [vec!["f(xs, a, rest)"], vec!["g(x)"], vec!["P"]]);
    }
}
