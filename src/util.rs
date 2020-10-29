// (C) Copyright 2019 Hewlett Packard Enterprise Development LP

use std::fmt;

use crate::error::*;
use crate::parser::*;
use crate::splicer::Span;

use enquote::unquote;
use snafu::ResultExt;

/// Given a node ostensibly containing a string array, returns an unescaped
/// array of strings
pub(crate) fn parse_string_array(array: Pair) -> Result<Vec<String>> {
  let mut ret = Vec::new();

  for field in array.into_inner() {
    match field.as_rule() {
      Rule::string => {
        let s = unquote(field.as_str()).context(UnescapeError)?;

        ret.push(s);
      },
      _ => return Err(unexpected_token(field))
    }
  }

  Ok(ret)
}

/// Removes escaped line breaks (\\\n) from a string
///
/// This should be used to clean any input from the any_breakable rule
pub(crate) fn clean_escaped_breaks(s: &str) -> String {
  s.replace("\\\n", "")
}

/// A comment with a character span.
#[derive(Debug, Eq, PartialEq, Ord, PartialOrd, Clone)]
pub struct SpannedComment {
  pub span: Span,
  pub content: String,
}

/// A string with a character span.
#[derive(Debug, Eq, PartialEq, Ord, PartialOrd, Clone)]
pub struct SpannedString {
  pub span: Span,
  pub content: String,
}

/// A component of a breakable string.
#[derive(Debug, Eq, PartialEq, Ord, PartialOrd, Clone)]
pub enum BreakableStringComponent {
  String(SpannedString),
  Comment(SpannedComment),
}

impl From<SpannedString> for BreakableStringComponent {
  fn from(s: SpannedString) -> Self {
    BreakableStringComponent::String(s)
  }
}

impl From<((usize, usize), &str)> for BreakableStringComponent {
  fn from(s: ((usize, usize), &str)) -> Self {
    let ((start, end), content) = s;

    BreakableStringComponent::String(SpannedString {
      span: (start, end).into(),
      content: content.to_string(),
    })
  }
}

impl From<SpannedComment> for BreakableStringComponent {
  fn from(c: SpannedComment) -> Self {
    BreakableStringComponent::Comment(c)
  }
}

/// A Docker string that may be broken across several lines, separated by line
/// continuations (`\\\n`), and possibly intermixed with comments.
///
/// These strings have several potentially valid interpretations. As these line
/// continuations match those natively supported by bash, a given multiline
/// `RUN` block can be pasted into a bash shell unaltered and with line
/// continuations included. However, at "runtime" line continuations and
/// comments (*) are stripped from the string handed to the shell.
///
/// To ensure output is correct in all cases, `BreakableString` preserves the
/// user's original AST, including comments, and implements Docker's
/// continuation-stripping behavior in the `Display` implementation.
#[derive(Debug, Eq, PartialEq, Ord, PartialOrd, Clone)]
pub struct BreakableString {
  pub span: Span,
  pub components: Vec<BreakableStringComponent>,
}

/// Formats this breakable string as it will be interpreted by the underlying
/// Docker engine, i.e. on a single line with line continuations removed
impl fmt::Display for BreakableString {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    for component in &self.components {
      if let BreakableStringComponent::String(s) = &component {
        write!(f, "{}", s.content)?;
      }
    }

    Ok(())
  }
}

impl BreakableString {
  pub fn new(span: impl Into<Span>) -> Self {
    BreakableString {
      span: span.into(),
      components: Vec::new(),
    }
  }

  pub fn add(mut self, c: impl Into<BreakableStringComponent>) -> Self {
    self.components.push(c.into());

    self
  }

  pub fn add_string(mut self, s: impl Into<Span>, c: impl Into<String>) -> Self {
    self.components.push(SpannedString {
      span: s.into(),
      content: c.into(),
    }.into());

    self
  }

  pub fn add_comment(mut self, s: impl Into<Span>, c: impl Into<String>) -> Self {
    self.components.push(SpannedComment {
      span: s.into(),
      content: c.into(),
    }.into());

    self
  }

  pub fn iter_components(&self) -> impl Iterator<Item = &BreakableStringComponent> {
    self.components.iter()
  }
}

impl From<((usize, usize), &str)> for BreakableString {
  fn from(s: ((usize, usize), &str)) -> Self {
    let ((start, end), content) = s;

    BreakableString::new((start, end))
      .add_string((start, end), content)
  }
}

fn parse_any_breakable_inner(pair: Pair) -> Result<Vec<BreakableStringComponent>> {
  let mut components = Vec::new();

  for field in pair.into_inner() {
    match field.as_rule() {
      Rule::any_breakable => components.extend(parse_any_breakable_inner(field)?),
      Rule::comment => components.push(SpannedComment {
        span: (&field).into(),
        content: field.as_str().to_string(),
      }.into()),
      Rule::any_content => components.push(SpannedString {
        span: (&field).into(),
        content: field.as_str().to_string(),
      }.into()),
      _ => return Err(unexpected_token(field))
    }
  }

  Ok(components)
}

pub(crate) fn parse_any_breakable(pair: Pair) -> Result<BreakableString> {
  Ok(BreakableString {
    span: (&pair).into(),
    components: parse_any_breakable_inner(pair)?,
  })
}
