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

use crate::{
    currencies::currency::Currency,
    scripting::nodes::traits::{ConstVisitable, NodeConstVisitor, NodeVisitor, Visitable},
    time::date::Date,
};

// pub type ExprTree = Box<Node>;

/// Common mutable-child access for AST node payloads.
pub trait HasChildren {
    /// Returns the child-node vector.
    fn children(&mut self) -> &mut Vec<Node>;
}

#[derive(Default, Debug, Clone, PartialEq)]
/// Payload for a node containing only ordered children.
pub struct NodeData {
    /// Ordered child expressions.
    pub children: Vec<Node>,
}

impl HasChildren for NodeData {
    fn children(&mut self) -> &mut Vec<Node> {
        &mut self.children
    }
}

#[derive(Default, Debug, Clone, PartialEq)]
/// Payload for a Boolean operator and its static truth analysis.
pub struct BoolData {
    /// Boolean operands.
    pub children: Vec<Node>,
    /// Whether static analysis proved the expression true.
    pub always_true: bool,
    /// Whether static analysis proved the expression false.
    pub always_false: bool,
}

impl HasChildren for BoolData {
    fn children(&mut self) -> &mut Vec<Node> {
        &mut self.children
    }
}

#[derive(Default, Debug, Clone, PartialEq)]
/// Payload for a comparison, including smoothing metadata.
pub struct CompData {
    /// Left- and right-hand expressions.
    pub children: Vec<Node>,
    /// Whether static analysis proved the comparison true.
    pub always_true: bool,
    /// Whether static analysis proved the comparison false.
    pub always_false: bool,
    /// Whether the comparison should remain discrete.
    pub discrete: bool,
    /// Smoothing width used by fuzzy evaluation.
    pub eps: f64,
    /// Lower smoothing boundary.
    pub lb: f64,
    /// Upper smoothing boundary.
    pub rb: f64,
}

impl HasChildren for CompData {
    fn children(&mut self) -> &mut Vec<Node> {
        &mut self.children
    }
}

#[derive(Default, Debug, Clone, PartialEq)]
/// Payload for an arithmetic expression and its constant-folding result.
pub struct ExprData {
    /// Ordered operand expressions.
    pub children: Vec<Node>,
    /// Whether static analysis proved the expression constant.
    pub is_constant: bool,
    /// Constant value when `is_constant` is true.
    pub const_value: f64,
}

impl HasChildren for ExprData {
    fn children(&mut self) -> &mut Vec<Node> {
        &mut self.children
    }
}

#[derive(Default, Debug, Clone, PartialEq)]
/// Variable or numeric-literal metadata.
pub struct VarData {
    /// Source-level variable name or literal spelling.
    pub name: String,
    /// Runtime variable-slot index.
    pub id: Option<usize>,
    /// Child expressions attached during parsing or transformation.
    pub children: Vec<Node>,
    /// Whether this node represents a compile-time constant.
    pub is_constant: bool,
    /// Compile-time numeric value.
    pub const_value: f64,
    /// Whether static analysis proved the value logically true.
    pub always_true: bool,
    /// Whether static analysis proved the value logically false.
    pub always_false: bool,
}

#[derive(Default, Debug, Clone, PartialEq)]
/// Conditional-block metadata.
pub struct IfData {
    /// Condition followed by then/else branch statements.
    pub children: Vec<Node>,
    /// Index of the first else-branch statement, when present.
    pub first_else: Option<usize>,
    /// Runtime variable slots written by either branch.
    pub affected_vars: Vec<usize>,
}

impl HasChildren for IfData {
    fn children(&mut self) -> &mut Vec<Node> {
        &mut self.children
    }
}

#[derive(Debug, Clone, PartialEq)]
/// Underlying referenced by a `Spot` expression.
pub enum SpotUnderlying {
    /// FX spot `first`/`second` (price of one unit of `first` in `second`).
    Fx {
        /// Base currency.
        first: Currency,
        /// Quote currency.
        second: Currency,
    },
    /// Equity spot identified by ticker / index name.
    Equity(String),
}

