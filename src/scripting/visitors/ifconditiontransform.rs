/*
This file is part of QuantSupport's Rust rewrite and adaptation of the
derivatives scripting code written by Antoine Savine in 2018.

The original code is the strict intellectual property of Antoine Savine.

A license to use and alter the original code for personal and commercial
applications is freely granted to any person or company that purchased a copy
of the book:

Modern Computational Finance: Scripting for Derivatives and XVA
Jesper Andreasen and Antoine Savine
Wiley, 2018

This attribution and license notice must be preserved at the top of this file.
*/

use crate::scripting::nodes::{node::Node, traits::NodeVisitor};

/// Visitor transforming `if` statement conditions into a canonical
/// form using only `> 0`, `>= 0` or `== 0` comparisons. This is
/// required by the fuzzy evaluator which expects conditions as
/// differences versus zero.
pub struct IfConditionTransform;

impl IfConditionTransform {
    /// Creates a conditional canonicalization visitor.
    pub fn new() -> Self {
        Self
    }

    fn transform_cond(&self, node: &mut Node) {
        match node {
            Node::Superior(data) => {
                for c in data.children.iter_mut() {
                    self.transform_cond(c);
                }
                let left = data.children[0].clone();
                let right = data.children[1].clone();
                data.children.clear();
                data.children
                    .push(Node::new_subtract_with_values(left, right));
                data.children.push(Node::new_constant(0.0));
            }
            Node::SuperiorOrEqual(data) => {
                for c in data.children.iter_mut() {
                    self.transform_cond(c);
                }
                let left = data.children[0].clone();
                let right = data.children[1].clone();
                data.children.clear();
                data.children
                    .push(Node::new_subtract_with_values(left, right));
                data.children.push(Node::new_constant(0.0));
            }
            Node::Inferior(data) => {
                for c in data.children.iter_mut() {
                    self.transform_cond(c);
                }
                let left = data.children[1].clone();
                let right = data.children[0].clone();
                *node = Node::new_superior_with_values(
                    Node::new_subtract_with_values(left, right),
                    Node::new_constant(0.0),
                );
            }
            Node::InferiorOrEqual(data) => {
                for c in data.children.iter_mut() {
                    self.transform_cond(c);
                }
                let left = data.children[1].clone();
                let right = data.children[0].clone();
                *node = Node::new_superior_or_equal_with_values(
                    Node::new_subtract_with_values(left, right),
                    Node::new_constant(0.0),
                );
            }
            Node::Equal(data) => {
                for c in data.children.iter_mut() {
                    self.transform_cond(c);
                }
                let left = data.children[0].clone();
                let right = data.children[1].clone();
                data.children.clear();
                data.children
                    .push(Node::new_subtract_with_values(left, right));
                data.children.push(Node::new_constant(0.0));
            }
            Node::NotEqual(data) => {
                for c in &mut data.children {
                    self.transform_cond(c);
                }
                let left = data.children[0].clone();
                let right = data.children[1].clone();
                *node = Node::new_not_with_value(Node::new_equal_with_values(
                    Node::new_subtract_with_values(left, right),
                    Node::new_constant(0.0),
                ));
            }
            Node::And(data) | Node::Or(data) | Node::Not(data) => {
                for c in data.children.iter_mut() {
                    self.transform_cond(c);
                }
            }
            Node::Constant(_)
            | Node::True
            | Node::False
            | Node::String(_)
            | Node::Spot(_)
            | Node::Df(_)
            | Node::RateIndex(_) => {}
            _ => {
                for c in node.children_mut().iter_mut() {
                    self.transform_cond(c);
                }
            }
        }
    }
}

impl Default for IfConditionTransform {
    fn default() -> Self {
        Self::new()
    }
}

impl NodeVisitor for IfConditionTransform {
    type Output = ();

    fn visit(&self, node: &mut Node) {
        match node {
            Node::If(data) => {
                if let Some(cond) = data.children.get_mut(0) {
                    self.transform_cond(cond);
                }
                for c in data.children.iter_mut().skip(1) {
                    self.visit(c);
                }
            }
            Node::Constant(_)
            | Node::True
            | Node::False
            | Node::String(_)
            | Node::Spot(_)
            | Node::Df(_)
            | Node::RateIndex(_) => {}
            _ => {
                for c in node.children_mut().iter_mut() {
                    self.visit(c);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scripting::parsing::{lexer::Lexer, parser::Parser};

    #[test]
    fn test_transform_inferior() {
        let script = "if a < 1 { b = 2; }".to_string();
        let tokens = Lexer::new(script).tokenize().unwrap();
        let mut expr = Parser::new(tokens).parse().unwrap();
        let transformer = IfConditionTransform::new();
        transformer.visit(&mut expr);

        let cond = match &expr {
            Node::Base(b) => match &b.children[0] {
                Node::If(data) => &data.children[0],
                _ => panic!("expected if"),
            },
            _ => panic!("expected base"),
        };

        let expected = Node::new_superior_with_values(
            Node::new_subtract_with_values(
                Node::new_constant(1.0),
                Node::new_variable("a".to_string()),
            ),
            Node::new_constant(0.0),
        );
        assert_eq!(*cond, expected);
    }
}
