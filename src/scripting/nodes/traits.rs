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

use crate::scripting::nodes::node::Node;

/// Visitor that can mutate an AST node.
pub trait NodeVisitor {
    /// Value returned by the visitor.
    type Output;
    /// Visits and optionally mutates `node`.
    fn visit(&self, node: &mut Node) -> Self::Output;
}

/// Visitor that reads an AST node without mutation.
pub trait NodeConstVisitor {
    /// Value returned by the visitor.
    type Output;
    /// Visits `node` without mutating it.
    fn const_visit(&self, node: &Node) -> Self::Output;
}

/// AST node accepting a mutable visitor.
pub trait Visitable {
    /// Value returned by the accepted visitor.
    type Output;
    /// Dispatches `visitor` to this value.
    fn accept(&mut self, visitor: &impl NodeVisitor) -> Self::Output;
}

/// AST node accepting a read-only visitor.
pub trait ConstVisitable {
    /// Value returned by the accepted visitor.
    type Output;
    /// Dispatches `visitor` without mutating this value.
    fn const_accept(&self, visitor: &impl NodeConstVisitor) -> Self::Output;
}