#[derive(Debug, Clone, PartialEq)]
/// Metadata for a spot-market observation.
pub struct SpotData {
    /// Observed FX pair or equity.
    pub underlying: SpotUnderlying,
    /// Observation date; `None` means the current event date.
    pub date: Option<Date>,
    /// Indexed market-response slot.
    pub id: Option<usize>,
}

#[derive(Default, Debug, Clone, PartialEq)]
/// Metadata for a discount-factor request.
pub struct DfData {
    /// Discount-factor maturity date.
    pub date: Date,
    /// Optional curve identifier; local discounting is used when absent.
    pub curve: Option<String>,
    /// Indexed market-response slot.
    pub id: Option<usize>,
}

#[derive(Default, Debug, Clone, PartialEq)]
/// Metadata for a forward-rate index observation.
pub struct RateIndexData {
    /// Market-index identifier.
    pub name: String,
    /// Accrual start date.
    pub start: Date,
    /// Accrual end date.
    pub end: Date,
    /// Indexed market-response slot.
    pub id: Option<usize>,
}

#[derive(Default, Debug, Clone, PartialEq)]
/// Metadata for a discounted payment expression.
pub struct PaysData {
    /// Paid amount expression and optional supporting children.
    pub children: Vec<Node>,
    /// Payment date; `None` uses the current event date.
    pub date: Option<Date>,
    /// Payment currency; `None` uses the local currency.
    pub currency: Option<Currency>,
    /// Payment identifier assigned during indexing.
    pub id: Option<usize>,
    /// Discount-factor response slot.
    pub df_id: Option<usize>,
    /// FX conversion response slot.
    pub spot_id: Option<usize>,
}

impl HasChildren for PaysData {
    fn children(&mut self) -> &mut Vec<Node> {
        &mut self.children
    }
}

#[derive(Default, Debug, Clone, PartialEq)]
/// Metadata for a `for each` loop.
pub struct ForEachData {
    /// Loop-variable name.
    pub var: String,
    /// Runtime loop-variable slot.
    pub id: Option<usize>,
    /// Loop-body statements.
    pub children: Vec<Node>,
    /// Iterable expression.
    pub node: Box<Node>,
}

impl HasChildren for ForEachData {
    fn children(&mut self) -> &mut Vec<Node> {
        &mut self.children
    }
}

#[derive(Default, Debug, Clone, PartialEq)]
/// Metadata for array indexing.
pub struct IndexData {
    /// Indexed collection expression.
    pub children: Vec<Node>,
    /// Index expression.
    pub index: Box<Node>,
}

impl HasChildren for IndexData {
    fn children(&mut self) -> &mut Vec<Node> {
        &mut self.children
    }
}

#[derive(Default, Debug, Clone, PartialEq)]
/// Abstract syntax tree node for the payoff scripting language.
pub enum Node {
    /// Root or grouping node.
    Base(NodeData),

    // variables
    /// Runtime variable reference.
    Variable(VarData),
    /// Numeric literal.
    Constant(VarData),
    /// String literal.
    String(String),

    // financial
    /// FX or equity spot observation.
    Spot(SpotData),
    /// Discount-factor observation.
    Df(DfData),
    /// Forward-rate index observation.
    RateIndex(RateIndexData),
    /// Discounted payment into an accumulator.
    Pays(PaysData),

    // math
    /// Addition expression.
    Add(NodeData),
    /// Subtraction expression.
    Subtract(NodeData),
    /// Multiplication expression.
    Multiply(NodeData),
    /// Division expression.
    Divide(NodeData),
    /// Variable assignment.
    Assign(NodeData),
    /// Minimum of two values.
    Min(NodeData),
    /// Maximum of two values.
    Max(NodeData),
    /// Exponential function.
    Exp(NodeData),
    /// Power function.
    Pow(NodeData),
    /// Natural logarithm.
    Ln(NodeData),
    /// Explicit differentiable conditional function.
    Fif(NodeData),
    /// Day-count year fraction.
    Cvg(NodeData),
    /// Array append operation.
    Append(NodeData),
    /// Arithmetic mean.
    Mean(NodeData),
    /// Standard deviation.
    Std(NodeData),
    /// Array index operation.
    Index(IndexData),

