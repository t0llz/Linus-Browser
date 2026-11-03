//! CSS parser, selector matching, and computed style tree.

use std::collections::{HashMap, HashSet};
use std::fmt;

use crate::dom::{ElementData, Node, NodeType};

pub type PropertyMap = HashMap<String, Value>;
pub type Specificity = (u32, u32, u32);

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Keyword(String),
    Length(f32, Unit),
    ColorValue(Color),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unit {
    Px,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

#[derive(Debug, PartialEq)]
pub struct Stylesheet {
    pub rules: Vec<Rule>,
}

#[derive(Debug, PartialEq)]
pub struct Rule {
    pub selectors: Vec<Selector>,
    pub declarations: Vec<Declaration>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Selector {
    Simple(SimpleSelector),
}

#[derive(Clone, Debug, PartialEq)]
pub struct SimpleSelector {
    pub tag_name: Option<String>,
    pub id: Option<String>,
    pub class: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Declaration {
    pub name: String,
    pub value: Value,
}

#[derive(Clone, Debug, PartialEq)]
pub struct StyledNode<'a> {
    pub node: &'a Node,
    pub specified_values: PropertyMap,
    pub children: Vec<StyledNode<'a>>,
}

impl SimpleSelector {
    pub fn specificity(&self) -> Specificity {
        let a = u32::from(self.id.is_some());
        let b = self.class.len() as u32;
        let c = u32::from(self.tag_name.is_some());
        (a, b, c)
    }
}

impl Selector {
    pub fn specificity(&self) -> Specificity {
        match self {
            Selector::Simple(simple) => simple.specificity(),
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Keyword(keyword) => write!(f, "{keyword}"),
            Value::Length(value, Unit::Px) => write!(f, "{value}px"),
            Value::ColorValue(color) => {
                if color.a == 255 {
                    write!(f, "#{:02x}{:02x}{:02x}", color.r, color.g, color.b)
                } else {
                    write!(
                        f,
                        "#{:02x}{:02x}{:02x}{:02x}",
                        color.r, color.g, color.b, color.a
                    )
                }
            }
        }
    }
}

impl<'a> StyledNode<'a> {
    pub fn value(&self, name: &str) -> Option<&Value> {
        self.specified_values.get(name)
    }
}

impl fmt::Display for StyledNode<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_styled(self, f, 0)
    }
}

fn write_styled(node: &StyledNode<'_>, f: &mut fmt::Formatter<'_>, indent: usize) -> fmt::Result {
    let pad = "  ".repeat(indent);
    match &node.node.node_type {
        NodeType::Text(text) => {
            writeln!(f, "{pad}\"{}\"", text.trim())?;
            write_props(node, f, indent + 1)
        }
            NodeType::Element(element) => {
            write!(f, "{pad}<{}", element.tag_name)?;
            if let Some(class) = element.attributes.get("class") {
                write!(f, " class=\"{class}\"")?;
            }
            if let Some(id) = element.attributes.get("id") {
                write!(f, " id=\"{id}\"")?;
            }
            writeln!(f, ">")?;
            write_props(node, f, indent + 1)?;
            for child in &node.children {
                write_styled(child, f, indent + 1)?;
            }
            Ok(())
        }
    }
}

fn write_props(node: &StyledNode<'_>, f: &mut fmt::Formatter<'_>, indent: usize) -> fmt::Result {
    let pad = "  ".repeat(indent);
    for name in ["color", "font-size", "margin"] {
        if let Some(value) = node.value(name) {
            writeln!(f, "{pad}{name}: {value}")?;
        }
    }
    Ok(())
}

fn element_id(element: &ElementData) -> Option<&str> {
    element.attributes.get("id").map(String::as_str)
}

fn element_classes(element: &ElementData) -> HashSet<&str> {
    match element.attributes.get("class") {
        Some(classlist) => classlist.split_whitespace().collect(),
        None => HashSet::new(),
    }
}

fn matches_simple(element: &ElementData, selector: &SimpleSelector) -> bool {
    if let Some(tag) = &selector.tag_name {
        if tag != &element.tag_name {
            return false;
        }
    }
    if let Some(id) = &selector.id {
        if element_id(element) != Some(id.as_str()) {
            return false;
        }
    }
    let classes = element_classes(element);
    selector
        .class
        .iter()
        .all(|class_name| classes.contains(class_name.as_str()))
}

