use core::fmt;
use std::collections::{HashMap, HashSet};

use anyhow::anyhow;
use regex::Regex;
use serde::{
    Deserialize,
    de::{DeserializeSeed, IntoDeserializer, MapAccess, Visitor},
    forward_to_deserialize_any,
};

#[derive(PartialEq, Eq, Clone, Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
enum ReferenceType {
    Journal,
    Website,
}

#[derive(PartialEq, Eq, Clone, Debug, Deserialize)]
pub struct Reference {
    name: String,
    kind: ReferenceType,
    author: String,
    title: String,
    journal: Option<String>,
    year: Option<i16>,
}

struct ReferenceDeserializer<'de> {
    input: &'de str,
}

impl<'de> ReferenceDeserializer<'de> {
    pub fn from_str(input: &'de str) -> Self {
        ReferenceDeserializer { input }
    }

    // Look at the first character in the input without consuming it.
    fn peek_char(&mut self) -> Result<char, DeserializeError> {
        self.input.chars().next().ok_or(DeserializeError::eof())
    }

    // Consume the first character in the input.
    fn next_char(&mut self) -> Result<char, DeserializeError> {
        let ch = self.peek_char()?;
        self.input = &self.input[ch.len_utf8()..];
        Ok(ch)
    }

    fn skip_whitespace(&mut self) {
        if let Some(len) = self.input.find(|c: char| !c.is_whitespace()) {
            self.input = &self.input[len..];
        } else {
            self.input = "";
        }
    }

    // Consume the first character in the input.
    fn advance_if_char(&mut self, test: char) -> Result<char, DeserializeError> {
        let ch = self.peek_char()?;
        if ch == test {
            self.input = &self.input[ch.len_utf8()..];
            Ok(ch)
        } else {
            Err(DeserializeError::new(format!(
                "Expected {} but found {}",
                test, ch
            )))
        }
    }

    fn parse_signed(&mut self) -> Result<i16, DeserializeError> {
        let is_negative = self.peek_char()? == '-';
        if is_negative {
            self.next_char()?;
        }
        match self.input.find(|c: char| !c.is_ascii_digit()) {
            Some(len) => {
                let s = &self.input[..len];
                self.input = &self.input[len..];
                match str::parse::<i16>(s) {
                    Ok(num) => Ok(if is_negative { -num } else { num }),
                    Err(err) => Err(DeserializeError::new(err.to_string())),
                }
            }
            None => Err(DeserializeError::new("Couldn't find end of number")),
        }
    }

    // Parses until a non-word character is encountered
    fn parse_word(&mut self) -> Result<&'de str, DeserializeError> {
        let next = self.peek_char()?;
        if !(next.is_alphanumeric() || next == '_') {
            return Err(DeserializeError::new("Expected a bare word"));
        }
        match self
            .input
            .find(|c: char| !(c.is_alphanumeric() || c == '_'))
        {
            Some(len) => {
                let s = &self.input[..len];
                self.input = &self.input[len..];
                Ok(s)
            }
            None => Err(DeserializeError::eof()),
        }
    }

    fn parse_string(&mut self) -> Result<&'de str, DeserializeError> {
        self.advance_if_char('"')?;
        match self.input.find('"') {
            Some(len) => {
                let s = &self.input[..len];
                self.input = &self.input[len + 1..];
                Ok(s)
            }
            None => Err(DeserializeError::new("Could not find end of string")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DeserializeError {
    msg: String,
}
impl DeserializeError {
    pub fn eof() -> Self {
        Self {
            msg: "End of reference encountered".into(),
        }
    }

    pub fn new<T: fmt::Display>(msg: T) -> Self {
        Self {
            msg: msg.to_string(),
        }
    }
}

impl std::error::Error for DeserializeError {
    fn description(&self) -> &str {
        &self.msg
    }
}

impl serde::de::Error for DeserializeError {
    fn custom<T: fmt::Display>(msg: T) -> DeserializeError {
        DeserializeError {
            msg: msg.to_string(),
        }
    }
}

impl fmt::Display for DeserializeError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.msg)
    }
}

impl<'de> serde::de::Deserializer<'de> for &'_ mut ReferenceDeserializer<'de> {
    type Error = DeserializeError;

    fn deserialize_any<V>(self, _: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        unimplemented!()
    }

