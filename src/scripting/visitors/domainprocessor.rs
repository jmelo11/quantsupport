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

use crate::scripting::{
    nodes::{node::Node, traits::NodeVisitor},
    utils::errors::Result,
};

/// Simplified domain representation used for constant propagation.
#[derive(Clone, Debug, PartialEq)]
pub enum Domain {
    /// Value cannot be determined statically.
    Any,
    /// Statically known numeric value.
    Constant(f64),
}

impl Domain {
    fn add(&self, other: &Domain) -> Domain {
        match (self, other) {
            (Domain::Constant(a), Domain::Constant(b)) => Domain::Constant(a + b),
            _ => Domain::Any,
        }
    }

    fn sub(&self, other: &Domain) -> Domain {
        match (self, other) {
            (Domain::Constant(a), Domain::Constant(b)) => Domain::Constant(a - b),
            _ => Domain::Any,
        }
    }

    fn mul(&self, other: &Domain) -> Domain {
        match (self, other) {
            (Domain::Constant(a), Domain::Constant(b)) => Domain::Constant(a * b),
            _ => Domain::Any,
        }
    }

    fn div(&self, other: &Domain) -> Domain {
        match (self, other) {
            (Domain::Constant(a), Domain::Constant(b)) => Domain::Constant(a / b),
            _ => Domain::Any,
        }
    }

    fn apply_unary<F: Fn(f64) -> f64>(&self, f: F) -> Domain {
        match self {
            Domain::Constant(a) => Domain::Constant(f(*a)),
            _ => Domain::Any,
        }
    }

