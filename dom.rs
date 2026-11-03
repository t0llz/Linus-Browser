use std::collections::HashMap;

#[derive(Debug, PartialEq)]
pub enum NodeType {
    Text(String),
    Element(ElementData),
}

#[derive(Debug, PartialEq)]
pub struct ElementData {
    pub tag_name: String,
    pub attributes: HashMap<String, String>,
}

#[derive(Debug, PartialEq)]
pub struct Node {
    pub children: Vec<Node>,
    pub node_type: NodeType,
}

impl Node {
    pub fn text(data: String) -> Node {
        Node { children: Vec::new(), node_type: NodeType::Text(data) }
    }

    pub fn elem(name: String, attrs: HashMap<String, String>, children: Vec<Node>) -> Node {
        Node { children, node_type: NodeType::Element(ElementData { tag_name: name, attributes: attrs }) }
    }
}
