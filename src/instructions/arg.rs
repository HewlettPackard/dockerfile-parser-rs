// (C) Copyright 2019-2020 Hewlett Packard Enterprise Development LP

use std::convert::TryFrom;

use crate::dockerfile_parser::Instruction;
use crate::error::*;
use crate::parser::{Pair, Rule};
use crate::splicer::Span;

use enquote::unquote;
use snafu::ResultExt;

/// A Dockerfile [`ARG` instruction][arg].
///
/// [arg]: https://docs.docker.com/engine/reference/builder/#arg
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArgInstruction {
  pub span: Span,

  /// The argument key
  pub name: String,

  pub name_span: Span,

  /// An optional argument value.
  ///
  /// This may be unset when passing arguments through to later stages in a
  /// [multi-stage build][build].
  ///
  /// [build]: https://docs.docker.com/develop/develop-images/multistage-build/
  pub value: Option<String>,

  pub value_span: Option<Span>,
}

impl ArgInstruction {
  pub(crate) fn from_record(record: Pair) -> Result<ArgInstruction> {
    let span = Span::from_pair(&record);
    let mut name = None;
    let mut value = None;

    for field in record.into_inner() {
      match field.as_rule() {
        Rule::arg_name => name = Some((field.as_str(), Span::from_pair(&field))),
        Rule::arg_quoted_value => {
          let v = unquote(field.as_str()).context(UnescapeError)?;

          value = Some((v, Span::from_pair(&field)));
        },
        Rule::arg_value => value = Some((
          field.as_str().to_string(),
          Span::from_pair(&field)
        )),
        _ => return Err(unexpected_token(field))
      }
    }

    let (name, name_span) = match name {
      Some((name, name_span)) => (name.to_string(), name_span),
      _ => return Err(Error::GenericParseError {
        message: "arg name is required".into()
      })
    };

    let (value, value_span) = match value {
      Some((value, value_span)) => (Some(value), Some(value_span)),
      None => (None, None)
    };

    Ok(ArgInstruction {
      span,
      name,
      name_span,
      value,
      value_span,
    })
  }
}

impl<'a> TryFrom<&'a Instruction> for &'a ArgInstruction {
 type Error = Error;

 fn try_from(instruction: &'a Instruction) -> std::result::Result<Self, Self::Error> {
   if let Instruction::Arg(a) = instruction {
     Ok(a)
   } else {
     Err(Error::ConversionError {
       from: format!("{:?}", instruction),
       to: "ArgInstruction".into()
     })
   }
 }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::Dockerfile;
  use crate::test_util::*;

  #[test]
  fn arg_strings() -> Result<()> {
    assert_eq!(
      parse_single(r#"arg foo=bar"#, Rule::arg)?,
      ArgInstruction {
        span: Span::new(0, 11),
        name: "foo".into(),
        name_span: Span::new(4, 7),
        value: Some("bar".into()),
        value_span: Some(Span::new(8, 11)),
      }.into()
    );

    assert_eq!(
      parse_single(r#"arg foo="bar""#, Rule::arg)?,
      ArgInstruction {
        span: Span::new(0, 13),
        name: "foo".into(),
        name_span: Span::new(4, 7),
        value: Some("bar".into()),
        value_span: Some(Span::new(8, 13)),
      }.into()
    );

    assert_eq!(
      parse_single(r#"arg foo='bar'"#, Rule::arg)?,
      ArgInstruction {
        span: Span::new(0, 13),
        name: "foo".into(),
        name_span: Span::new(4, 7),
        value: Some("bar".into()),
        value_span: Some(Span::new(8, 13)),
      }.into()
    );

    assert!(Dockerfile::parse(r#"arg foo="bar"bar"#).is_err());
    assert!(Dockerfile::parse(r#"arg foo='bar'bar"#).is_err());

    Ok(())
  }
}
