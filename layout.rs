//! Turns a styled DOM tree into a simple visual display list.

use crate::dom::NodeType;
use crate::parser::Parser;
use crate::style::{parse_css, style_tree, Color, StyledNode, Unit, Value};

#[derive(Clone, Debug)]
pub struct VisualBlock {
    pub text: String,
    pub color: Color,
    pub font_size: f32,
    pub margin: f32,
    pub heading: bool,
}

pub fn layout_document(html: &str, css: &str) -> Vec<VisualBlock> {
    let mut parser = Parser::new(html.to_string());
    let nodes = parser.parse_nodes();
    let root = nodes.into_iter().next().expect("document root");
    let stylesheet = parse_css(css.to_string());
    let styled = style_tree(&root, &stylesheet);
    let mut blocks = Vec::new();
    collect(&styled, false, &mut blocks);
    blocks
}

fn collect(node: &StyledNode<'_>, heading: bool, blocks: &mut Vec<VisualBlock>) {
    match &node.node.node_type {
        NodeType::Text(text) => {
            let trimmed = text.trim();
            if trimmed.is_empty() {
                return;
            }
            blocks.push(VisualBlock {
                text: trimmed.to_string(),
                color: color_value(node),
                font_size: length_value(node, "font-size", 16.0),
                margin: length_value(node, "margin", 0.0),
                heading,
            });
        }
        NodeType::Element(element) => {
            let child_heading = matches!(element.tag_name.as_str(), "h1" | "h2" | "h3");
            for child in &node.children {
                collect(child, child_heading, blocks);
            }
        }
    }
}

fn color_value(node: &StyledNode<'_>) -> Color {
    match node.value("color") {
        Some(Value::ColorValue(color)) => *color,
        _ => Color {
            r: 29,
            g: 29,
            b: 32,
            a: 255,
        },
    }
}

fn length_value(node: &StyledNode<'_>, name: &str, fallback: f32) -> f32 {
    match node.value(name) {
        Some(Value::Length(value, Unit::Px)) => *value,
        _ => fallback,
    }
}
