//! A method body as structogram elements (the mapping of `docs/nsd-spec.md` § 4).

use tree_sitter::Node;
use umlcraft_nsd::edit::new_element;
use umlcraft_nsd::{Color, Diagram, Element, ElementKind, Provenance, RootKind, Sequence};

use crate::methods::signature;
use crate::text::{block_comment, comment, condition, lines, node_text, without_semicolon};
use crate::{Made, Note, NoteKind, Report};

/// Structorizer's colour for what its import could not express.
const RED: Color = Color([0xff, 0x00, 0x00]);

/// Statements that end a `switch` case.
const ENDS_CASE: [&str; 5] = [
    "break_statement",
    "return_statement",
    "throw_statement",
    "continue_statement",
    "yield_statement",
];

const LOCAL_TYPES: [&str; 4] =
    ["class_declaration", "record_declaration", "enum_declaration", "interface_declaration"];

pub fn diagram(method: Node, src: &str, made: &Made) -> (Diagram, Report) {
    let provenance = Provenance { author: made.author, date: made.date, origin: made.origin };
    let mut d = Diagram::new(RootKind::Sub, &[signature(method, src)], &provenance);
    if let Some(doc) = javadoc(method, src) {
        d.set_comment(&doc);
    }
    let mut b = Builder { src, header: d.clone(), notes: Vec::new(), sinks: Vec::new() };
    let body = method.child_by_field_name("body").map(|n| b.block(n)).unwrap_or_default();
    d.children = Sequence::new(body);
    (d, Report { notes: b.notes })
}

/// The `/** … */` right before a declaration (before its annotations too).
fn javadoc(method: Node, src: &str) -> Option<Vec<String>> {
    let prev = method.prev_sibling()?;
    let text = node_text(prev, src);
    (prev.kind() == "block_comment" && text.starts_with("/**")).then(|| block_comment(text))
}

/// Where the values of a `switch` expression go.
#[derive(Clone)]
enum Sink {
    /// `x = switch …`, `T x = switch …`: each value is assigned.
    Assign(String),
    /// `return switch …`.
    Return,
}

struct Builder<'a> {
    src: &'a str,
    /// The diagram's header, for the keywords for loops are read with.
    header: Diagram,
    notes: Vec<Note>,
    /// The switch expressions being built, innermost last (for `yield`).
    sinks: Vec<Sink>,
}