    // unary
    /// Unary plus.
    UnaryPlus(NodeData),
    /// Unary minus.
    UnaryMinus(NodeData),

    // logic
    /// Boolean true literal.
    #[default]
    True,
    /// Boolean false literal.
    False,

    /// Equality comparison.
    Equal(CompData),
    /// Inequality comparison.
    NotEqual(CompData),
    /// Greater-than comparison.
    Superior(CompData),
    /// Greater-than-or-equal comparison.
    SuperiorOrEqual(CompData),

    /// Logical conjunction.
    And(BoolData),
    /// Logical disjunction.
    Or(BoolData),
    /// Logical negation.
    Not(BoolData),

    /// Less-than comparison.
    Inferior(CompData),
    /// Less-than-or-equal comparison.
    InferiorOrEqual(CompData),

    // control flow
    /// Conditional statement.
    If(IfData),
    /// Loop over an iterable value.
    ForEach(ForEachData),

    // iterable
    /// Integer range expression.
    Range(NodeData),
    /// Array literal.
    List(NodeData),
}

impl Node {
    /// Creates an empty root node.
    pub fn new_base() -> Node {
        Node::Base(NodeData::default())
    }

    /// Creates an empty addition node.
    pub fn new_add() -> Node {
        Node::Add(NodeData::default())
    }

    /// Creates an empty subtraction node.
    pub fn new_subtract() -> Node {
        Node::Subtract(NodeData::default())
    }

    /// Creates an empty multiplication node.
    pub fn new_multiply() -> Node {
        Node::Multiply(NodeData::default())
    }

    /// Creates an empty division node.
    pub fn new_divide() -> Node {
        Node::Divide(NodeData::default())
    }

    /// Creates a string-literal node.
    pub fn new_string(value: String) -> Node {
        Node::String(value)
    }

    /// Creates an assignment from `right` to `left`.
    pub fn new_asign_with_values(left: Node, right: Node) -> Node {
        let mut node = Node::Assign(NodeData::default());
        node.add_child(left);
        node.add_child(right);
        node
    }

    /// Creates an addition with two operands.
    pub fn new_add_with_values(left: Node, right: Node) -> Node {
        let mut node = Node::Add(NodeData::default());
        node.add_child(left);
        node.add_child(right);
        node
    }
    /// Creates a subtraction with two operands.
    pub fn new_subtract_with_values(left: Node, right: Node) -> Node {
        let mut node = Node::Subtract(NodeData::default());
        node.add_child(left);
        node.add_child(right);
        node
    }
    /// Creates a multiplication with two operands.
    pub fn new_multiply_with_values(left: Node, right: Node) -> Node {
        let mut node = Node::Multiply(NodeData::default());
        node.add_child(left);
        node.add_child(right);
        node
    }
    /// Creates a division with two operands.
    pub fn new_divide_with_values(left: Node, right: Node) -> Node {
        let mut node = Node::Divide(NodeData::default());
        node.add_child(left);
        node.add_child(right);
        node
    }

    /// Creates an unindexed variable reference.
    pub fn new_variable(name: String) -> Node {
        Node::Variable(VarData {
            name,
            id: None,
            is_constant: false,
            const_value: 0.0,
            children: Vec::new(),
            always_true: false,
            always_false: false,
        })
    }

    /// Creates a variable reference assigned to runtime slot `id`.
    pub fn new_variable_with_id(name: String, id: usize) -> Node {
        Node::Variable(VarData {
            name,
            id: Some(id),
            is_constant: false,
            const_value: 0.0,
            children: Vec::new(),
            always_true: false,
            always_false: false,
        })
    }

    /// Creates an empty minimum node.
    pub fn new_min() -> Node {
        Node::Min(NodeData::default())
    }