    fn deserialize_str<V>(self, visitor: V) -> Result<V::Value, DeserializeError>
    where
        V: Visitor<'de>,
    {
        visitor.visit_borrowed_str(self.parse_string()?)
    }

    fn deserialize_string<V>(self, visitor: V) -> Result<V::Value, DeserializeError>
    where
        V: Visitor<'de>,
    {
        self.deserialize_str(visitor)
    }

    fn deserialize_identifier<V>(self, visitor: V) -> Result<V::Value, DeserializeError>
    where
        V: Visitor<'de>,
    {
        visitor.visit_borrowed_str(self.parse_word()?)
    }

    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value, DeserializeError>
    where
        V: Visitor<'de>,
    {
        // There's no 'null' here, an empty value is omitted
        visitor.visit_some(self)
    }

    fn deserialize_i16<V>(self, visitor: V) -> Result<V::Value, DeserializeError>
    where
        V: Visitor<'de>,
    {
        visitor.visit_i16(self.parse_signed()?)
    }

    fn deserialize_struct<V>(
        self,
        name: &'de str,
        fields: &'de [&'de str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        if self.next_char()? == '@' {
            let kind = self.parse_word()?;
            self.advance_if_char('{')?;
            let reference_name = self.parse_word()?;
            visitor.visit_map(ReferenceFields::new(self, kind, reference_name))
        } else {
            Err(DeserializeError::new("Cannot find start of reference"))
        }
    }

    forward_to_deserialize_any! {
        bool i8 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char
        bytes byte_buf unit unit_struct newtype_struct seq tuple
        tuple_struct map enum ignored_any
    }
}

fn reference_from_str<'a, T>(s: &'a str) -> anyhow::Result<T>
where
    T: Deserialize<'a>,
{
    let mut deserializer = ReferenceDeserializer::from_str(s);
    match T::deserialize(&mut deserializer) {
        Ok(t) => {
            deserializer.skip_whitespace();
            if deserializer.input.is_empty() {
                Ok(t)
            } else {
                Err(anyhow!("TrailingCharacters -{}-", deserializer.input))
            }
        }
        Err(error) => Err(anyhow::anyhow!(
            "Deserialise error: {}, remaining: {}",
            error,
            deserializer.input
        )),
    }
}

struct ConstantStringDeserializer<'a> {
    value: &'a str,
}

impl<'a> ConstantStringDeserializer<'a> {
    pub fn new(value: &'a str) -> Self {
        Self { value }
    }
}

impl<'de> serde::de::Deserializer<'de> for &'_ mut ConstantStringDeserializer<'de> {
    type Error = DeserializeError;

    fn deserialize_any<V>(self, _: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        unimplemented!("Not a supported constant string deserialize operation")
    }

    fn deserialize_str<V>(self, visitor: V) -> Result<V::Value, DeserializeError>
    where
        V: Visitor<'de>,
    {
        visitor.visit_borrowed_str(self.value)
    }

    fn deserialize_string<V>(self, visitor: V) -> Result<V::Value, DeserializeError>
    where
        V: Visitor<'de>,
    {
        self.deserialize_str(visitor)
    }

    fn deserialize_identifier<V>(self, visitor: V) -> Result<V::Value, DeserializeError>
    where
        V: Visitor<'de>,
    {
        visitor.visit_borrowed_str(self.value)
    }

    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value, DeserializeError>
    where
        V: Visitor<'de>,
    {
        visitor.visit_some(self)
    }

    fn deserialize_enum<V>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, DeserializeError>
    where
        V: Visitor<'de>,
    {
        visitor.visit_enum(self.value.into_deserializer())
    }

    forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char
        bytes byte_buf unit unit_struct newtype_struct seq tuple
        tuple_struct map ignored_any struct
    }
}

struct ReferenceFields<'a, 'de: 'a> {
    de: &'a mut ReferenceDeserializer<'de>,
    kind: &'de str,
    name: &'de str,
    position: i8,
}

impl<'a, 'de> ReferenceFields<'a, 'de> {
    fn new(de: &'a mut ReferenceDeserializer<'de>, kind: &'de str, name: &'de str) -> Self {
        Self {
            de,
            kind,
            name,
            position: 0,
        }
    }
}

impl<'de> MapAccess<'de> for ReferenceFields<'_, 'de> {
    type Error = DeserializeError;

    fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>, DeserializeError>
    where
        K: DeserializeSeed<'de>,
    {
        match self.position {
            0 => {
                let mut constant = ConstantStringDeserializer::new("kind");
                seed.deserialize(&mut constant).map(Some)
            }
            1 => {
                let mut constant = ConstantStringDeserializer::new("name");
                seed.deserialize(&mut constant).map(Some)
            }
            _ => {
                self.de.skip_whitespace();
                // Check if there are no more entries.
                if self.de.peek_char()? == '}' {
                    self.de.next_char()?;
                    return Ok(None);
                }
                if self.de.next_char()? != ',' {
                    return Err(DeserializeError::new("Expected a comma"));
                }
                self.de.skip_whitespace();
                seed.deserialize(&mut *self.de).map(Some)
            }
        }
    }

    fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value, DeserializeError>
    where
        V: DeserializeSeed<'de>,
    {
        match self.position {
            0 => {
                self.position += 1;
                let mut constant = ConstantStringDeserializer::new(self.kind);
                seed.deserialize(&mut constant)
            }
            1 => {
                self.position += 1;
                let mut constant = ConstantStringDeserializer::new(self.name);
                seed.deserialize(&mut constant)
            }
            _ => {
                self.de.skip_whitespace();
                if self.de.next_char()? != '=' {
                    return Err(DeserializeError::new("Expected equals"));
                }
                // Deserialize a map value.
                self.de.skip_whitespace();
                seed.deserialize(&mut *self.de)
            }
        }
    }
}

#[derive(PartialEq, Eq, Clone, Debug)]
struct Citation {
    name: String,
}

pub fn process_references(source: String) -> String {
    source
}

pub fn extract_citations(source: &str) -> anyhow::Result<Vec<String>> {
    let extract_re = Regex::new(r"\\cite\{([^}]+)\}")?;

    let mut seen = HashSet::new();
    let mut captures: Vec<&str> = extract_re
        .captures_iter(source)
        .filter_map(|c| c.get(1))
        .map(|m| m.as_str())
        .collect();
    captures.retain(|x| seen.insert(*x));
    Ok(captures.iter().map(|c| c.to_string()).collect())
}

pub fn extract_references(
    source: &str,
) -> Result<(String, HashMap<String, Reference>), anyhow::Error> {
    let extract_re = Regex::new(r"@(journal|website)\{.*?,(?s).*?(?-s)\}\r?\n?")?;
    let without_citations = extract_re.replace_all(source, "");
    let matches: anyhow::Result<Vec<Reference>> = extract_re
        .find_iter(source)
        .map(|m| reference_from_str(m.as_str()))
        .collect();

    let refs = HashMap::from_iter(matches?.into_iter().map(|r| (r.name.clone(), r)));

    Ok((without_citations.into(), refs))
}

#[test]
pub fn test_extract_reference() {
    let input = r#""
this is a citation test \cite{test1}


@journal{test2,
author = "test 2",
title = "Another test article",
journal = "A journal",
year = 2024
}

@website{test1,
author = "test 1",
title = "A test article",
journal = "A journal",
year = 2023
}
""#;
    let (text, references) = extract_references(input).unwrap();
    assert_eq!(
        text,
        r#""
this is a citation test \cite{test1}



""#
    );
    assert!(references.contains_key("test1"));
    assert!(references.contains_key("test2"));
    assert_eq!(
        *references.get("test1").unwrap(),
        Reference {
            name: "test1".into(),
            kind: ReferenceType::Website,
            author: "test 1".into(),
            title: "A test article".into(),
            journal: Some("A journal".into()),
            year: Some(2023)
        }
    );
    assert_eq!(
        *references.get("test2").unwrap(),
        Reference {
            name: "test2".into(),
            kind: ReferenceType::Journal,
            author: "test 2".into(),
            title: "Another test article".into(),
            journal: Some("A journal".into()),
            year: Some(2024)
        }
    );
}

#[test]
pub fn test_extract_citations() {
    let input = r#""
		\cite{test1} \cite{test1} \cite{test2} \cite{test1} \cite{test3}
		""#;
    let extracted = extract_citations(input).unwrap();
    assert_eq!(extracted.len(), 3);
    assert_eq!(extracted, vec!["test1", "test2", "test3"]);
}
