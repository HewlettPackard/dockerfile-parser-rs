// (C) Copyright 2019-2020 Hewlett Packard Enterprise Development LP

use std::convert::TryFrom;

use crate::dockerfile_parser::Instruction;
use crate::error::*;
use crate::parser::{Pair, Rule};
use crate::util::*;

use enquote::unquote;
use snafu::ResultExt;

/// An environment variable key/value pair
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct EnvVar {
  pub key: String,
  pub value: BreakableString,
}

impl EnvVar {
  pub fn new<S1: Into<String>>(key: S1, value: impl Into<BreakableString>) -> Self {
    EnvVar {
      key: key.into(),
      value: value.into(),
    }
  }
}

/// A Dockerfile [`ENV` instruction][env].
///
/// [env]: https://docs.docker.com/engine/reference/builder/#env
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct EnvInstruction(pub Vec<EnvVar>);

impl From<EnvVar> for EnvInstruction {
  fn from(var: EnvVar) -> Self {
    EnvInstruction(vec![var])
  }
}

impl From<Vec<EnvVar>> for EnvInstruction {
  fn from(vars: Vec<EnvVar>) -> Self {
    EnvInstruction(vars)
  }
}

/// Parses an env pair token, e.g. key=value or key="value"
fn parse_env_pair(record: Pair) -> Result<EnvVar> {
  let mut key = None;
  let mut value = None;

  for field in record.into_inner() {
    match field.as_rule() {
      Rule::env_name => key = Some(field.as_str()),
      Rule::env_pair_value => {
        value = Some(
          BreakableString::new(&field).add_string(&field, field.as_str())
        );
      },
      Rule::env_pair_quoted_value => {
        let v = unquote(field.as_str()).context(UnescapeError)?;

        value = Some(
          BreakableString::new(&field).add_string(&field, v)
        );
      },
      _ => return Err(unexpected_token(field))
    }
  }

  let key = key.ok_or_else(|| Error::GenericParseError {
    message: "env pair requires a key".into()
  })?.to_string();

  let value = value.ok_or_else(|| Error::GenericParseError {
    message: "env pair requires a value".into()
  })?;

  Ok(EnvVar { key, value })
}

impl EnvInstruction {
  pub fn new_single<S1>(key: S1, value: impl Into<BreakableString>) -> Self
  where
    S1: Into<String>
  {
    EnvInstruction(vec!(EnvVar {
      key: key.into(),
      value: value.into(),
    }))
  }

  pub(crate) fn from_pairs_record(record: Pair) -> Result<EnvInstruction> {
    let mut vars = Vec::new();

    for field in record.into_inner() {
      match field.as_rule() {
        Rule::env_pair => vars.push(parse_env_pair(field)?),
        Rule::comment => continue,
        _ => return Err(unexpected_token(field))
      }
    }

    Ok(EnvInstruction(vars))
  }

  pub(crate) fn from_single_record(record: Pair) -> Result<EnvInstruction> {
    let mut key = None;
    let mut value = None;

    for field in record.into_inner() {
      match field.as_rule() {
        Rule::env_name => key = Some(field.as_str()),
        Rule::env_single_value => value = Some(parse_any_breakable(field)?),
        Rule::env_single_quoted_value => {
          let v = unquote(field.as_str()).context(UnescapeError)?;

          value = Some(
            BreakableString::new(&field).add_string(&field, v)
          );
        },
        Rule::comment => continue,
        _ => return Err(unexpected_token(field))
      }
    }

    let key = key.ok_or_else(|| Error::GenericParseError {
      message: "env requires a key".into()
    })?.to_string();

    let value = value.ok_or_else(|| Error::GenericParseError {
        message: "env requires a value".into()
    })?;

    Ok(EnvInstruction(vec![EnvVar { key, value }]))
  }
}

impl<'a> TryFrom<&'a Instruction> for &'a EnvInstruction {
  type Error = Error;

  fn try_from(instruction: &'a Instruction) -> std::result::Result<Self, Self::Error> {
    if let Instruction::Env(e) = instruction {
      Ok(e)
    } else {
      Err(Error::ConversionError {
        from: format!("{:?}", instruction),
        to: "EnvInstruction".into()
      })
    }
  }
}

#[cfg(test)]
mod tests {
  use indoc::indoc;

  use super::*;
  use crate::Dockerfile;
  use crate::test_util::*;