impl Builder<'_> {
    fn note(&mut self, node: Node, kind: NoteKind) {
        self.notes.push(Note { line: node.start_position().row + 1, kind });
    }

    fn element(&self, kind: ElementKind, text: &[String]) -> Element {
        new_element(&self.header, kind, text)
    }

    fn instruction(&self, node: Node) -> Element {
        self.element(ElementKind::Instruction, &lines(node, self.src))
    }

    /// A stand-in for what cannot be shown: red and disabled, as Structorizer marks it.
    fn stand_in(&self, node: Node) -> Element {
        let mut e = self.instruction(node);
        e.set_color(RED);
        e.set_disabled(true);
        e
    }

    /// A block's statements, or a single statement where a block could be.
    fn block(&mut self, node: Node) -> Vec<Element> {
        if matches!(node.kind(), "block" | "constructor_body" | "switch_block_statement_group") {
            let mut cursor = node.walk();
            let children: Vec<Node> = node.named_children(&mut cursor).collect();
            self.statements(&children)
        } else {
            self.statements(&[node])
        }
    }

    /// Statements with their comments: a comment goes to the element made of the next
    /// statement; one on the line a statement ends on goes to that statement's element; at
    /// the end of a block to its last element.
    fn statements(&mut self, nodes: &[Node]) -> Vec<Element> {
        let mut out: Vec<Element> = Vec::new();
        let mut pending: Vec<String> = Vec::new();
        let mut last_row: Option<(usize, usize)> = None;
        for &node in nodes {
            if matches!(node.kind(), "line_comment" | "block_comment") {
                let text = comment(node, self.src);
                match last_row {
                    Some((row, at)) if row == node.start_position().row && at < out.len() => {
                        append_comment(&mut out[at], &text);
                    }
                    _ => pending.extend(text),
                }
                continue;
            }
            if node.kind() == "switch_label" {
                continue;
            }
            let before = out.len();
            self.statement(node, &mut out);
            if out.len() > before {
                if !pending.is_empty() {
                    append_comment(&mut out[before], &std::mem::take(&mut pending));
                }
                last_row = Some((node.end_position().row, out.len() - 1));
            }
        }
        if !pending.is_empty()
            && let Some(last) = out.last_mut()
        {
            append_comment(last, &pending);
        }
        out
    }

    fn statement(&mut self, node: Node, out: &mut Vec<Element>) {
        let src = self.src;
        if node.is_error() || node.is_missing() {
            self.note(node, NoteKind::SyntaxError);
            out.push(self.stand_in(node));
            return;
        }
        match node.kind() {
            "block" => out.extend(self.block(node)),
            ";" | "empty_statement" => {}
            "local_variable_declaration" => {
                if let Some((name, switch)) = declared_switch(node, src) {
                    let declaration = format!("{} {name};", type_text(node, src));
                    out.push(self.element(ElementKind::Instruction, &[declaration]));
                    out.push(self.switch(switch, Some(Sink::Assign(name))));
                } else {
                    out.push(self.instruction(node));
                }
            }
            "expression_statement" => out.push(self.expression_statement(node)),
            "switch_expression" => out.push(self.switch(node, None)),
            "if_statement" => out.push(self.if_statement(node)),
            "for_statement" => self.for_statement(node, out),
            "enhanced_for_statement" => {
                let mut e = self.element(ElementKind::For, &self.head(node));
                e.branches[0] = self.body(node);
                out.push(e);
            }
            "while_statement" => {
                let cond = node.child_by_field_name("condition");
                let endless = cond.is_some_and(|c| condition(c, src) == ["true"]);
                let mut e = if endless {
                    self.element(ElementKind::Forever, &[])
                } else {
                    let text = cond.map(|c| condition(c, src)).unwrap_or_default();
                    self.element(ElementKind::While, &text)
                };
                e.branches[0] = self.body(node);
                out.push(e);
            }
            "do_statement" => {
                let cond = node.child_by_field_name("condition").map(|c| condition(c, src));
                let mut text = cond.unwrap_or_default();
                if let Some(first) = text.first_mut() {
                    first.insert_str(0, "!(");
                }
                if let Some(last) = text.last_mut() {
                    last.push(')');
                }
                let mut e = self.element(ElementKind::Repeat, &text);
                e.branches[0] = self.body(node);
                out.push(e);
            }
            "return_statement"
                if node.named_child(0).is_some_and(|v| v.kind() == "switch_expression") =>
            {
                let switch = node.named_child(0).expect("checked above");
                out.push(self.switch(switch, Some(Sink::Return)));
            }
            "return_statement" | "throw_statement" | "break_statement" => {
                out.push(self.element(ElementKind::Jump, &without_semicolon(lines(node, src))));
            }
            "yield_statement" => out.push(self.yielded(node)),
            "continue_statement" => {
                self.note(node, NoteKind::Continue);
                out.push(self.stand_in(node));
            }
            "labeled_statement" => {
                self.note(node, NoteKind::Label);
                let mut cursor = node.walk();
                let inner: Vec<Node> =
                    node.named_children(&mut cursor).filter(|c| c.kind() != "identifier").collect();
                out.extend(self.statements(&inner));
            }
            "try_statement" | "try_with_resources_statement" => out.push(self.try_statement(node)),
            "synchronized_statement" => {
                self.note(node, NoteKind::Synchronized);
                let lock = node.child_by_field_name("lock").or_else(|| node.named_child(0));
                let mut head = String::from("synchronized ");
                head.push_str(&lock.map(|l| node_text(l, src).to_owned()).unwrap_or_default());
                let body = node.child_by_field_name("body").map(|b| self.block(b));
                let mut body = body.unwrap_or_default();
                if let Some(first) = body.first_mut() {
                    let mut text = vec![head];
                    text.extend(first.comment());
                    first.set_comment(&text);
                }
                out.extend(body);
            }
            kind if LOCAL_TYPES.contains(&kind) => self.note(node, NoteKind::LocalClass),
            _ => {
                if node.has_error() {
                    self.note(node, NoteKind::SyntaxError);
                    out.push(self.stand_in(node));
                } else {
                    out.push(self.instruction(node));
                }
            }
        }
    }

    /// Calls of the class's own methods (`f(x);`, `this.f(x);`) become call elements; a
    /// `switch` assigned to a variable becomes a case element.
    fn expression_statement(&mut self, node: Node) -> Element {
        let src = self.src;
        let Some(expr) = node.named_child(0) else { return self.instruction(node) };
        match expr.kind() {
            "method_invocation" => {
                let object = expr.child_by_field_name("object");
                let own = object.is_none_or(|o| o.kind() == "this");
                if own {
                    return self.element(ElementKind::Call, &lines(node, src));
                }
            }
            "assignment_expression" => {
                let op = expr.child_by_field_name("operator").map(|o| node_text(o, src));
                if let (Some("="), Some(left), Some(right)) =
                    (op, expr.child_by_field_name("left"), expr.child_by_field_name("right"))
                    && right.kind() == "switch_expression"
                {
                    let sink = Sink::Assign(node_text(left, src).to_owned());
                    return self.switch(right, Some(sink));
                }
            }
            "switch_expression" => return self.switch(expr, None),
            _ => {}
        }
        if node.has_error() {
            self.note(node, NoteKind::SyntaxError);
            return self.stand_in(node);
        }
        self.instruction(node)
    }

    fn if_statement(&mut self, node: Node) -> Element {
        let text = node
            .child_by_field_name("condition")
            .map(|c| condition(c, self.src))
            .unwrap_or_default();
        let mut e = self.element(ElementKind::Alternative, &text);
        if let Some(then) = node.child_by_field_name("consequence") {
            e.branches[0] = Sequence::new(self.block(then));
        }
        if let Some(otherwise) = node.child_by_field_name("alternative") {
            e.branches[1] = Sequence::new(self.block(otherwise));
        }
        e
    }

    /// A loop's head as written, up to its body: `for (int i = 0; i < n; i++)`.
    fn head(&self, node: Node) -> Vec<String> {
        let end = node.child_by_field_name("body").map_or(node.end_byte(), |b| b.start_byte());
        let text = self.src[node.start_byte()..end].trim_end();
        crate::text::lines_of(text, node.start_position().column)
    }

    fn body(&mut self, node: Node) -> Sequence {
        Sequence::new(node.child_by_field_name("body").map(|b| self.block(b)).unwrap_or_default())
    }

    /// A for loop with a condition is a for element with its head as text; without one it is
    /// endless: its initialization goes before it, its update at the end of its body.
    fn for_statement(&mut self, node: Node, out: &mut Vec<Element>) {
        let src = self.src;
        if node.child_by_field_name("condition").is_some() {
            let mut e = self.element(ElementKind::For, &self.head(node));
            e.branches[0] = self.body(node);
            out.push(e);
            return;
        }
        let mut cursor = node.walk();
        for init in node.children_by_field_name("init", &mut cursor) {
            let mut text = lines(init, src);
            if !text.last().is_some_and(|l| l.ends_with(';'))
                && let Some(last) = text.last_mut()
            {
                last.push(';');
            }
            out.push(self.element(ElementKind::Instruction, &text));
        }
        let mut e = self.element(ElementKind::Forever, &[]);
        let mut body = node.child_by_field_name("body").map(|b| self.block(b)).unwrap_or_default();
        let mut cursor = node.walk();
        for update in node.children_by_field_name("update", &mut cursor) {
            let mut text = lines(update, src);
            if let Some(last) = text.last_mut() {
                last.push(';');
            }
            body.push(self.element(ElementKind::Instruction, &text));
        }
        e.branches[0] = Sequence::new(body);
        out.push(e);
    }

    /// `yield v;`: the value goes where the switch expression's value goes.
    fn yielded(&mut self, node: Node) -> Element {
        let value = node.named_child(0).map(|v| lines(v, self.src)).unwrap_or_default();
        self.valued(value, node)
    }

    fn valued(&mut self, mut value: Vec<String>, node: Node) -> Element {
        match self.sinks.last().cloned() {
            Some(Sink::Assign(name)) => {
                if let Some(first) = value.first_mut() {
                    first.insert_str(0, &format!("{name} = "));
                }
                if let Some(last) = value.last_mut() {
                    last.push(';');
                }
                self.element(ElementKind::Instruction, &value)
            }
            Some(Sink::Return) => {
                if let Some(first) = value.first_mut() {
                    first.insert_str(0, "return ");
                }
                self.element(ElementKind::Jump, &value)
            }
            None => self.instruction(node),
        }
    }

    /// A switch statement or expression as a case element: one branch per case (labels of a
    /// shared body joined with `, `), the default last (`%` when there is none). A case that
    /// runs on gets the following cases' statements copied, as Structorizer does.
    fn switch(&mut self, node: Node, sink: Option<Sink>) -> Element {
        let src = self.src;
        let discriminator = node
            .child_by_field_name("condition")
            .map(|c| condition(c, src).join(" "))
            .unwrap_or_default();
        let mut groups: Vec<Group> = Vec::new();
        if let Some(body) = node.child_by_field_name("body") {
            let mut cursor = body.walk();
            for child in body.named_children(&mut cursor) {
                match child.kind() {
                    "switch_block_statement_group" | "switch_rule" => {
                        groups.push(Group::of(child, src));
                    }
                    _ => {}
                }
            }
        }
        // An empty group (`case 2:` right before `case 3:`) shares the next one's body.
        let mut merged: Vec<Group> = Vec::new();
        let mut carried: Option<Group> = None;
        for mut g in groups {
            if let Some(c) = carried.take() {
                let mut labels = c.labels;
                labels.extend(g.labels);
                g.labels = labels;
                g.default |= c.default;
            }
            if g.statements.is_empty() && !g.rule {
                carried = Some(g);
            } else {
                merged.push(g);
            }
        }
        if let Some(c) = carried {
            merged.push(c);
        }
        // What runs for each case: its statements and, while they don't end the case, the
        // next case's.
        let mut bodies: Vec<Vec<Node>> = Vec::new();
        for (i, g) in merged.iter().enumerate() {
            let mut body = g.statements.clone();
            let mut j = i;
            while !g.rule && !ends_case(&body) && j + 1 < merged.len() {
                j += 1;
                body.extend(merged[j].statements.iter().copied());
            }
            if j > i {
                self.note(g.node, NoteKind::Fallthrough);
            }
            // The last unlabelled `break` only ends the case.
            if body
                .last()
                .is_some_and(|s| s.kind() == "break_statement" && s.named_child(0).is_none())
            {
                body.pop();
            }
            bodies.push(body);
        }
        let default = merged.iter().position(|g| g.default);
        let mut order: Vec<usize> = (0..merged.len()).filter(|&i| Some(i) != default).collect();
        let mut text = vec![discriminator];
        text.extend(order.iter().map(|&i| merged[i].labels.join(", ")));
        match default {
            Some(i) => {
                order.push(i);
                text.push("default".to_owned());
            }
            None => text.push("%".to_owned()),
        }
        let mut e = self.element(ElementKind::Case, &text);
        if let Some(sink) = sink.clone() {
            self.sinks.push(sink);
        }
        for (branch, &i) in order.iter().enumerate() {
            let elements = if merged[i].rule && sink.is_some() {
                self.rule_value(&bodies[i])
            } else {
                self.statements(&bodies[i])
            };
            e.branches[branch] = Sequence::new(elements);
        }
        if sink.is_some() {
            self.sinks.pop();
        }
        e
    }

    /// The body of a `case … ->` in a switch expression: a value, a block or a throw.
    fn rule_value(&mut self, body: &[Node]) -> Vec<Element> {
        match body {
            [single] if single.kind() == "expression_statement" => {
                let value = single.named_child(0).map(|v| lines(v, self.src)).unwrap_or_default();
                vec![self.valued(value, *single)]
            }
            _ => self.statements(body),
        }
    }

    fn try_statement(&mut self, node: Node) -> Element {
        let src = self.src;
        let mut catches = Vec::new();
        let mut finally = None;
        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            match child.kind() {
                "catch_clause" => catches.push(child),
                "finally_clause" => finally = Some(child),
                _ => {}
            }
        }
        let mut tried = Vec::new();
        if let Some(resources) = node.child_by_field_name("resources") {
            let mut cursor = resources.walk();
            for r in resources.named_children(&mut cursor).filter(|r| r.kind() == "resource") {
                let mut text = lines(r, src);
                if let Some(last) = text.last_mut() {
                    last.push(';');
                }
                tried.push(self.element(ElementKind::Instruction, &text));
            }
        }
        if let Some(body) = node.child_by_field_name("body") {
            tried.extend(self.block(body));
        }
        let (variable, caught) = match catches.as_slice() {
            [] => (String::new(), Vec::new()),
            [only] if !catch_type(*only, src).contains('|') => {
                (catch_name(*only, src), self.catch_body(*only))
            }
            [only] => {
                // One catch of several types: a case on the exception's type.
                let name = catch_name(*only, src);
                let text = vec![
                    format!("{name} instanceof"),
                    catch_type(*only, src).replace(" | ", ", ").replace('|', ", "),
                    "%".to_owned(),
                ];
                let mut case = self.element(ElementKind::Case, &text);
                case.branches[0] = Sequence::new(self.catch_body(*only));
                (name, vec![case])
            }
            several => {
                // Several catches: one case on the exception's type, each branch naming the
                // exception as its catch does.
                let mut text = vec!["exception instanceof".to_owned()];
                text.extend(several.iter().map(|c| catch_type(*c, src).replace(" | ", ", ")));
                text.push("%".to_owned());
                let mut case = self.element(ElementKind::Case, &text);
                for (i, c) in several.iter().enumerate() {
                    let rename = format!("{} = exception;", catch_name(*c, src));
                    let mut body = vec![self.element(ElementKind::Instruction, &[rename])];
                    body.extend(self.catch_body(*c));
                    case.branches[i] = Sequence::new(body);
                }
                ("exception".to_owned(), vec![case])
            }
        };
        let mut e = self.element(ElementKind::Try, &[variable]);
        e.branches[0] = Sequence::new(tried);
        e.branches[1] = Sequence::new(caught);
        if let Some(f) = finally {
            let mut cursor = f.walk();
            let block = f.named_children(&mut cursor).find(|c| c.kind() == "block");
            e.branches[2] = Sequence::new(block.map(|b| self.block(b)).unwrap_or_default());
        }
        e
    }

    fn catch_body(&mut self, clause: Node) -> Vec<Element> {
        clause.child_by_field_name("body").map(|b| self.block(b)).unwrap_or_default()
    }
}

