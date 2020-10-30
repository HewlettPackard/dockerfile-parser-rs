// (C) Copyright 2019-2020 Hewlett Packard Enterprise Development LP

use std::convert::TryFrom;

use crate::dockerfile_parser::Instruction;
use crate::parser::{Pair, Rule};
use crate::util::*;
use crate::error::*;

use enquote::unquote;
use snafu::ResultExt;

/// A single label key/value pair.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Label {
  pub name: String,
  pub value: String
}

impl Label {
  pub fn new<S>(name: S, value: S) -> Label
  where
    S: Into<String>
  {
    Label {
      name: name.into(), value: value.into()
    }
  }

  pub(crate) fn from_record(record: Pair) -> Result<Label> {
    let mut name = None;
    let mut value = None;

    for field in record.into_inner() {
      match field.as_rule() {
        Rule::label_name | Rule::label_single_name => name = Some(field.as_str().to_string()),
        Rule::label_quoted_name | Rule::label_single_quoted_name => {
          // label seems to be uniquely able to span multiple lines when quoted
          let v = unquote(&clean_escaped_breaks(field.as_str()))
            .context(UnescapeError)?;

          name = Some(v);
        },

        Rule::label_value => value = Some(field.as_str().to_string()),
        Rule::label_quoted_value => {
          let v = unquote(&clean_escaped_breaks(field.as_str()))
            .context(UnescapeError)?;

          value = Some(v);
        },
        Rule::comment => continue,
        _ => return Err(unexpected_token(field))
      }
    }

    let name = name.ok_or_else(|| Error::GenericParseError {
      message: "label name is required".into()
    })?;

    let value = value.ok_or_else(|| Error::GenericParseError {
      message: "label value is required".into()
    })?;

    Ok(Label::new(name, value))
  }
}

/// A Dockerfile [`LABEL` instruction][label].
///
/// A single `LABEL` instruction may set many labels.
///
/// [label]: https://docs.docker.com/engine/reference/builder/#label
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct LabelInstruction(pub Vec<Label>);

impl LabelInstruction {
  pub(crate) fn from_record(record: Pair) -> Result<LabelInstruction> {
    let mut labels = Vec::new();

    for field in record.into_inner() {
      match field.as_rule() {
        Rule::label_pair => labels.push(Label::from_record(field)?),
        Rule::label_single => labels.push(Label::from_record(field)?),
        Rule::comment => continue,
        _ => return Err(unexpected_token(field))
      }
    }

    Ok(LabelInstruction(labels))
  }
}

impl<'a> TryFrom<&'a Instruction> for &'a LabelInstruction {
  type Error = Error;

  fn try_from(instruction: &'a Instruction) -> std::result::Result<Self, Self::Error> {
    if let Instruction::Label(l) = instruction {
      Ok(l)
    } else {
      Err(Error::ConversionError {
        from: format!("{:?}", instruction),
        to: "LabelInstruction".into()
      })
    }
  }
}

#[cfg(test)]
mod tests {
  use indoc::indoc;

  use super::*;
  use crate::test_util::*;

  #[test]
  fn label_basic() -> Result<()> {
    assert_eq!(
      parse_single("label foo=bar", Rule::label)?,
      LabelInstruction(vec![
        Label::new("foo", "bar")
      ]).into()
    );

    assert_eq!(
      parse_single("label foo.bar=baz", Rule::label)?,
      LabelInstruction(vec![
        Label::new("foo.bar", "baz")
      ]).into()
    );

    assert_eq!(
      parse_single(r#"label "foo.bar"="baz qux""#, Rule::label)?,
      LabelInstruction(vec![
        Label::new("foo.bar", "baz qux")
      ]).into()
    );

    // this is undocumented but supported :(
    assert_eq!(
      parse_single(r#"label foo.bar baz"#, Rule::label)?,
      LabelInstruction(vec![
        Label::new("foo.bar", "baz")
      ]).into()
    );
    assert_eq!(
      parse_single(r#"label "foo.bar" "baz qux""#, Rule::label)?,
      LabelInstruction(vec![
        Label::new("foo.bar", "baz qux")
      ]).into()
    );

    Ok(())
  }

  #[test]
  fn label_multi() -> Result<()> {
    assert_eq!(
      parse_single(r#"label foo=bar baz="qux" "quux quuz"="corge grault""#, Rule::label)?,
      LabelInstruction(vec![
        Label::new("foo", "bar"),
        Label::new("baz", "qux"),
        Label::new("quux quuz", "corge grault")
      ]).into()
    );

    assert_eq!(
      parse_single(
        r#"label foo=bar \
          baz="qux" \
          "quux quuz"="corge grault""#,
        Rule::label
      )?,
      LabelInstruction(vec![
        Label::new("foo", "bar"),
        Label::new("baz", "qux"),
        Label::new("quux quuz", "corge grault")
      ]).into()
    );

    Ok(())
  }

  #[test]
  fn label_multiline() -> Result<()> {
    assert_eq!(
      parse_single(r#"label "foo.bar"="baz\n qux""#, Rule::label)?,
      LabelInstruction(vec![
        Label::new("foo.bar", "baz\n qux")
      ]).into()
    );

    assert_eq!(
      parse_single(r#"label "foo\nbar"="baz\n qux""#, Rule::label)?,
      LabelInstruction(vec![
        Label::new("foo\nbar", "baz\n qux")
      ]).into()
    );

    Ok(())
  }

  #[test]
  fn label_multi_multiline() -> Result<()> {
    assert_eq!(
      parse_single(
        r#"label foo=bar \
          "lorem ipsum
          dolor
          "="sit
          amet" \
          baz=qux"#,
        Rule::label
      )?,
      LabelInstruction(vec![
        Label::new("foo", "bar"),
        Label::new("lorem ipsum\n          dolor\n          ", "sit\n          amet"),
        Label::new("baz", "qux")
      ]).into()
    );

    Ok(())
  }

  #[test]
  fn label_multiline_improper_continuation() -> Result<()> {
    // note: docker allows empty line continuations (but may print a warning)
    assert_eq!(
      parse_single(
        indoc!(r#"
          label foo=a \
            bar=b \
            baz=c \

        "#),
        Rule::label
      )?.into_label().unwrap().0,
      vec![
        Label::new("foo", "a"),
        Label::new("bar", "b"),
        Label::new("baz", "c"),
      ]
    );

    Ok(())
  }
}
