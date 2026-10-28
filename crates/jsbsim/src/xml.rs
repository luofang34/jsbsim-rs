// SPDX-License-Identifier: LGPL-2.1-or-later
// FGXMLElement: Copyright (C) 2001 Jon S. Berndt (jon@jsbsim.org).
// Derived from JSBSim src/input_output/FGXMLElement.{h,cpp}.
//
//! Loader for JSBSim's aircraft, engine, system, script and
//! initial-conditions XML, with JSBSim's unit conversions. Constructs that are
//! not supported are rejected with [`Error::Unsupported`](crate::Error::Unsupported).
//!
//! An [`Element`] holds what JSBSim's `Element` holds: the tag name, the
//! attributes, the non-blank lines of character data (trimmed), the child
//! elements, and the file and line it came from.

mod convert;
mod error;
mod loader;
mod numbers;
mod parse;

#[cfg(test)]
mod tests;

use std::collections::BTreeMap;
use std::rc::Rc;

pub use error::{Location, XmlError};
pub use loader::{DocumentLoader, IncludeKind};
pub use numbers::atof_locale_c;
pub use parse::parse_document;

/// One XML element as JSBSim sees it.
#[derive(Debug, Clone, PartialEq)]
pub struct Element {
    name: String,
    attributes: BTreeMap<String, String>,
    data_lines: Vec<String>,
    children: Vec<Element>,
    file: Rc<str>,
    line: u32,
}

impl Element {
    /// Creates an element with no attributes, data or children.
    #[must_use]
    pub fn new(name: impl Into<String>, file: Rc<str>, line: u32) -> Self {
        Self {
            name: name.into(),
            attributes: BTreeMap::new(),
            data_lines: Vec::new(),
            children: Vec::new(),
            file,
            line,
        }
    }

    /// Tag name, including any namespace prefix.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Where the element starts.
    #[must_use]
    pub fn location(&self) -> Location {
        Location {
            file: self.file.to_string(),
            line: self.line,
        }
    }

    /// Resource path of the document the element came from.
    #[must_use]
    pub fn file_name(&self) -> &str {
        &self.file
    }

    /// One-based line of the start tag.
    #[must_use]
    pub fn line_number(&self) -> u32 {
        self.line
    }

    /// All attributes, sorted by name.
    #[must_use]
    pub fn attributes(&self) -> &BTreeMap<String, String> {
        &self.attributes
    }

    /// `HasAttribute`.
    #[must_use]
    pub fn has_attribute(&self, key: &str) -> bool {
        self.attributes.contains_key(key)
    }

    /// `GetAttributeValue`: the value, or `""` when absent.
    #[must_use]
    pub fn attribute(&self, key: &str) -> &str {
        self.attributes.get(key).map_or("", String::as_str)
    }

    /// `AddAttribute`: sets `key`, replacing an existing value.
    pub fn set_attribute(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.attributes.insert(key.into(), value.into());
    }

    /// `GetAttributeValueAsNumber`.
    pub fn attribute_as_number(&self, key: &str) -> Result<f64, XmlError> {
        let text = self.attribute(key);
        if text.is_empty() {
            return Err(XmlError::MissingAttribute {
                element: self.name.clone(),
                attribute: key.to_owned(),
                at: self.location(),
            });
        }
        atof_locale_c(text).map_err(|message| self.invalid_number(message))
    }

    fn invalid_number(&self, message: String) -> XmlError {
        XmlError::InvalidNumber {
            element: self.name.clone(),
            at: self.location(),
            message,
        }
    }

    /// Non-blank, trimmed lines of character data.
    #[must_use]
    pub fn data_lines(&self) -> &[String] {
        &self.data_lines
    }

    /// `GetDataLine(i)`: line `i`, or `""` when there is no such line.
    #[must_use]
    pub fn data_line(&self, i: usize) -> &str {
        self.data_lines.get(i).map_or("", String::as_str)
    }

    /// `AddData`: appends a line, dropping leading spaces and tabs.
    pub fn add_data(&mut self, line: &str) {
        self.data_lines
            .push(line.trim_start_matches([' ', '\t']).to_owned());
    }

    /// `GetDataAsNumber`: the single data line as a number.
    pub fn data_as_number(&self) -> Result<f64, XmlError> {
        match self.data_lines.as_slice() {
            [line] => atof_locale_c(line).map_err(|message| self.invalid_number(message)),
            [] => Err(XmlError::NoData {
                element: self.name.clone(),
                at: self.location(),
            }),
            lines => Err(XmlError::MultipleLines {
                element: self.name.clone(),
                at: self.location(),
                lines: lines.len(),
            }),
        }
    }

    /// Child elements in document order.
    #[must_use]
    pub fn children(&self) -> &[Element] {
        &self.children
    }

    /// Mutable child elements in document order.
    pub fn children_mut(&mut self) -> &mut Vec<Element> {
        &mut self.children
    }

    /// `AddChildElement`.
    pub fn add_child(&mut self, child: Element) {
        self.children.push(child);
    }

    /// `FindElement(name)`: the first child named `name`; the first child
    /// of any name when `name` is empty.
    #[must_use]
    pub fn find_element(&self, name: &str) -> Option<&Element> {
        self.children
            .iter()
            .find(|c| name.is_empty() || c.name == name)
    }

    /// The children `FindElement(name)` followed by `FindNextElement(name)`
    /// visit: those named `name`, or all of them when `name` is empty.
    pub fn find_elements<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a Element> + 'a {
        self.children
            .iter()
            .filter(move |c| name.is_empty() || c.name == name)
    }

    /// `GetNumElements(name)`.
    #[must_use]
    pub fn num_elements(&self, name: &str) -> usize {
        self.find_elements(name).count()
    }

    /// `FindElementValue(name)`: first data line of the first child named
    /// `name`, or `""`.
    #[must_use]
    pub fn find_element_value(&self, name: &str) -> &str {
        self.find_element(name).map_or("", |e| e.data_line(0))
    }

    fn require(&self, name: &str) -> Result<&Element, XmlError> {
        self.find_element(name)
            .ok_or_else(|| XmlError::MissingElement {
                parent: self.name.clone(),
                name: name.to_owned(),
                at: self.location(),
            })
    }

    /// `FindElementValueAsNumber(name)`.
    pub fn find_element_value_as_number(&self, name: &str) -> Result<f64, XmlError> {
        self.require(name)?.data_as_number()
    }

    /// `FindElementValueAsBoolean(name)`: non-zero is true; a missing child
    /// is logged and reads as false.
    pub fn find_element_value_as_boolean(&self, name: &str) -> Result<bool, XmlError> {
        match self.find_element(name) {
            Some(child) => Ok(child.data_as_number()? != 0.0),
            None => {
                tracing::error!(
                    "{}: Attempting to get non-existent element {name} ;returning false",
                    self.location()
                );
                Ok(false)
            }
        }
    }

    /// `MergeAttributes`: copies the attributes of `other` that this
    /// element does not have.
    pub fn merge_attributes(&mut self, other: &Element) {
        for (key, value) in &other.attributes {
            self.attributes
                .entry(key.clone())
                .or_insert_with(|| value.clone());
        }
    }
}