/// The cases of a `switch` before they are joined.
struct Group<'t> {
    node: Node<'t>,
    labels: Vec<String>,
    default: bool,
    /// `case … ->`: never runs on.
    rule: bool,
    statements: Vec<Node<'t>>,
}

impl<'t> Group<'t> {
    fn of(node: Node<'t>, src: &str) -> Group<'t> {
        let mut labels = Vec::new();
        let mut default = false;
        let mut statements = Vec::new();
        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            if child.kind() == "switch_label" {
                let label = node_text(child, src).trim();
                match label.strip_prefix("case") {
                    Some(rest) => labels.push(crate::text::one_line(rest)),
                    None => default = true,
                }
            } else {
                statements.push(child);
            }
        }
        Group { node, labels, default, rule: node.kind() == "switch_rule", statements }
    }
}

/// Whether the statements leave the case (comments aside).
fn ends_case(statements: &[Node]) -> bool {
    statements
        .iter()
        .rev()
        .find(|s| !matches!(s.kind(), "line_comment" | "block_comment"))
        .is_some_and(|s| ENDS_CASE.contains(&s.kind()))
}

/// `T x = switch …;` with a single variable: its name and the switch.
fn declared_switch<'t>(node: Node<'t>, src: &str) -> Option<(String, Node<'t>)> {
    let mut cursor = node.walk();
    let declarators: Vec<Node> = node.children_by_field_name("declarator", &mut cursor).collect();
    let [declarator] = declarators.as_slice() else { return None };
    let value = declarator.child_by_field_name("value")?;
    let name = declarator.child_by_field_name("name")?;
    (value.kind() == "switch_expression").then(|| (node_text(name, src).to_owned(), value))
}

fn type_text(node: Node, src: &str) -> String {
    node.child_by_field_name("type").map(|t| node_text(t, src).to_owned()).unwrap_or_default()
}

fn catch_parameter(clause: Node) -> Option<Node> {
    let mut cursor = clause.walk();
    clause.named_children(&mut cursor).find(|c| c.kind() == "catch_formal_parameter")
}

fn catch_type(clause: Node, src: &str) -> String {
    let parameter = catch_parameter(clause);
    let mut cursor = clause.walk();
    let t =
        parameter.and_then(|p| p.named_children(&mut cursor).find(|c| c.kind() == "catch_type"));
    t.map(|t| crate::text::one_line(node_text(t, src))).unwrap_or_default()
}

fn catch_name(clause: Node, src: &str) -> String {
    catch_parameter(clause)
        .and_then(|p| p.child_by_field_name("name"))
        .map(|n| node_text(n, src).to_owned())
        .unwrap_or_default()
}

fn append_comment(e: &mut Element, lines: &[String]) {
    let mut text: Vec<String> = e.comment().into_iter().filter(|l| !l.is_empty()).collect();
    text.extend(lines.iter().cloned());
    e.set_comment(&text);
}