    /// Creates an empty maximum node.
    pub fn new_max() -> Node {
        Node::Max(NodeData::default())
    }

    /// Creates an empty exponential node.
    pub fn new_exp() -> Node {
        Node::Exp(NodeData::default())
    }

    /// Creates an empty natural-logarithm node.
    pub fn new_ln() -> Node {
        Node::Ln(NodeData::default())
    }

    /// Creates an empty differentiable-conditional node.
    pub fn new_fif() -> Node {
        Node::Fif(NodeData::default())
    }

    /// Creates an empty power node.
    pub fn new_pow() -> Node {
        Node::Pow(NodeData::default())
    }

    /// Creates an empty day-count coverage node.
    pub fn new_cvg() -> Node {
        Node::Cvg(NodeData::default())
    }

    /// Creates an empty array-append node.
    pub fn new_append() -> Node {
        Node::Append(NodeData::default())
    }

    /// Creates an empty mean node.
    pub fn new_mean() -> Node {
        Node::Mean(NodeData::default())
    }

    /// Creates an empty standard-deviation node.
    pub fn new_std() -> Node {
        Node::Std(NodeData::default())
    }

    /// Creates an empty array-index node.
    pub fn new_index() -> Node {
        Node::Index(IndexData::default())
    }

    /// Creates an array-index node for a collection and index expression.
    pub fn new_index_with_values(children: Vec<Node>, index: Node) -> Node {
        Node::Index(IndexData {
            children,
            index: Box::new(index),
        })
    }

    /// Creates a numeric-literal node.
    pub fn new_constant(value: f64) -> Node {
        Node::Constant(VarData {
            name: value.to_string(),
            id: None,
            is_constant: true,
            const_value: value,
            children: Vec::new(),
            always_true: false,
            always_false: false,
        })
    }

    /// Creates a logical conjunction with two operands.
    pub fn new_and_with_values(left: Node, right: Node) -> Node {
        let mut node = Node::And(BoolData::default());
        node.add_child(left);
        node.add_child(right);
        node
    }

    /// Creates a logical disjunction with two operands.
    pub fn new_or_with_values(left: Node, right: Node) -> Node {
        let mut node = Node::Or(BoolData::default());
        node.add_child(left);
        node.add_child(right);
        node
    }

    /// Creates a logical negation around one operand.
    pub fn new_not_with_value(value: Node) -> Node {
        let mut node = Node::Not(BoolData::default());
        node.add_child(value);
        node
    }

    /// Creates an equality comparison.
    pub fn new_equal_with_values(left: Node, right: Node) -> Node {
        let mut node = Node::Equal(CompData::default());
        node.add_child(left);
        node.add_child(right);
        node
    }

    /// Creates an inequality comparison.
    pub fn new_not_equal_with_values(left: Node, right: Node) -> Node {
        let mut node = Node::NotEqual(CompData::default());
        node.add_child(left);
        node.add_child(right);
        node
    }

    /// Creates a greater-than comparison.
    pub fn new_superior_with_values(left: Node, right: Node) -> Node {
        let mut node = Node::Superior(CompData::default());
        node.add_child(left);
        node.add_child(right);
        node
    }

    /// Creates a less-than comparison.
    pub fn new_inferior_with_values(left: Node, right: Node) -> Node {
        let mut node = Node::Inferior(CompData::default());
        node.add_child(left);
        node.add_child(right);
        node
    }

    /// Creates a greater-than-or-equal comparison.
    pub fn new_superior_or_equal_with_values(left: Node, right: Node) -> Node {
        let mut node = Node::SuperiorOrEqual(CompData::default());
        node.add_child(left);
        node.add_child(right);
        node
    }

    /// Creates a less-than-or-equal comparison.
    pub fn new_inferior_or_equal_with_values(left: Node, right: Node) -> Node {
        let mut node = Node::InferiorOrEqual(CompData::default());
        node.add_child(left);
        node.add_child(right);
        node
    }

