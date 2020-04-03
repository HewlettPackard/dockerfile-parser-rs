// (C) Copyright 2019-2020 Hewlett Packard Enterprise Development LP

use std::convert::TryFrom;

use crate::dockerfile::Instruction;
use crate::parser::{Pair, Rule};
use crate::error::*;

use enquote::unquote;
use snafu::ResultExt;

/// A Dockerfile [`ARG` instruction][arg].
///
/// [arg]: https://docs.docker.com/engine/reference/builder/#arg
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArgInstruction {
  /// The argument key
  pub name: String,

  /// An optional argument value.
  ///
  /// This may be unset when passing arguments through to later stages in a
  /// [multi-stage build][build].
  ///
  /// [build]: https://docs.docker.com/develop/develop-images/multistage-build/
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
