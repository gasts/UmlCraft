//! Writing `.nsd` files the way Structorizer writes them: the XML declaration, one tab of
//! indentation per level, an element without branches on one line, every branch tag on lines
//! of its own, LF line ends and none after `</root>`. Attributes keep their order.

use crate::model::{Attrs, Branches, Diagram, Element, Sequence};
use crate::text::escape_attr;

/// The file content of `diagram`.
pub fn write(diagram: &Diagram) -> String {
    let mut out = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    open_tag(&mut out, 0, "root", &diagram.attrs);
    out.push('\n');
    sequence(&mut out, 1, "children", &diagram.children);
    out.push_str("</root>");
    out
}

fn open_tag(out: &mut String, depth: usize, tag: &str, attrs: &Attrs) {
    out.extend(std::iter::repeat_n('\t', depth));
    out.push('<');
    out.push_str(tag);
    for a in &attrs.0 {
        out.push(' ');
        out.push_str(&a.name);
        out.push_str("=\"");
        out.push_str(&escape_attr(&a.value));
        out.push('"');
    }
    out.push('>');
}

fn close_tag(out: &mut String, depth: usize, tag: &str) {
    out.extend(std::iter::repeat_n('\t', depth));
    out.push_str("</");
    out.push_str(tag);
    out.push_str(">\n");
}

fn sequence(out: &mut String, depth: usize, tag: &str, seq: &Sequence) {
    open_tag(out, depth, tag, &seq.attrs);
    out.push('\n');
    for e in &seq.elements {
        element(out, depth + 1, e);
    }
    close_tag(out, depth, tag);
}

fn element(out: &mut String, depth: usize, e: &Element) {
    let tag = e.kind.tag();
    open_tag(out, depth, tag, &e.attrs);
    if e.branches.is_empty() {
        out.push_str("</");
        out.push_str(tag);
        out.push_str(">\n");
        return;
    }
    out.push('\n');
    for (i, b) in e.branches.iter().enumerate() {
        let branch_tag = match e.kind.branches() {
            Branches::Fixed(tags) => tags[i],
            Branches::Repeated(t) => t,
        };
        sequence(out, depth + 1, branch_tag, b);
    }
    close_tag(out, depth, tag);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::read;

    #[test]
    fn writes_back_what_it_read() {
        let file = r#"<?xml version="1.0" encoding="UTF-8"?>
<root xmlns:nsd="https://structorizer.fisch.lu" version="3.32-35" text="&#34;demo&#34;" comment="" color="ffffff" type="program" style="nice">
	<children>
		<instruction text="&#34;a &#60;- 1&#34;,&#34;s &#60;- &#34;&#34;&amp;&#34;&#34;&#34;" comment="" color="ffffff" rotated="0" disabled="0"></instruction>
		<alternative text="&#34;a &#62; b&#34;" comment="" color="ffffff" branch_colors="," disabled="0">
			<qTrue>
				<jump text="&#34;return&#34;" comment="" color="ffffff" disabled="0"></jump>
			</qTrue>
			<qFalse color="ffffff">
			</qFalse>
		</alternative>
		<instruction text="&#34;tab	here&#34;" comment="&#34;line&#34;" color="ffffff" rotated="0" disabled="0"></instruction>
	</children>
</root>"#;
        assert_eq!(write(&read(file).unwrap()), file);
    }

    #[test]
    fn writes_an_empty_diagram_like_structorizer() {
        let file = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<root text=\"\">\n\t<children color=\"ffffff\">\n\t</children>\n</root>";
        assert_eq!(write(&read(file).unwrap()), file);
    }

    #[test]
    fn escapes_line_feeds_in_attributes() {
        let file = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<root license=\"a&#10;b&#10;\">\n\t<children>\n\t</children>\n</root>";
        let d = read(file).unwrap();
        assert_eq!(d.header("license"), Some("a\nb\n"));
        assert_eq!(write(&d), file);
    }
}