fn matches(element: &ElementData, selector: &Selector) -> bool {
    match selector {
        Selector::Simple(simple) => matches_simple(element, simple),
    }
}

fn matching_rules<'a>(element: &ElementData, stylesheet: &'a Stylesheet) -> Vec<(Specificity, &'a Rule)> {
    stylesheet
        .rules
        .iter()
        .filter_map(|rule| {
            rule.selectors
                .iter()
                .filter(|selector| matches(element, selector))
                .map(Selector::specificity)
                .max()
                .map(|specificity| (specificity, rule))
        })
        .collect()
}

fn specified_values(element: &ElementData, stylesheet: &Stylesheet, parent: Option<&PropertyMap>) -> PropertyMap {
    let mut values = PropertyMap::new();

    if let Some(parent_values) = parent {
        if let Some(color) = parent_values.get("color") {
            values.insert("color".to_string(), color.clone());
        }
        if let Some(font_size) = parent_values.get("font-size") {
            values.insert("font-size".to_string(), font_size.clone());
        }
    } else {
        values.insert(
            "color".to_string(),
            Value::ColorValue(Color {
                r: 0,
                g: 0,
                b: 0,
                a: 255,
            }),
        );
        values.insert("font-size".to_string(), Value::Length(16.0, Unit::Px));
    }
    values.insert("margin".to_string(), Value::Length(0.0, Unit::Px));

    let mut rules = matching_rules(element, stylesheet);
    rules.sort_by(|a, b| a.0.cmp(&b.0));
    for (_, rule) in rules {
        for declaration in &rule.declarations {
            values.insert(declaration.name.clone(), declaration.value.clone());
        }
    }
    values
}

pub fn style_tree<'a>(root: &'a Node, stylesheet: &Stylesheet) -> StyledNode<'a> {
    style_from(root, stylesheet, None)
}

fn style_from<'a>(node: &'a Node, stylesheet: &Stylesheet, parent: Option<&PropertyMap>) -> StyledNode<'a> {
    let specified_values = match &node.node_type {
        NodeType::Element(element) => specified_values(element, stylesheet, parent),
        NodeType::Text(_) => parent.cloned().unwrap_or_default(),
    };
    let children = node
        .children
        .iter()
        .map(|child| style_from(child, stylesheet, Some(&specified_values)))
        .collect();
    StyledNode {
        node,
        specified_values,
        children,
    }
}

pub fn parse_css(source: impl Into<String>) -> Stylesheet {
    CssParser::new(source.into()).parse_stylesheet()
}

struct CssParser {
    pos: usize,
    input: String,
}

impl CssParser {
    fn new(input: String) -> CssParser {
        CssParser { pos: 0, input }
    }

    fn eof(&self) -> bool {
        self.pos >= self.input.len()
    }

    fn next_char(&self) -> char {
        self.input[self.pos..].chars().next().unwrap()
    }

    fn consume_char(&mut self) -> char {
        let mut iter = self.input[self.pos..].char_indices();
        let (_, current) = iter.next().unwrap();
        let (next_pos, _) = iter.next().unwrap_or((self.input.len() - self.pos, ' '));
        self.pos += next_pos;
        current
    }

    fn consume_while<F>(&mut self, test: F) -> String
    where
        F: Fn(char) -> bool,
    {
        let mut result = String::new();
        while !self.eof() && test(self.next_char()) {
            result.push(self.consume_char());
        }
        result
    }

    fn consume_whitespace(&mut self) {
        self.consume_while(char::is_whitespace);
    }

    fn parse_stylesheet(&mut self) -> Stylesheet {
        let mut rules = Vec::new();
        loop {
            self.consume_whitespace();
            if self.eof() {
                break;
            }
            rules.push(self.parse_rule());
        }
        Stylesheet { rules }
    }

    fn parse_rule(&mut self) -> Rule {
        Rule {
            selectors: self.parse_selectors(),
            declarations: self.parse_declarations(),
        }
    }

