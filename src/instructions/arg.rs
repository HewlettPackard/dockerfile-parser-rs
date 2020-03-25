// (C) Copyright 2019 Hewlett Packard Enterprise Development LP

use crate::parser::{Pair, Rule};
use crate::error::*;

use enquote::unquote;
use snafu::ResultExt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArgInstruction {
  pub name: String,
  pub value: Option<String>
}

impl ArgInstruction {
  pub(crate) fn from_record(record: Pair) -> Result<ArgInstruction> {
    let mut name = None;
    let mut value = None;

    for field in record.into_inner() {
      match field.as_rule() {
        Rule::arg_name => name = Some(field.as_str()),
        Rule::arg_quoted_value => {
          let v = unquote(field.as_str()).context(UnescapeError)?;

          value = Some(v);
        }
        Rule::arg_value => value = Some(field.as_str().to_string()),
        _ => return Err(unexpected_token(field))
      }
    }

    let name = name.ok_or_else(|| Error::GenericParseError {
      message: "arg name is required".into()
    })?.to_string();

    let value = value.map(String::from);

    Ok(ArgInstruction {
      name, value
    })
  }
}