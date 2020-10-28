// (C) Copyright 2019-2020 Hewlett Packard Enterprise Development LP

use std::convert::TryFrom;

use crate::dockerfile_parser::Instruction;
use crate::parser::{Pair, Rule};
use crate::error::*;

use enquote::unquote;
use snafu::ResultExt;

/// An environment variable key/value pair
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct EnvVar {
  pub key: String,
  pub value: String
}

impl EnvVar {
  pub fn new<S1, S2>(key: S1, value: S2) -> EnvVar
  where
    S1: Into<String>,
    S2: Into<String>,
  {
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
pub struct EnvInstruction(Vec<EnvVar>);

/// Parses an env pair token, e.g. key=value or key="value"
fn parse_env_pair(record: Pair) -> Result<EnvVar> {
  let mut key = None;
  let mut value = None;

  for field in record.into_inner() {
    match field.as_rule() {
      Rule::env_pair_name => key = Some(field.as_str()),
      Rule::env_pair_value => value = Some(field.as_str().to_string()),
      Rule::env_pair_quoted_value => {
        let v = unquote(field.as_str()).context(UnescapeError)?;

        value = Some(v)
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
  pub(crate) fn from_pairs_record(record: Pair) -> Result<EnvInstruction> {
    let mut vars = Vec::new();

    for field in record.into_inner() {
      match field.as_rule() {
        Rule::env_pair => vars.push(parse_env_pair(field)?),
        _ => return Err(unexpected_token(field))
      }
    }

    Ok(EnvInstruction(vars))
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
  use super::*;
  use crate::Dockerfile;
  use crate::test_util::*;

  #[test]
  fn env() -> Result<()> {
    assert_eq!(
      parse_single(r#"env foo=bar"#, Rule::env)?,
      EnvInstruction(vec![EnvVar::new("foo", "bar")]).into()
    );

    assert_eq!(
      parse_single(r#"env foo="bar""#, Rule::env)?,
      EnvInstruction(vec![EnvVar::new("foo", "bar")]).into()
    );

    assert_eq!(
      parse_single(r#"env foo="bar\"baz""#, Rule::env)?,
      EnvInstruction(vec![EnvVar::new("foo", "bar\"baz")]).into()
    );

    assert_eq!(
      parse_single(r#"env foo='bar'"#, Rule::env)?,
      EnvInstruction(vec![EnvVar::new("foo", "bar")]).into()
    );

    assert_eq!(
      parse_single(r#"env foo='bar\'baz'"#, Rule::env)?,
      EnvInstruction(vec![EnvVar::new("foo", "bar'baz")]).into()
    );

    assert_eq!(
      parse_single(r#"env foo="123" bar='456' baz=789"#, Rule::env)?,
      EnvInstruction(vec![
        EnvVar::new("foo", "123"),
        EnvVar::new("bar", "456"),
        EnvVar::new("baz", "789"),
      ]).into()
    );

    assert!(Dockerfile::parse(r#"env foo="bar"bar"#).is_err());
    assert!(Dockerfile::parse(r#"env foo='bar'bar"#).is_err());

    Ok(())
  }
}
