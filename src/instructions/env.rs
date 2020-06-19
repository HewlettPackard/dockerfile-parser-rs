// (C) Copyright 2019-2020 Hewlett Packard Enterprise Development LP

use std::convert::TryFrom;

use crate::dockerfile_parser::Instruction;
use crate::parser::{Pair, Rule};
use crate::util::*;
use crate::error::*;

use enquote::unquote;
use snafu::ResultExt;

/// An environment variable key/value pair
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct EnvVar {
  pub key: String,
  pub value: String
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
  pub(crate) fn from_single_record(record: Pair) -> Result<EnvInstruction> {
    let mut key = None;
    let mut value = None;

    for field in record.into_inner() {
      match field.as_rule() {
        Rule::env_single_name => key = Some(field.as_str()),
        Rule::env_single_value => value = Some(field.as_str()),
        _ => return Err(unexpected_token(field))
      }
    }

    let key = key.ok_or_else(|| Error::GenericParseError {
      message: "env requires a key".into()
    })?.to_string();

    let value = clean_escaped_breaks(
      value.ok_or_else(|| Error::GenericParseError {
        message: "env requires a value".into()
      })?
    );

    Ok(EnvInstruction(vec![EnvVar { key, value }]))
  }

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