  #[test]
  fn env() -> Result<()> {
    assert_eq!(
      parse_single(r#"env foo=bar"#, Rule::env)?.into_env().unwrap(),
      EnvInstruction(vec![EnvVar::new("foo", ((8, 11), "bar"))])
    );

    assert_eq!(
      parse_single(r#"env FOO_BAR="baz""#, Rule::env)?,
      EnvInstruction(vec![EnvVar::new("FOO_BAR", ((12, 17), "baz"))]).into()
    );

    assert_eq!(
      parse_single(r#"env FOO_BAR "baz""#, Rule::env)?,
      EnvInstruction(vec![EnvVar::new("FOO_BAR", ((12, 17), "baz"))]).into()
    );

    assert_eq!(
      parse_single(r#"env foo="bar\"baz""#, Rule::env)?,
      EnvInstruction(vec![EnvVar::new("foo", ((8, 18), "bar\"baz"))]).into()
    );

    assert_eq!(
      parse_single(r#"env foo='bar'"#, Rule::env)?,
      EnvInstruction(vec![EnvVar::new("foo", ((8, 13), "bar"))]).into()
    );

    assert_eq!(
      parse_single(r#"env foo='bar\'baz'"#, Rule::env)?,
      EnvInstruction(vec![EnvVar::new("foo", ((8, 18), "bar'baz"))]).into()
    );

    assert_eq!(
      parse_single(r#"env foo="123" bar='456' baz=789"#, Rule::env)?,
      EnvInstruction(vec![
        EnvVar::new("foo", ((8, 13), "123")),
        EnvVar::new("bar", ((18, 23), "456")),
        EnvVar::new("baz", ((28, 31), "789")),
      ]).into()
    );

    assert!(Dockerfile::parse(r#"env foo="bar"bar"#).is_err());
    assert!(Dockerfile::parse(r#"env foo='bar'bar"#).is_err());

    Ok(())
  }

  #[test]
  fn test_multiline_pairs() -> Result<()> {
    // note: docker allows empty line continuations (but may print a warning)
    assert_eq!(
      parse_single(
        indoc!(r#"
          env foo=a \
            bar=b \
            baz=c \

        "#),
        Rule::env
      )?.into_env().unwrap().0,
      vec![
        EnvVar::new("foo", ((8, 9), "a")),
        EnvVar::new("bar", ((18, 19), "b")),
        EnvVar::new("baz", ((28, 29), "c"))
      ]
    );

    Ok(())
  }

  #[test]
  fn test_multiline_single_env() -> Result<()> {
    assert_eq!(
      parse_single(
        indoc!(r#"
          env foo Lorem ipsum dolor sit amet, \
            consectetur adipiscing elit, \
            sed do eiusmod tempor incididunt ut \
            labore et dolore magna aliqua.
        "#),
        Rule::env
      )?.into_env().unwrap().0,
      vec![
        EnvVar::new("foo", BreakableString::new((8, 143))
          .add_string((8, 36), "Lorem ipsum dolor sit amet, ")
          .add_string((38, 69), "  consectetur adipiscing elit, ")
          .add_string((71, 109), "  sed do eiusmod tempor incididunt ut ")
          .add_string((111, 143), "  labore et dolore magna aliqua.")
        )
      ]
    );

    // note: maybe a small bug here, leading whitespace on the first value line
    // is eaten (this will hopefully never matter...)
    assert_eq!(
      parse_single(
        indoc!(r#"
          env \
            foo \
            Lorem ipsum dolor sit amet, \
            consectetur adipiscing elit
        "#),
        Rule::env
      )?.into_env().unwrap().0,
      vec![
        EnvVar::new("foo", BreakableString::new((16, 75))
          .add_string((16, 44), "Lorem ipsum dolor sit amet, ")
          .add_string((46, 75), "  consectetur adipiscing elit")
        )
      ]
    );

    assert_eq!(
      parse_single(
        indoc!(r#"
          env \
            foo \
            # bar
            Lorem ipsum dolor sit amet, \
            # baz
            consectetur adipiscing elit
        "#),
        Rule::env
      )?.into_env().unwrap().0,
      vec![
        EnvVar::new("foo", BreakableString::new((24, 91))
          .add_string((24, 52), "Lorem ipsum dolor sit amet, ")
          .add_comment((56, 61), "# baz")
          .add_string((62, 91), "  consectetur adipiscing elit")
        )
      ]
    );

    Ok(())
  }
}
