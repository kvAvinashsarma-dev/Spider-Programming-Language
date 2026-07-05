//! Character spans for CST nodes.
//!
//! The lossless tree stores no positions; positions are derivable because the
//! tree reproduces the source exactly. One pre-order walk assigns every node
//! its (start, end) character range, keyed by node identity.

use spider_syntax::{Element, Node};
use std::collections::HashMap;
use std::rc::Rc;

pub struct SpanMap {
    map: HashMap<usize, (usize, usize)>,
}

impl SpanMap {
    pub fn build(root: &Rc<Node>) -> SpanMap {
        let mut map = HashMap::new();
        let mut offset = 0usize;
        walk(root, &mut offset, &mut map);
        SpanMap { map }
    }

    pub fn of(&self, node: &Rc<Node>) -> (usize, usize) {
        *self
            .map
            .get(&(Rc::as_ptr(node) as usize))
            .unwrap_or(&(0, 0))
    }
}

fn walk(node: &Rc<Node>, offset: &mut usize, map: &mut HashMap<usize, (usize, usize)>) {
    let start = *offset;
    for el in &node.children {
        match el {
            Element::Token(t) => *offset += t.text.chars().count(),
            Element::Node(n) => walk(n, offset, map),
        }
    }
    map.insert(Rc::as_ptr(node) as usize, (start, *offset));
}
