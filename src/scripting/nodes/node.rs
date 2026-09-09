use crate::{
    currencies::currency::Currency,
    scripting::nodes::traits::{ConstVisitable, NodeConstVisitor, NodeVisitor, Visitable},
    time::date::Date,
};

// pub type ExprTree = Box<Node>;

pub trait HasChildren {
    fn children(&mut self) -> &mut Vec<Node>;
}

#[derive(Default, Debug, Clone, PartialEq)]
pub struct NodeData {
    pub children: Vec<Node>,
}

impl HasChildren for NodeData {
    fn children(&mut self) -> &mut Vec<Node> {
        &mut self.children
    }
}

#[derive(Default, Debug, Clone, PartialEq)]
pub struct BoolData {
    pub children: Vec<Node>,
    pub always_true: bool,
    pub always_false: bool,
}

impl HasChildren for BoolData {
    fn children(&mut self) -> &mut Vec<Node> {
        &mut self.children
    }
}

#[derive(Default, Debug, Clone, PartialEq)]
pub struct CompData {
    pub children: Vec<Node>,
    pub always_true: bool,
    pub always_false: bool,
    pub discrete: bool,
    pub eps: f64,
    pub lb: f64,
    pub rb: f64,
}

impl HasChildren for CompData {
    fn children(&mut self) -> &mut Vec<Node> {
        &mut self.children
    }
}

#[derive(Default, Debug, Clone, PartialEq)]
pub struct ExprData {
    pub children: Vec<Node>,
    pub is_constant: bool,
    pub const_value: f64,
}

impl HasChildren for ExprData {
    fn children(&mut self) -> &mut Vec<Node> {
        &mut self.children
    }
}

#[derive(Default, Debug, Clone, PartialEq)]
pub struct VarData {
    pub name: String,
    pub id: Option<usize>,
    pub children: Vec<Node>,
    pub is_constant: bool,
    pub const_value: f64,
    pub always_true: bool,
    pub always_false: bool,
}

#[derive(Default, Debug, Clone, PartialEq)]
pub struct IfData {
    pub children: Vec<Node>,
    pub first_else: Option<usize>,
    pub affected_vars: Vec<usize>,
}

