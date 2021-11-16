// (C) Copyright 2019-2020 Hewlett Packard Enterprise Development LP

use std::convert::TryFrom;

use crate::dockerfile_parser::Instruction;
use crate::image::ImageRef;
use crate::parser::{Pair, Rule};
use crate::parse_string;
use crate::SpannedString;
use crate::splicer::*;
use crate::error::*;

/// A Dockerfile [`FROM` instruction][from].
///
/// Contains spans for the entire instruction, the image, and the alias (if
/// any).
///
/// [from]: https://docs.docker.com/engine/reference/builder/#from
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct FromInstruction {
  pub span: Span,
  pub image: SpannedString,
  pub image_parsed: ImageRef,

  pub index: usize,
  pub alias: Option<SpannedString>,
}

impl FromInstruction {
  pub(crate) fn from_record(record: Pair, index: usize) -> Result<FromInstruction> {
    let span = Span::from_pair(&record);
    let mut image_field = None;
    let mut alias_field = None;

    for field in record.into_inner() {
      match field.as_rule() {
        Rule::from_image => image_field = Some(field),
        Rule::from_alias => alias_field = Some(field),
        Rule::comment => continue,
        _ => return Err(unexpected_token(field))
      };
    }

    let image = if let Some(image_field) = image_field {
      parse_string(&image_field)?
    } else {
      return Err(Error::GenericParseError {
        message: "missing from image".into()
      });
    };

    let image_parsed = ImageRef::parse(&image.as_ref());

    let alias = if let Some(alias_field) = alias_field {
      Some(parse_string(&alias_field)?)
    } else {
      None
    };

    Ok(FromInstruction {
      span, index,
      image, image_parsed,
      alias,
    })
  }

  // TODO: util for converting to an ImageRef while resolving ARG
  // per the docs, ARG instructions are only honored in FROMs if they occur
  // before the *first* FROM (but this should be verified)
  // fn image_ref(&self) -> ImageRef { ... }
}

impl<'a> TryFrom<&'a Instruction> for &'a FromInstruction {
  type Error = Error;

  fn try_from(instruction: &'a Instruction) -> std::result::Result<Self, Self::Error> {
    if let Instruction::From(f) = instruction {
      Ok(f)
    } else {
      Err(Error::ConversionError {
        from: format!("{:?}", instruction),
        to: "FromInstruction".into()
      })
    }
  }
}

#[cfg(test)]
mod tests {
  use indoc::indoc;
  use pretty_assertions::assert_eq;

  use super::*;
  use crate::test_util::*;

  #[test]
  fn from_no_alias() -> Result<()> {
    // pulling the FromInstruction out of the enum is messy, so just parse
    // directly
    let from = parse_direct(
      "from alpine:3.10",
      Rule::from,
      |p| FromInstruction::from_record(p, 0)
    )?;

    assert_eq!(from, FromInstruction {
      span: Span { start: 0, end: 16 },
      index: 0,
      image: SpannedString {
        span: Span { start: 5, end: 16 },
        content: "alpine:3.10".into(),
      },
      image_parsed: ImageRef {
        registry: None,
        image: "alpine".into(),
        tag: Some("3.10".into()),
        hash: None
      },
      alias: None,
    });

    Ok(())
  }

  #[test]
  fn from_no_newline() -> Result<()> {
    // unfortunately we can't use a single rule to test these as individual
    // rules have no ~ EOI requirement to ensure we parse the whole string
    assert!(parse_single(
      "from alpine:3.10 from example",
      Rule::dockerfile,
    ).is_err());

    Ok(())
  }

  #[test]
  fn from_missing_alias() -> Result<()> {
    assert!(parse_single(
      "from alpine:3.10 as",
      Rule::dockerfile,
    ).is_err());

    Ok(())
  }

  #[test]
  fn from_multiline() -> Result<()> {
    let from = parse_direct(
      indoc!(r#"
        from \
          # foo
          alpine:3.10 \

          # test
          # comment

          as \

          test
      "#),
      Rule::from,
      |p| FromInstruction::from_record(p, 0)
    )?;

    assert_eq!(from, FromInstruction {
      span: Span { start: 0, end: 68 },
      index: 0,
      image: SpannedString {
        span: Span { start: 17, end: 28 },
        content: "alpine:3.10".into(),
      },
      image_parsed: ImageRef {
        registry: None,
        image: "alpine".into(),
        tag: Some("3.10".into()),
        hash: None
      },
      alias: Some(SpannedString {
        span: (64, 68).into(),
        content: "test".into(),
      }),
    });

    Ok(())
  }
}