    fn parse_selectors(&mut self) -> Vec<Selector> {
        let mut selectors = Vec::new();
        loop {
            selectors.push(Selector::Simple(self.parse_simple_selector()));
            self.consume_whitespace();
            match self.next_char() {
                ',' => {
                    self.consume_char();
                    self.consume_whitespace();
                }
                '{' => break,
                other => panic!("Unexpected character in selector list: {other:?}"),
            }
        }
        selectors.sort_by(|a, b| b.specificity().cmp(&a.specificity()));
        selectors
    }

    fn parse_simple_selector(&mut self) -> SimpleSelector {
        let mut selector = SimpleSelector {
            tag_name: None,
            id: None,
            class: Vec::new(),
        };
        while !self.eof() {
            match self.next_char() {
                '#' => {
                    self.consume_char();
                    selector.id = Some(self.parse_identifier());
                }
                '.' => {
                    self.consume_char();
                    selector.class.push(self.parse_identifier());
                }
                '*' => {
                    self.consume_char();
                }
                c if valid_identifier_char(c) => {
                    selector.tag_name = Some(self.parse_identifier());
                }
                _ => break,
            }
        }
        selector
    }

    fn parse_declarations(&mut self) -> Vec<Declaration> {
        assert_eq!(self.consume_char(), '{');
        let mut declarations = Vec::new();
        loop {
            self.consume_whitespace();
            if self.next_char() == '}' {
                self.consume_char();
                break;
            }
            declarations.push(self.parse_declaration());
        }
        declarations
    }

    fn parse_declaration(&mut self) -> Declaration {
        let name = self.parse_identifier();
        self.consume_whitespace();
        assert_eq!(self.consume_char(), ':');
        self.consume_whitespace();
        let value = self.parse_value(&name);
        self.consume_whitespace();
        assert_eq!(self.consume_char(), ';');
        Declaration { name, value }
    }

    fn parse_value(&mut self, property: &str) -> Value {
        match self.next_char() {
            '#' => Value::ColorValue(self.parse_hex_color()),
            '0'..='9' | '.' => self.parse_length(),
            _ => {
                let keyword = self.parse_identifier();
                if property == "color" {
                    if let Some(color) = named_color(&keyword) {
                        return Value::ColorValue(color);
                    }
                }
                Value::Keyword(keyword)
            }
        }
    }

    fn parse_length(&mut self) -> Value {
        let number = self.parse_float();
        let unit = self.parse_unit();
        Value::Length(number, unit)
    }

    fn parse_float(&mut self) -> f32 {
        let raw = self.consume_while(|c| matches!(c, '0'..='9' | '.'));
        raw.parse().unwrap()
    }

    fn parse_unit(&mut self) -> Unit {
        match self.parse_identifier().to_ascii_lowercase().as_str() {
            "px" => Unit::Px,
            other => panic!("Unrecognized unit: {other}"),
        }
    }

    fn parse_hex_color(&mut self) -> Color {
        assert_eq!(self.consume_char(), '#');
        let hex = self.consume_while(|c| c.is_ascii_hexdigit());
        let (r, g, b) = match hex.len() {
            3 => (
                hex_byte(&hex[0..1].repeat(2)),
                hex_byte(&hex[1..2].repeat(2)),
                hex_byte(&hex[2..3].repeat(2)),
            ),
            6 => (
                hex_byte(&hex[0..2]),
                hex_byte(&hex[2..4]),
                hex_byte(&hex[4..6]),
            ),
            _ => panic!("Invalid hex color: #{hex}"),
        };
        Color { r, g, b, a: 255 }
    }

    fn parse_identifier(&mut self) -> String {
        self.consume_while(valid_identifier_char)
    }
}

fn valid_identifier_char(c: char) -> bool {
    matches!(c, 'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_')
}

fn hex_byte(text: &str) -> u8 {
    u8::from_str_radix(text, 16).unwrap()
}

fn named_color(name: &str) -> Option<Color> {
    match name {
        "black" => Some(Color {
            r: 0,
            g: 0,
            b: 0,
            a: 255,
        }),
        "white" => Some(Color {
            r: 255,
            g: 255,
            b: 255,
            a: 255,
        }),
        "red" => Some(Color {
            r: 255,
            g: 0,
            b: 0,
            a: 255,
        }),
        "green" => Some(Color {
            r: 0,
            g: 128,
            b: 0,
            a: 255,
        }),
        "blue" => Some(Color {
            r: 0,
            g: 0,
            b: 255,
            a: 255,
        }),
        _ => None,
    }
}