impl HasChildren for IfData {
    fn children(&mut self) -> &mut Vec<Node> {
        &mut self.children
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum SpotUnderlying {
    /// FX spot `first`/`second` (price of one unit of `first` in `second`).
    Fx { first: Currency, second: Currency },
    /// Equity spot identified by ticker / index name.
    Equity(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct SpotData {
    pub underlying: SpotUnderlying,
    pub date: Option<Date>,
    pub id: Option<usize>,
}

#[derive(Default, Debug, Clone, PartialEq)]
pub struct DfData {
    pub date: Date,
    pub curve: Option<String>,
    pub id: Option<usize>,
}

#[derive(Default, Debug, Clone, PartialEq)]
pub struct RateIndexData {
    pub name: String,
    pub start: Date,
    pub end: Date,
    pub id: Option<usize>,
}

#[derive(Default, Debug, Clone, PartialEq)]
pub struct PaysData {
    pub children: Vec<Node>,
    pub date: Option<Date>,
    pub currency: Option<Currency>,
    pub id: Option<usize>,
    pub df_id: Option<usize>,
    pub spot_id: Option<usize>,
}

impl HasChildren for PaysData {
    fn children(&mut self) -> &mut Vec<Node> {
        &mut self.children
    }
}

#[derive(Default, Debug, Clone, PartialEq)]
pub struct ForEachData {
    pub var: String,
    pub id: Option<usize>,
    pub children: Vec<Node>,
    pub node: Box<Node>,
}

impl HasChildren for ForEachData {
    fn children(&mut self) -> &mut Vec<Node> {
        &mut self.children
    }
}

#[derive(Default, Debug, Clone, PartialEq)]
pub struct IndexData {
    pub children: Vec<Node>,
    pub index: Box<Node>,
}

impl HasChildren for IndexData {
    fn children(&mut self) -> &mut Vec<Node> {
        &mut self.children
    }
}

#[derive(Default, Debug, Clone, PartialEq)]
pub enum Node {
    Base(NodeData),

    // variables
    Variable(VarData),
    Constant(VarData),
    String(String),

    // financial
    Spot(SpotData),
    Df(DfData),
    RateIndex(RateIndexData),
    Pays(PaysData),

    // math
    Add(NodeData),
    Subtract(NodeData),
    Multiply(NodeData),
    Divide(NodeData),
    Assign(NodeData),
    Min(NodeData),
    Max(NodeData),
    Exp(NodeData),
    Pow(NodeData),
    Ln(NodeData),
    Fif(NodeData),
    Cvg(NodeData),
    Append(NodeData),
    Mean(NodeData),
    Std(NodeData),
    Index(IndexData),

    // unary
    UnaryPlus(NodeData),
    UnaryMinus(NodeData),

    // logic
    #[default]
    True,
    False,

    Equal(CompData),
    NotEqual(CompData),
    Superior(CompData),
    SuperiorOrEqual(CompData),

    And(BoolData),
    Or(BoolData),
    Not(BoolData),

    Inferior(CompData),
    InferiorOrEqual(CompData),

    // control flow
    If(IfData),
    ForEach(ForEachData),

    // iterable
    Range(NodeData),
    List(NodeData),
}

impl Node {
    pub fn new_base() -> Node {
        Node::Base(NodeData::default())
    }

    pub fn new_add() -> Node {
        Node::Add(NodeData::default())
    }

    pub fn new_subtract() -> Node {
        Node::Subtract(NodeData::default())
    }

    pub fn new_multiply() -> Node {
        Node::Multiply(NodeData::default())
    }

    pub fn new_divide() -> Node {
        Node::Divide(NodeData::default())
    }

    pub fn new_string(value: String) -> Node {
        Node::String(value)
    }

    pub fn new_asign_with_values(left: Node, right: Node) -> Node {
        let mut node = Node::Assign(NodeData::default());
        node.add_child(left);
        node.add_child(right);
        node
    }

    pub fn new_add_with_values(left: Node, right: Node) -> Node {
        let mut node = Node::Add(NodeData::default());
        node.add_child(left);
        node.add_child(right);
        node
    }
    pub fn new_subtract_with_values(left: Node, right: Node) -> Node {
        let mut node = Node::Subtract(NodeData::default());
        node.add_child(left);
        node.add_child(right);
        node
    }
    pub fn new_multiply_with_values(left: Node, right: Node) -> Node {
        let mut node = Node::Multiply(NodeData::default());
        node.add_child(left);
        node.add_child(right);
        node
    }
    pub fn new_divide_with_values(left: Node, right: Node) -> Node {
        let mut node = Node::Divide(NodeData::default());
        node.add_child(left);
        node.add_child(right);
        node
    }

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

    pub fn new_min() -> Node {
        Node::Min(NodeData::default())
    }

    pub fn new_max() -> Node {
        Node::Max(NodeData::default())
    }

    pub fn new_exp() -> Node {
        Node::Exp(NodeData::default())
    }

    pub fn new_ln() -> Node {
        Node::Ln(NodeData::default())
    }

    pub fn new_fif() -> Node {
        Node::Fif(NodeData::default())
    }

    pub fn new_pow() -> Node {
        Node::Pow(NodeData::default())
    }

    pub fn new_cvg() -> Node {
        Node::Cvg(NodeData::default())
    }

    pub fn new_append() -> Node {
        Node::Append(NodeData::default())
    }

    pub fn new_mean() -> Node {
        Node::Mean(NodeData::default())
    }

    pub fn new_std() -> Node {
        Node::Std(NodeData::default())
    }

    pub fn new_index() -> Node {
        Node::Index(IndexData::default())
    }

    pub fn new_index_with_values(children: Vec<Node>, index: Node) -> Node {
        Node::Index(IndexData {
            children,
            index: Box::new(index),
        })
    }

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

    pub fn new_and_with_values(left: Node, right: Node) -> Node {
        let mut node = Node::And(BoolData::default());
        node.add_child(left);
        node.add_child(right);
        node
    }

    pub fn new_or_with_values(left: Node, right: Node) -> Node {
        let mut node = Node::Or(BoolData::default());
        node.add_child(left);
        node.add_child(right);
        node
    }

    pub fn new_not_with_value(value: Node) -> Node {
        let mut node = Node::Not(BoolData::default());
        node.add_child(value);
        node
    }

    pub fn new_equal_with_values(left: Node, right: Node) -> Node {
        let mut node = Node::Equal(CompData::default());
        node.add_child(left);
        node.add_child(right);
        node
    }

    pub fn new_not_equal_with_values(left: Node, right: Node) -> Node {
        let mut node = Node::NotEqual(CompData::default());
        node.add_child(left);
        node.add_child(right);
        node
    }

    pub fn new_superior_with_values(left: Node, right: Node) -> Node {
        let mut node = Node::Superior(CompData::default());
        node.add_child(left);
        node.add_child(right);
        node
    }

    pub fn new_inferior_with_values(left: Node, right: Node) -> Node {
        let mut node = Node::Inferior(CompData::default());
        node.add_child(left);
        node.add_child(right);
        node
    }

    pub fn new_superior_or_equal_with_values(left: Node, right: Node) -> Node {
        let mut node = Node::SuperiorOrEqual(CompData::default());
        node.add_child(left);
        node.add_child(right);
        node
    }

    pub fn new_inferior_or_equal_with_values(left: Node, right: Node) -> Node {
        let mut node = Node::InferiorOrEqual(CompData::default());
        node.add_child(left);
        node.add_child(right);
        node
    }

    pub fn new_list_with_values(values: Vec<Node>) -> Node {
        Node::List(NodeData { children: values })
    }

    pub fn new_append_with_values(list: Node, value: Node) -> Node {
        let mut node = Node::Append(NodeData::default());
        node.add_child(list);
        node.add_child(value);
        node
    }

    pub fn new_mean_with_values(values: Node) -> Node {
        let mut node = Node::Mean(NodeData::default());
        node.add_child(values);
        node
    }

    pub fn new_std_with_values(values: Node) -> Node {
        let mut node = Node::Std(NodeData::default());
        node.add_child(values);
        node
    }

    pub fn new_assign() -> Node {
        Node::Assign(NodeData::default())
    }

    pub fn new_and() -> Node {
        Node::And(BoolData::default())
    }

    pub fn new_or() -> Node {
        Node::Or(BoolData::default())
    }

    pub fn new_not() -> Node {
        Node::Not(BoolData::default())
    }

    pub fn new_superior() -> Node {
        Node::Superior(CompData::default())
    }

    pub fn new_inferior() -> Node {
        Node::Inferior(CompData::default())
    }

    pub fn new_superior_or_equal() -> Node {
        Node::SuperiorOrEqual(CompData::default())
    }

    pub fn new_equal() -> Node {
        Node::Equal(CompData::default())
    }

    pub fn new_if() -> Node {
        Node::If(IfData::default())
    }

    pub fn new_unary_plus() -> Node {
        Node::UnaryPlus(NodeData::default())
    }

    pub fn new_unary_minus() -> Node {
        Node::UnaryMinus(NodeData::default())
    }

    pub fn new_inferior_or_equal() -> Node {
        Node::InferiorOrEqual(CompData::default())
    }

    pub fn new_not_equal() -> Node {
        Node::NotEqual(CompData::default())
    }

    pub fn new_true() -> Node {
        Node::True
    }

    pub fn new_false() -> Node {
        Node::False
    }

    pub fn new_pays() -> Node {
        Node::Pays(PaysData::default())
    }

    pub fn new_spot(first: Currency, second: Currency, date: Option<Date>) -> Node {
        Node::Spot(SpotData {
            underlying: SpotUnderlying::Fx { first, second },
            date,
            id: None,
        })
    }

    pub fn new_equity_spot(name: String, date: Option<Date>) -> Node {
        Node::Spot(SpotData {
            underlying: SpotUnderlying::Equity(name),
            date,
            id: None,
        })
    }

    pub fn new_df(date: Date, curve: Option<String>) -> Node {
        Node::Df(DfData {
            date,
            curve,
            id: None,
        })
    }

    pub fn new_rate_index(name: String, start: Date, end: Date) -> Node {
        Node::RateIndex(RateIndexData {
            name,
            start,
            end,
            id: None,
        })
    }

    pub fn new_pow_with_values(base: Node, exponent: Node) -> Node {
        let mut node = Node::Pow(NodeData::default());
        node.add_child(base);
        node.add_child(exponent);
        node
    }

    pub fn new_range() -> Node {
        Node::Range(NodeData::default())
    }

    pub fn new_list() -> Node {
        Node::List(NodeData::default())
    }

    pub fn new_for_each(var: String, node: Box<Node>, children: Vec<Node>) -> Node {
        Node::ForEach(ForEachData {
            var,
            children,
            node,
            id: None,
        })
    }

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
