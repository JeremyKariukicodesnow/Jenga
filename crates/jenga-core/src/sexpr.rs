//! Lossless semantic AST: unfamiliar KiCad fields survive editing and saving.
use anyhow::{Result, bail};

#[derive(Clone, Debug, PartialEq)]
pub enum Node {
    Atom(String),
    String(String),
    List(Vec<Node>),
}

impl Node {
    pub fn atom(value: impl ToString) -> Self {
        Self::Atom(value.to_string())
    }
    pub fn string(value: impl ToString) -> Self {
        Self::String(value.to_string())
    }
    pub fn list(name: &str, values: Vec<Node>) -> Self {
        let mut nodes = vec![Self::atom(name)];
        nodes.extend(values);
        Self::List(nodes)
    }
    pub fn text(&self) -> &str {
        match self {
            Self::Atom(s) | Self::String(s) => s,
            _ => "",
        }
    }
    pub fn items(&self) -> &[Node] {
        match self {
            Self::List(xs) => xs,
            _ => &[],
        }
    }
    pub fn items_mut(&mut self) -> &mut Vec<Node> {
        match self {
            Self::List(xs) => xs,
            _ => panic!("expected list"),
        }
    }
    pub fn name(&self) -> &str {
        self.items().first().map(Self::text).unwrap_or("")
    }
    pub fn value(&self, i: usize) -> &str {
        self.items().get(i).map(Self::text).unwrap_or("")
    }
    pub fn number(&self, i: usize) -> f64 {
        self.value(i).parse().unwrap_or(0.0)
    }
    pub fn child(&self, name: &str) -> Option<&Node> {
        self.items().iter().skip(1).find(|n| n.name() == name)
    }
    pub fn child_mut(&mut self, name: &str) -> Option<&mut Node> {
        self.items_mut()
            .iter_mut()
            .skip(1)
            .find(|n| n.name() == name)
    }
    pub fn children<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a Node> {
        self.items()
            .iter()
            .skip(1)
            .filter(move |n| n.name() == name)
    }
    pub fn set_child(&mut self, value: Node) {
        if let Some(existing) = self.child_mut(value.name()) {
            *existing = value;
        } else {
            self.items_mut().push(value);
        }
    }
    pub fn encoded(&self) -> String {
        match self {
            Self::Atom(s) => s.clone(),
            Self::String(s) => format!(
                "\"{}\"",
                s.replace('\\', "\\\\")
                    .replace('"', "\\\"")
                    .replace('\n', "\\n")
                    .replace('\r', "\\r")
                    .replace('\t', "\\t")
            ),
            Self::List(xs) => format!(
                "({})",
                xs.iter().map(Self::encoded).collect::<Vec<_>>().join(" ")
            ),
        }
    }
    pub fn document_text(&self) -> String {
        let mut out = format!("({}\n", self.name());
        for child in self.items().iter().skip(1) {
            out.push_str("  ");
            out.push_str(&child.encoded());
            out.push('\n');
        }
        out.push_str(")\n");
        out
    }
}

pub fn parse(source: &str) -> Result<Node> {
    struct Parser<'a> {
        bytes: &'a [u8],
        pos: usize,
    }
    impl Parser<'_> {
        fn space(&mut self) {
            loop {
                while self.pos < self.bytes.len() && self.bytes[self.pos].is_ascii_whitespace() {
                    self.pos += 1;
                }
                if self.bytes.get(self.pos) == Some(&b';') {
                    while self.pos < self.bytes.len() && self.bytes[self.pos] != b'\n' {
                        self.pos += 1;
                    }
                } else {
                    break;
                }
            }
        }
        fn node(&mut self, depth: usize) -> Result<Node> {
            if depth > 128 {
                bail!("S-expression nesting exceeds 128 levels");
            }
            self.space();
            match self.bytes.get(self.pos).copied() {
                Some(b'(') => {
                    self.pos += 1;
                    let mut xs = vec![];
                    loop {
                        self.space();
                        match self.bytes.get(self.pos) {
                            Some(b')') => {
                                self.pos += 1;
                                break;
                            }
                            None => bail!("Unclosed list at byte {}", self.pos),
                            _ => xs.push(self.node(depth + 1)?),
                        }
                    }
                    Ok(Node::List(xs))
                }
                Some(b'"') => {
                    self.pos += 1;
                    let mut bytes = vec![];
                    loop {
                        match self.bytes.get(self.pos).copied() {
                            None => bail!("Unclosed quoted string"),
                            Some(b'"') => {
                                self.pos += 1;
                                break;
                            }
                            Some(b'\\') => {
                                self.pos += 1;
                                let c = *self
                                    .bytes
                                    .get(self.pos)
                                    .ok_or_else(|| anyhow::anyhow!("Unclosed escape"))?;
                                bytes.push(match c {
                                    b'n' => b'\n',
                                    b'r' => b'\r',
                                    b't' => b'\t',
                                    _ => c,
                                });
                                self.pos += 1;
                            }
                            Some(c) => {
                                bytes.push(c);
                                self.pos += 1;
                            }
                        }
                    }
                    Ok(Node::String(String::from_utf8(bytes)?))
                }
                Some(b')') | None => bail!("Unexpected end at byte {}", self.pos),
                _ => {
                    let start = self.pos;
                    while self.pos < self.bytes.len()
                        && !self.bytes[self.pos].is_ascii_whitespace()
                        && !b"()".contains(&self.bytes[self.pos])
                    {
                        self.pos += 1;
                    }
                    Ok(Node::Atom(String::from_utf8(
                        self.bytes[start..self.pos].to_vec(),
                    )?))
                }
            }
        }
    }
    let mut parser = Parser {
        bytes: source.trim_start_matches('\u{feff}').as_bytes(),
        pos: 0,
    };
    let root = parser.node(0)?;
    parser.space();
    if parser.pos != parser.bytes.len() {
        bail!("Unexpected trailing data at byte {}", parser.pos);
    }
    Ok(root)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unicode_escaping_and_unknown_fields_roundtrip() {
        let source =
            "(root ; comment\n (future (opaque yes)) (text \"µ Ω \\\"quoted\\\"\\nnext\"))";
        let n = parse(source).unwrap();
        assert_eq!(parse(&n.document_text()).unwrap(), n);
    }
    #[test]
    fn rejects_incomplete_and_trailing_input() {
        for s in ["(x", "(x) (y)", "(x \"oops)"] {
            assert!(parse(s).is_err());
        }
    }
}
