//! Typed AST views over the lossless CST.
//!
//! These wrappers do not copy the tree; they are thin lenses used by later
//! compiler stages (name resolution, type inference in M2) and by tools that
//! want structure without token-level detail.

use crate::kind::SyntaxKind as K;
use crate::tree::Node;
use std::rc::Rc;

pub struct SourceFile<'a>(pub &'a Rc<Node>);

impl<'a> SourceFile<'a> {
    pub fn functions(&self) -> Vec<FnDecl<'a>> {
        self.0.nodes_of(K::FnDecl).into_iter().map(FnDecl).collect()
    }

    pub fn records(&self) -> Vec<RecordDecl<'a>> {
        self.0
            .nodes_of(K::RecordDecl)
            .into_iter()
            .map(RecordDecl)
            .collect()
    }

    pub fn uses(&self) -> Vec<String> {
        self.0
            .nodes_of(K::UseDecl)
            .into_iter()
            .map(|u| {
                u.child_tokens()
                    .into_iter()
                    .filter(|t| matches!(t.kind, K::Ident | K::Dot))
                    .map(|t| t.text.clone())
                    .collect::<String>()
            })
            .collect()
    }
}

pub struct FnDecl<'a>(pub &'a Rc<Node>);

impl<'a> FnDecl<'a> {
    pub fn name(&self) -> Option<String> {
        self.0.find_token(K::Ident).map(|t| t.text.clone())
    }

    pub fn is_public(&self) -> bool {
        self.0.find_token(K::PublicKw).is_some()
    }

    pub fn param_names(&self) -> Vec<String> {
        self.0
            .find_node(K::ParamList)
            .map(|pl| {
                pl.nodes_of(K::Param)
                    .into_iter()
                    .filter_map(|p| p.find_token(K::Ident).map(|t| t.text.clone()))
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn has_return_type(&self) -> bool {
        self.0.find_node(K::RetType).is_some()
    }
}

pub struct RecordDecl<'a>(pub &'a Rc<Node>);

impl<'a> RecordDecl<'a> {
    pub fn name(&self) -> Option<String> {
        self.0.find_token(K::Ident).map(|t| t.text.clone())
    }

    pub fn field_names(&self) -> Vec<String> {
        self.0
            .find_node(K::Block)
            .map(|b| {
                b.nodes_of(K::FieldDecl)
                    .into_iter()
                    .filter_map(|f| f.find_token(K::Ident).map(|t| t.text.clone()))
                    .collect()
            })
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse;

    #[test]
    fn typed_views() {
        let src = "\
use math

public fn total(prices: List of Float) -> Float
    return 0.0

record Point
    x: Float
    y: Float
";
        let p = parse(src);
        assert!(p.diagnostics.is_empty(), "{:?}", p.diagnostics);
        let file = SourceFile(&p.root);

        assert_eq!(file.uses(), vec!["math"]);

        let fns = file.functions();
        assert_eq!(fns.len(), 1);
        assert_eq!(fns[0].name().as_deref(), Some("total"));
        assert!(fns[0].is_public());
        assert!(fns[0].has_return_type());
        assert_eq!(fns[0].param_names(), vec!["prices"]);

        let recs = file.records();
        assert_eq!(recs[0].name().as_deref(), Some("Point"));
        assert_eq!(recs[0].field_names(), vec!["x", "y"]);
    }
}