    /// Creates an array literal from `values`.
    pub fn new_list_with_values(values: Vec<Node>) -> Node {
        Node::List(NodeData { children: values })
    }

    /// Creates an operation that appends `value` to `list`.
    pub fn new_append_with_values(list: Node, value: Node) -> Node {
        let mut node = Node::Append(NodeData::default());
        node.add_child(list);
        node.add_child(value);
        node
    }

    /// Creates a mean operation over an array expression.
    pub fn new_mean_with_values(values: Node) -> Node {
        let mut node = Node::Mean(NodeData::default());
        node.add_child(values);
        node
    }

    /// Creates a standard-deviation operation over an array expression.
    pub fn new_std_with_values(values: Node) -> Node {
        let mut node = Node::Std(NodeData::default());
        node.add_child(values);
        node
    }

    /// Creates an empty assignment node.
    pub fn new_assign() -> Node {
        Node::Assign(NodeData::default())
    }

    /// Creates an empty logical-conjunction node.
    pub fn new_and() -> Node {
        Node::And(BoolData::default())
    }

    /// Creates an empty logical-disjunction node.
    pub fn new_or() -> Node {
        Node::Or(BoolData::default())
    }

    /// Creates an empty logical-negation node.
    pub fn new_not() -> Node {
        Node::Not(BoolData::default())
    }

    /// Creates an empty greater-than node.
    pub fn new_superior() -> Node {
        Node::Superior(CompData::default())
    }

    /// Creates an empty less-than node.
    pub fn new_inferior() -> Node {
        Node::Inferior(CompData::default())
    }

    /// Creates an empty greater-than-or-equal node.
    pub fn new_superior_or_equal() -> Node {
        Node::SuperiorOrEqual(CompData::default())
    }

    /// Creates an empty equality node.
    pub fn new_equal() -> Node {
        Node::Equal(CompData::default())
    }

    /// Creates an empty conditional node.
    pub fn new_if() -> Node {
        Node::If(IfData::default())
    }

    /// Creates an empty unary-plus node.
    pub fn new_unary_plus() -> Node {
        Node::UnaryPlus(NodeData::default())
    }

    /// Creates an empty unary-minus node.
    pub fn new_unary_minus() -> Node {
        Node::UnaryMinus(NodeData::default())
    }

    /// Creates an empty less-than-or-equal node.
    pub fn new_inferior_or_equal() -> Node {
        Node::InferiorOrEqual(CompData::default())
    }

    /// Creates an empty inequality node.
    pub fn new_not_equal() -> Node {
        Node::NotEqual(CompData::default())
    }

    /// Creates a Boolean true literal.
    pub fn new_true() -> Node {
        Node::True
    }

    /// Creates a Boolean false literal.
    pub fn new_false() -> Node {
        Node::False
    }

    /// Creates an empty payment node.
    pub fn new_pays() -> Node {
        Node::Pays(PaysData::default())
    }

    /// Creates an FX spot observation.
    pub fn new_spot(first: Currency, second: Currency, date: Option<Date>) -> Node {
        Node::Spot(SpotData {
            underlying: SpotUnderlying::Fx { first, second },
            date,
            id: None,
        })
    }

    /// Creates an equity spot observation.
    pub fn new_equity_spot(name: String, date: Option<Date>) -> Node {
        Node::Spot(SpotData {
            underlying: SpotUnderlying::Equity(name),
            date,
            id: None,
        })
    }

    /// Creates a discount-factor observation.
    pub fn new_df(date: Date, curve: Option<String>) -> Node {
        Node::Df(DfData {
            date,
            curve,
            id: None,
        })
    }

    /// Creates a forward-rate index observation over an accrual period.
    pub fn new_rate_index(name: String, start: Date, end: Date) -> Node {
        Node::RateIndex(RateIndexData {
            name,
            start,
            end,
            id: None,
        })
    }

    /// Creates a power operation from its base and exponent.
    pub fn new_pow_with_values(base: Node, exponent: Node) -> Node {
        let mut node = Node::Pow(NodeData::default());
        node.add_child(base);
        node.add_child(exponent);
        node
    }

