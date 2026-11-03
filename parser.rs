use std::collections::HashMap;
use crate::dom::Node;

pub struct Parser {
    pos: usize,
    input: String,
}

impl Parser {
    pub fn new(input: String) -> Parser {
        Parser { pos: 0, input }
    }

    fn next_char(&self) -> char {
        self.input[self.pos..].chars().next().unwrap()
    }

    fn eof(&self) -> bool {
        self.pos >= self.input.len()
    }

    fn consume_char(&mut self) -> char {
        let mut iter = self.input[self.pos..].char_indices();
        let (_, cur_char) = iter.next().unwrap();
        let (next_pos, _) = iter.next().unwrap_or((self.input.len() - self.pos, ' '));
        self.pos += next_pos;
        cur_char
    }

    fn consume_while<F>(&mut self, test: F) -> String
    where F: Fn(char) -> bool {
        let mut result = String::new();
        while !self.eof() && test(self.next_char()) {
            result.push(self.consume_char());
        }
        result
    }

    fn consume_whitespace(&mut self) {
        self.consume_while(|c| c.is_whitespace());
    }

    fn parse_tag_name(&mut self) -> String {
        self.consume_while(|c| matches!(c, 'a'..='z' | 'A'..='Z' | '0'..='9'))
    }

    pub fn parse_nodes(&mut self) -> Vec<Node> {
        let mut nodes = Vec::new();
        loop {
            self.consume_whitespace();
            if self.eof() || self.input[self.pos..].starts_with("</") {
                break;
            }
            nodes.push(self.parse_node());
        }
        nodes
    }

    fn parse_node(&mut self) -> Node {
        if self.next_char() == '<' {
            self.parse_element()
        } else {
            self.parse_text()
        }
    }

    fn parse_text(&mut self) -> Node {
        Node::text(self.consume_while(|c| c != '<'))
    }

    fn parse_attr(&mut self) -> (String, String) {
        let name = self.parse_tag_name();
        self.consume_whitespace();
        assert_eq!(self.consume_char(), '=');
        self.consume_whitespace();
        let quote = self.consume_char();
        assert!(quote == '"' || quote == '\'');
        let value = self.consume_while(|c| c != quote);
        assert_eq!(self.consume_char(), quote);
        (name, value)
    }

    fn parse_attributes(&mut self) -> HashMap<String, String> {
        let mut attributes = HashMap::new();
        loop {
            self.consume_whitespace();
            if self.eof() || self.next_char() == '>' {
                break;
            }
            let (name, value) = self.parse_attr();
            attributes.insert(name, value);
        }
        attributes
    }

    fn parse_element(&mut self) -> Node {
        assert_eq!(self.consume_char(), '<');
        let tag_name = self.parse_tag_name();
        let attrs = self.parse_attributes();
        assert_eq!(self.consume_char(), '>');

        let children = self.parse_nodes();

        assert_eq!(self.consume_char(), '<');
        assert_eq!(self.consume_char(), '/');
        assert_eq!(self.parse_tag_name(), tag_name);
        assert_eq!(self.consume_char(), '>');

        Node::elem(tag_name, attrs, children)
    }
}
