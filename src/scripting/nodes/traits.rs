use crate::scripting::nodes::node::Node;

pub trait NodeVisitor {
    type Output;
    fn visit(&self, node: &mut Node) -> Self::Output;
}

pub trait NodeConstVisitor {
    type Output;
    fn const_visit(&self, node: &Node) -> Self::Output;
}

pub trait Visitable {
    type Output;
    fn accept(&mut self, visitor: &impl NodeVisitor) -> Self::Output;
}

pub trait ConstVisitable {
    type Output;
    fn const_accept(&self, visitor: &impl NodeConstVisitor) -> Self::Output;
}