    /// Creates an empty range node.
    pub fn new_range() -> Node {
        Node::Range(NodeData::default())
    }

    /// Creates an empty array-literal node.
    pub fn new_list() -> Node {
        Node::List(NodeData::default())
    }

    /// Creates a loop over `node` using `var` and the supplied body.
    pub fn new_for_each(var: String, node: Box<Node>, children: Vec<Node>) -> Node {
        Node::ForEach(ForEachData {
            var,
            children,
            node,
            id: None,
        })
    }

    /// Appends a child expression to a composite node.
    ///
    /// # Panics
    /// Panics when called on a leaf node or directly on a Boolean literal.
    pub fn add_child(&mut self, child: Node) {
        match self {
            Node::Base(inner) => inner.children.push(child),
            Node::Add(data) => data.children.push(child),
            Node::Subtract(data) => data.children.push(child),
            Node::Multiply(data) => data.children.push(child),
            Node::Divide(data) => data.children.push(child),
            Node::Variable(data) => data.children.push(child),
            Node::Assign(data) => data.children.push(child),
            Node::And(data) => data.children.push(child),
            Node::Or(data) => data.children.push(child),
            Node::Not(data) => data.children.push(child),
            Node::Superior(data) => data.children.push(child),
            Node::Inferior(data) => data.children.push(child),
            Node::SuperiorOrEqual(data) => data.children.push(child),
            Node::InferiorOrEqual(data) => data.children.push(child),
            Node::Equal(data) => data.children.push(child),
            Node::If(data) => data.children.push(child),
            Node::UnaryPlus(data) => data.children.push(child),
            Node::UnaryMinus(data) => data.children.push(child),
            Node::Min(data) => data.children.push(child),
            Node::Max(data) => data.children.push(child),
            Node::Exp(data) => data.children.push(child),
            Node::Ln(data) => data.children.push(child),
            Node::Fif(data) => data.children.push(child),
            Node::Pow(data) => data.children.push(child),
            Node::Cvg(data) => data.children.push(child),
            Node::Append(data) => data.children.push(child),
            Node::Mean(data) => data.children.push(child),
            Node::Std(data) => data.children.push(child),
            Node::Index(data) => data.children.push(child),
            Node::NotEqual(data) => data.children.push(child),
            Node::Pays(data) => data.children.push(child),
            Node::ForEach(data) => data.children.push(child),
            Node::Range(data) => data.children.push(child),
            Node::List(data) => data.children.push(child),
            Node::Spot(_) => panic!("Cannot add child to spot node"),
            Node::Df(_) => panic!("Cannot add child to df node"),
            Node::RateIndex(_) => panic!("Cannot add child to rate index node"),
            Node::True => panic!("Cannot add child to true node"),
            Node::False => panic!("Cannot add child to false node"),
            Node::Constant(_) => panic!("Cannot add child to constant node"),
            Node::String(_) => panic!("Cannot add child to string node"),
        }
    }

