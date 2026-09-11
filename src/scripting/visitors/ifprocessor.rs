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

use std::cell::{Cell, RefCell};
use std::collections::HashSet;

use crate::scripting::{
    nodes::{event::EventStream, node::Node, traits::NodeVisitor},
    utils::errors::Result,
};

/// Visitor that records, for each `if` node, the indices of variables
/// modified inside the `if` block (including nested `if`s). It also
/// keeps track of the maximum nesting depth of `if` statements.
pub struct IfProcessor {
    var_stack: RefCell<Vec<HashSet<usize>>>,
    nested_if_lvl: Cell<usize>,
    max_nested_ifs: Cell<usize>,
}

impl IfProcessor {
    /// Create a new `IfProcessor` visitor.
    pub fn new() -> Self {
        Self {
            var_stack: RefCell::new(Vec::new()),
            nested_if_lvl: Cell::new(0),
            max_nested_ifs: Cell::new(0),
        }
    }

    /// Maximum nesting depth encountered after visiting.
    pub fn max_nested_ifs(&self) -> usize {
        self.max_nested_ifs.get()
    }

    /// Visit all events in a stream.
    pub fn visit_events(&self, events: &mut EventStream) -> Result<()> {
        events.mut_events().iter_mut().try_for_each(|event| {
            self.visit(event.mut_expr())?;
            Ok(())
        })
    }
}

impl Default for IfProcessor {
    fn default() -> Self {
        Self::new()
    }
}

impl NodeVisitor for IfProcessor {
    type Output = Result<()>;

    fn visit(&self, node: &mut Node) -> Self::Output {
        match node {
            Node::If(data) => {
                let lvl = self.nested_if_lvl.get() + 1;
                self.nested_if_lvl.set(lvl);
                if lvl > self.max_nested_ifs.get() {
                    self.max_nested_ifs.set(lvl);
                }

                self.var_stack.borrow_mut().push(HashSet::new());

                for c in data.children.iter_mut().skip(1) {
                    self.visit(c)?;
                }

                let vars = self.var_stack.borrow_mut().pop().unwrap();
                let mut vec: Vec<usize> = vars.iter().cloned().collect();
                vec.sort_unstable();
                data.affected_vars = vec;

                let lvl = lvl - 1;
                self.nested_if_lvl.set(lvl);
                if lvl > 0 {
                    let mut stack = self.var_stack.borrow_mut();
                    if let Some(top) = stack.last_mut() {
                        for v in vars {
                            top.insert(v);
                        }
                    }
                }
                Ok(())
            }
            Node::Assign(data) => {
                if self.nested_if_lvl.get() > 0 {
                    if let Some(var) = data.children.get_mut(0) {
                        self.visit(var)?;
                    }
                }
                Ok(())
            }
            Node::Pays(_) => Ok(()),
            Node::ForEach(data) => {
                self.visit(&mut data.node)?;
                for child in &mut data.children {
                    self.visit(child)?;
                }
                Ok(())
            }
            Node::Index(data) => {
                for child in &mut data.children {
                    self.visit(child)?;
                }
                self.visit(&mut data.index)
            }
            Node::Variable(data) => {
                if self.nested_if_lvl.get() > 0 {
                    if let Some(i) = data.id {
                        if let Some(top) = self.var_stack.borrow_mut().last_mut() {
                            top.insert(i);
                        }
                    }
                }
                Ok(())
            }
            Node::Spot(_)
            | Node::Df(_)
            | Node::RateIndex(_)
            | Node::True
            | Node::False
            | Node::Constant(_)
            | Node::String(_) => Ok(()),
            _ => {
                let children = node.children_mut();
                for c in children.iter_mut() {
                    self.visit(c)?;
                }
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        scripting::{nodes::event::CodedEvent, visitors::varindexer::VarIndexer},
        time::date::Date,
    };

    #[test]
    fn test_if_processor_nested() {
        let script = "x = 0; if x == 0 { y = 1; if y == 1 { z = 2; } w = 3; }";
        let mut expr = Node::try_from(script).unwrap();

        let indexer = VarIndexer::new();
        indexer.visit(&mut expr).unwrap();

        let processor = IfProcessor::new();
        processor.visit(&mut expr).unwrap();

        // outer if is second expression in base node
        let outer_if = match &expr {
            Node::Base(data) => &data.children[1],
            _ => panic!("expected base node"),
        };

        let inner_if = match outer_if {
            Node::If(data) => &data.children[2],
            _ => panic!("expected if node"),
        };

        if let Node::If(data) = outer_if {
            assert_eq!(data.affected_vars, vec![1, 2, 3]);
        } else {
            panic!("expected if node");
        }

        if let Node::If(data) = inner_if {
            assert_eq!(data.affected_vars, vec![2]);
        } else {
            panic!("expected inner if node");
        }

        assert_eq!(processor.max_nested_ifs(), 2);
    }

    #[test]
    fn test_if_processor_else_branch() {
        let script = "if a == 1 { x = 2; } else { y = 3; }";
        let mut expr = Node::try_from(script).unwrap();

        let indexer = VarIndexer::new();
        indexer.visit(&mut expr).unwrap();

        let processor = IfProcessor::new();
        processor.visit(&mut expr).unwrap();

        if let Node::Base(data) = &expr {
            if let Node::If(ifnode) = &data.children[0] {
                assert_eq!(ifnode.affected_vars.len(), 2);
            } else {
                panic!("expected if node");
            }
        } else {
            panic!("expected base node");
        }
    }

    #[test]
    fn test_if_processor_on_event_stream() {
        let script1 = "if b == 0 { x = 1; }";
        let script2 = "y = 2;";
        let mut events = EventStream::try_from(vec![
            CodedEvent::new(Date::new(2024, 1, 1), script1.to_string()),
            CodedEvent::new(Date::new(2024, 1, 2), script2.to_string()),
        ])
        .unwrap();

        let indexer = VarIndexer::new();
        indexer.visit_events(&mut events).unwrap();

        let processor = IfProcessor::new();
        processor.visit_events(&mut events).unwrap();

        let ifnode = match events.mut_events()[0].mut_expr() {
            Node::Base(b) => match &b.children[0] {
                Node::If(data) => data,
                _ => panic!("expected if node"),
            },
            _ => panic!("expected base node"),
        };
        assert_eq!(ifnode.affected_vars.len(), 1);
        assert_eq!(processor.max_nested_ifs(), 1);
    }
}