    fn union(&self, other: &Domain) -> Domain {
        match (self, other) {
            (Domain::Constant(a), Domain::Constant(b)) if (*a - *b).abs() < f64::EPSILON => {
                Domain::Constant(*a)
            }
            _ => Domain::Any,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
/// Static truth classification for a condition.
pub enum CondProp {
    /// Condition is always true.
    AlwaysTrue,
    /// Condition is always false.
    AlwaysFalse,
    /// Condition depends on runtime values.
    TrueOrFalse,
}

/// Minimal implementation of the C++ `DomainProcessor`.
/// It propagates constant values and detects constant conditions.
pub struct DomainProcessor {
    var_domains: RefCell<Vec<Domain>>,
    dom_stack: RefCell<Vec<Domain>>,
    cond_stack: RefCell<Vec<CondProp>>,
    lhs_var: Cell<bool>,
    lhs_var_idx: Cell<usize>,
}

impl DomainProcessor {
    /// Creates a processor with `n_var` zero-initialized variable domains.
    pub fn new(n_var: usize) -> Self {
        Self {
            var_domains: RefCell::new(vec![Domain::Constant(0.0); n_var]),
            dom_stack: RefCell::new(Vec::new()),
            cond_stack: RefCell::new(Vec::new()),
            lhs_var: Cell::new(false),
            lhs_var_idx: Cell::new(0),
        }
    }

    /// Returns the inferred domain of every indexed variable.
    pub fn variable_domains(&self) -> Vec<Domain> {
        self.var_domains.borrow().clone()
    }
}

impl NodeVisitor for DomainProcessor {
    type Output = Result<()>;

    fn visit(&self, node: &mut Node) -> Self::Output {
        match node {
            // Binary expressions and some functions
            Node::Add(data)
            | Node::Subtract(data)
            | Node::Multiply(data)
            | Node::Divide(data)
            | Node::Pow(data)
            | Node::Max(data)
            | Node::Min(data)
            | Node::Append(data)
            | Node::Mean(data)
            | Node::Std(data) => {
                for c in data.children.iter_mut() {
                    self.visit(c)?;
                }
                let mut stack = self.dom_stack.borrow_mut();
                let mut res = stack.pop().unwrap_or(Domain::Any);
                while let Some(arg) = stack.pop() {
                    res = match node {
                        Node::Add(_) => arg.add(&res),
                        Node::Subtract(_) => arg.sub(&res),
                        Node::Multiply(_) => arg.mul(&res),
                        Node::Divide(_) => arg.div(&res),
                        Node::Pow(_) => match (&arg, &res) {
                            (Domain::Constant(a), Domain::Constant(b)) => {
                                Domain::Constant(a.powf(*b))
                            }
                            _ => Domain::Any,
                        },
                        Node::Min(_) => match (&arg, &res) {
                            (Domain::Constant(a), Domain::Constant(b)) => {
                                Domain::Constant(a.min(*b))
                            }
                            _ => Domain::Any,
                        },
                        Node::Max(_) => match (&arg, &res) {
                            (Domain::Constant(a), Domain::Constant(b)) => {
                                Domain::Constant(a.max(*b))
                            }
                            _ => Domain::Any,
                        },
                        _ => Domain::Any,
                    };
                }
                stack.push(res);
                Ok(())
            }
            // Unary
            Node::UnaryPlus(data) | Node::UnaryMinus(data) | Node::Exp(data) | Node::Ln(data) => {
                for c in data.children.iter_mut() {
                    self.visit(c)?;
                }
                let mut stack = self.dom_stack.borrow_mut();
                let arg = stack.pop().unwrap_or(Domain::Any);
                let res = match node {
                    Node::UnaryMinus(_) => arg.apply_unary(|v| -v),
                    Node::Exp(_) => arg.apply_unary(|v| v.exp()),
                    Node::Ln(_) => arg.apply_unary(|v| v.ln()),
                    _ => arg,
                };
                stack.push(res);
                Ok(())
            }
            // smooth etc
            Node::Fif(data) | Node::Cvg(data) => {
                for c in data.children.iter_mut() {
                    self.visit(c)?;
                }
                self.dom_stack.borrow_mut().push(Domain::Any);
                Ok(())
            }
            // Conditions
            Node::Equal(data)
            | Node::NotEqual(data)
            | Node::Superior(data)
            | Node::Inferior(data)
            | Node::SuperiorOrEqual(data)
            | Node::InferiorOrEqual(data) => {
                for c in data.children.iter_mut() {
                    self.visit(c)?;
                }
                let right = self.dom_stack.borrow_mut().pop().unwrap_or(Domain::Any);
                let left = self.dom_stack.borrow_mut().pop().unwrap_or(Domain::Any);
                let diff = left.sub(&right);
                let prop = match (&diff, node) {
                    (Domain::Constant(v), Node::Equal(_)) => {
                        if v.abs() < f64::EPSILON {
                            CondProp::AlwaysTrue
                        } else {
                            CondProp::AlwaysFalse
                        }
                    }
                    (Domain::Constant(v), Node::NotEqual(_)) => {
                        if v.abs() >= f64::EPSILON {
                            CondProp::AlwaysTrue
                        } else {
                            CondProp::AlwaysFalse
                        }
                    }
                    (Domain::Constant(v), Node::Superior(_)) => {
                        if *v > 0.0 {
                            CondProp::AlwaysTrue
                        } else {
                            CondProp::AlwaysFalse
                        }
                    }
                    (Domain::Constant(v), Node::Inferior(_)) => {
                        if *v < 0.0 {
                            CondProp::AlwaysTrue
                        } else {
                            CondProp::AlwaysFalse
                        }
                    }
                    (Domain::Constant(v), Node::SuperiorOrEqual(_)) => {
                        if *v >= 0.0 {
                            CondProp::AlwaysTrue
                        } else {
                            CondProp::AlwaysFalse
                        }
                    }
                    (Domain::Constant(v), Node::InferiorOrEqual(_)) => {
                        if *v <= 0.0 {
                            CondProp::AlwaysTrue
                        } else {
                            CondProp::AlwaysFalse
                        }
                    }
                    _ => CondProp::TrueOrFalse,
                };
                self.cond_stack.borrow_mut().push(prop);
                self.dom_stack.borrow_mut().push(Domain::Any);
                Ok(())
            }
            Node::Not(data) => {
                for c in data.children.iter_mut() {
                    self.visit(c)?;
                }
                let prop = match self.cond_stack.borrow_mut().pop() {
                    Some(CondProp::AlwaysTrue) => CondProp::AlwaysFalse,
                    Some(CondProp::AlwaysFalse) => CondProp::AlwaysTrue,
                    _ => CondProp::TrueOrFalse,
                };
                self.cond_stack.borrow_mut().push(prop);
                Ok(())
            }
            Node::And(data) => {
                for c in data.children.iter_mut() {
                    self.visit(c)?;
                }
                let right = self
                    .cond_stack
                    .borrow_mut()
                    .pop()
                    .unwrap_or(CondProp::TrueOrFalse);
                let left = self
                    .cond_stack
                    .borrow_mut()
                    .pop()
                    .unwrap_or(CondProp::TrueOrFalse);
                let prop = if left == CondProp::AlwaysTrue && right == CondProp::AlwaysTrue {
                    CondProp::AlwaysTrue
                } else if left == CondProp::AlwaysFalse || right == CondProp::AlwaysFalse {
                    CondProp::AlwaysFalse
                } else {
                    CondProp::TrueOrFalse
                };
                self.cond_stack.borrow_mut().push(prop);
                Ok(())
            }
            Node::Or(data) => {
                for c in data.children.iter_mut() {
                    self.visit(c)?;
                }
                let right = self
                    .cond_stack
                    .borrow_mut()
                    .pop()
                    .unwrap_or(CondProp::TrueOrFalse);
                let left = self
                    .cond_stack
                    .borrow_mut()
                    .pop()
                    .unwrap_or(CondProp::TrueOrFalse);
                let prop = if left == CondProp::AlwaysTrue || right == CondProp::AlwaysTrue {
                    CondProp::AlwaysTrue
                } else if left == CondProp::AlwaysFalse && right == CondProp::AlwaysFalse {
                    CondProp::AlwaysFalse
                } else {
                    CondProp::TrueOrFalse
                };
                self.cond_stack.borrow_mut().push(prop);
                Ok(())
            }
            Node::True => {
                self.cond_stack.borrow_mut().push(CondProp::AlwaysTrue);
                Ok(())
            }
            Node::False => {
                self.cond_stack.borrow_mut().push(CondProp::AlwaysFalse);
                Ok(())
            }
            Node::If(data) => {
                let last_true = data.first_else.unwrap_or(data.children.len());
                self.visit(&mut data.children[0])?; // condition
                let prop = self
                    .cond_stack
                    .borrow_mut()
                    .pop()
                    .unwrap_or(CondProp::TrueOrFalse);
                if prop == CondProp::AlwaysTrue {
                    for c in data.children.iter_mut().take(last_true).skip(1) {
                        self.visit(c)?;
                    }
                } else if prop == CondProp::AlwaysFalse {
                    if let Some(start) = data.first_else {
                        for c in data.children.iter_mut().skip(start) {
                            self.visit(c)?;
                        }
                    }
                } else {
                    let mut before = Vec::new();
                    for &idx in &data.affected_vars {
                        before.push(self.var_domains.borrow()[idx].clone());
                    }
                    for c in data.children.iter_mut().take(last_true).skip(1) {
                        self.visit(c)?;
                    }
                    let mut after_true = Vec::new();
                    for &idx in &data.affected_vars {
                        after_true.push(self.var_domains.borrow()[idx].clone());
                    }
                    for (i, &idx) in data.affected_vars.iter().enumerate() {
                        self.var_domains.borrow_mut()[idx] = before[i].clone();
                    }
                    if let Some(start) = data.first_else {
                        for c in data.children.iter_mut().skip(start) {
                            self.visit(c)?;
                        }
                    }
                    for (i, &idx) in data.affected_vars.iter().enumerate() {
                        let v = self.var_domains.borrow()[idx].clone().union(&after_true[i]);
                        self.var_domains.borrow_mut()[idx] = v;
                    }
                }
                Ok(())
            }
            Node::Assign(data) => {
                self.lhs_var.set(true);
                self.visit(&mut data.children[0])?;
                self.lhs_var.set(false);
                self.visit(&mut data.children[1])?;
                let domain = self.dom_stack.borrow_mut().pop().unwrap_or(Domain::Any);
                let idx = self.lhs_var_idx.get();
                self.var_domains.borrow_mut()[idx] = domain;
                Ok(())
            }
            Node::Pays(data) => {
                for child in &mut data.children {
                    self.visit(child)?;
                    let _ = self.dom_stack.borrow_mut().pop();
                }
                self.dom_stack.borrow_mut().push(Domain::Any);
                Ok(())
            }
            Node::Variable(data) => {
                if self.lhs_var.get() {
                    if let Some(i) = data.id {
                        self.lhs_var_idx.set(i);
                    }
                } else if let Some(i) = data.id {
                    let dom = self.var_domains.borrow()[i].clone();
                    self.dom_stack.borrow_mut().push(dom);
                } else {
                    self.dom_stack.borrow_mut().push(Domain::Any);
                }
                Ok(())
            }
            Node::Constant(data) => {
                self.dom_stack
                    .borrow_mut()
                    .push(Domain::Constant(data.const_value));
                Ok(())
            }
            Node::Base(data) => {
                for c in &mut data.children {
                    self.visit(c)?;
                }
                Ok(())
            }
            Node::Range(data) | Node::List(data) => {
                for c in &mut data.children {
                    self.visit(c)?;
                    let _ = self.dom_stack.borrow_mut().pop();
                }
                self.dom_stack.borrow_mut().push(Domain::Any);
                Ok(())
            }
            Node::Index(data) => {
                for c in &mut data.children {
                    self.visit(c)?;
                    let _ = self.dom_stack.borrow_mut().pop();
                }
                self.visit(&mut data.index)?;
                let _ = self.dom_stack.borrow_mut().pop();
                self.dom_stack.borrow_mut().push(Domain::Any);
                Ok(())
            }
            Node::ForEach(data) => {
                self.visit(&mut data.node)?;
                let _ = self.dom_stack.borrow_mut().pop();
                for c in &mut data.children {
                    self.visit(c)?;
                }
                self.dom_stack.borrow_mut().push(Domain::Any);
                Ok(())
            }
            Node::Spot(_) | Node::Df(_) | Node::RateIndex(_) | Node::String(_) => {
                self.dom_stack.borrow_mut().push(Domain::Any);
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scripting::visitors::{ifprocessor::IfProcessor, varindexer::VarIndexer};

    #[test]
    fn test_constant_propagation() {
        let script = "x = 1; y = x + 1;";
        let mut expr = Node::try_from(script).unwrap();

        let indexer = VarIndexer::new();
        indexer.visit(&mut expr).unwrap();

        let ifp = IfProcessor::new();
        ifp.visit(&mut expr).unwrap();

        let dp = DomainProcessor::new(2);
        dp.visit(&mut expr).unwrap();

        let domains = dp.variable_domains();
        assert_eq!(domains, vec![Domain::Constant(1.0), Domain::Constant(2.0)]);
    }
}