    /// Returns the children of a composite node.
    ///
    /// # Panics
    /// Panics for leaf nodes and conditional nodes whose branches require
    /// structured access.
    pub fn children(&self) -> &Vec<Node> {
        match self {
            Node::Base(data) => &data.children,
            Node::Add(data) => &data.children,
            Node::Subtract(data) => &data.children,
            Node::Multiply(data) => &data.children,
            Node::Divide(data) => &data.children,
            Node::Variable(data) => &data.children,
            Node::Assign(data) => &data.children,
            Node::And(data) => &data.children,
            Node::Or(data) => &data.children,
            Node::Not(data) => &data.children,
            Node::Superior(data) => &data.children,
            Node::Inferior(data) => &data.children,
            Node::SuperiorOrEqual(data) => &data.children,
            Node::InferiorOrEqual(data) => &data.children,
            Node::Equal(data) => &data.children,
            Node::If(_) => panic!("Cannot get children from if node directly"),
            Node::UnaryPlus(data) => &data.children,
            Node::UnaryMinus(data) => &data.children,
            Node::Min(data) => &data.children,
            Node::Max(data) => &data.children,
            Node::Exp(data) => &data.children,
            Node::Ln(data) => &data.children,
            Node::Fif(data) => &data.children,
            Node::Pow(data) => &data.children,
            Node::Cvg(data) => &data.children,
            Node::Append(data) => &data.children,
            Node::Mean(data) => &data.children,
            Node::Std(data) => &data.children,
            Node::Index(data) => &data.children,
            Node::NotEqual(data) => &data.children,
            Node::Pays(data) => &data.children,
            Node::Range(data) => &data.children,
            Node::List(data) => &data.children,
            Node::ForEach(data) => &data.children,
            Node::Spot(_) => panic!("Cannot get children from spot node"),
            Node::Df(_) => panic!("Cannot get children from df node"),
            Node::RateIndex(_) => {
                panic!("Cannot get children from rate index node")
            }
            Node::True => panic!("Cannot get children from true node"),
            Node::False => panic!("Cannot get children from false node"),
            Node::Constant(_) => panic!("Cannot get children from constant node"),
            Node::String(_) => panic!("Cannot get children from string node"),
        }
    }

    /// Returns mutable access to the children of a composite node.
    ///
    /// # Panics
    /// Panics for leaf nodes and conditional nodes whose branches require
    /// structured access.
    pub fn children_mut(&mut self) -> &mut Vec<Node> {
        match self {
            Node::Base(data) => &mut data.children,
            Node::Add(data) => &mut data.children,
            Node::Subtract(data) => &mut data.children,
            Node::Multiply(data) => &mut data.children,
            Node::Divide(data) => &mut data.children,
            Node::Variable(data) => &mut data.children,
            Node::Assign(data) => &mut data.children,
            Node::And(data) => &mut data.children,
            Node::Or(data) => &mut data.children,
            Node::Not(data) => &mut data.children,
            Node::Superior(data) => &mut data.children,
            Node::Inferior(data) => &mut data.children,
            Node::SuperiorOrEqual(data) => &mut data.children,
            Node::InferiorOrEqual(data) => &mut data.children,
            Node::Equal(data) => &mut data.children,
            Node::If(_) => panic!("Cannot get children from if node directly"),
            Node::UnaryPlus(data) => &mut data.children,
            Node::UnaryMinus(data) => &mut data.children,
            Node::Min(data) => &mut data.children,
            Node::Max(data) => &mut data.children,
            Node::Exp(data) => &mut data.children,
            Node::Ln(data) => &mut data.children,
            Node::Fif(data) => &mut data.children,
            Node::Pow(data) => &mut data.children,
            Node::Cvg(data) => &mut data.children,
            Node::Append(data) => &mut data.children,
            Node::Mean(data) => &mut data.children,
            Node::Std(data) => &mut data.children,
            Node::Index(data) => &mut data.children,
            Node::NotEqual(data) => &mut data.children,
            Node::Pays(data) => &mut data.children,
            Node::Range(data) => &mut data.children,
            Node::List(data) => &mut data.children,
            Node::ForEach(data) => &mut data.children,
            Node::Spot(_) => panic!("Cannot get children from spot node"),
            Node::Df(_) => panic!("Cannot get children from df node"),
            Node::RateIndex(_) => {
                panic!("Cannot get children from rate index node")
            }
            Node::True => panic!("Cannot get children from true node"),
            Node::False => panic!("Cannot get children from false node"),
            Node::Constant(_) => panic!("Cannot get children from constant node"),
            Node::String(_) => panic!("Cannot get children from string node"),
        }
    }
}

impl Visitable for Node {
    type Output = ();
    fn accept(&mut self, visitor: &impl NodeVisitor) {
        visitor.visit(self);
    }
}

impl ConstVisitable for Node {
    type Output = ();
    fn const_accept(&self, visitor: &impl NodeConstVisitor) {
        visitor.const_visit(self);
    }
}
