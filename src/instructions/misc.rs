// (C) Copyright 2019 Hewlett Packard Enterprise Development LP

use crate::error::*;
use crate::util::*;
use crate::parser::*;

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct MiscInstruction {
  instruction: String,
  arguments: String
}

impl MiscInstruction {
  pub(crate) fn from_record(record: Pair) -> Result<MiscInstruction> {
    let mut instruction = None;
    let mut arguments = None;

    for field in record.into_inner() {
      match field.as_rule() {
        Rule::misc_instruction => instruction = Some(field.as_str()),
        Rule::misc_arguments => arguments = Some(field.as_str()),
        _ => return Err(unexpected_token(field))
      }
    }

    let instruction = instruction.ok_or_else(|| Error::GenericParseError {
      message: "generic instructions require a name".into()
    })?.to_string();

    let arguments = clean_escaped_breaks(arguments.ok_or_else(|| Error::GenericParseError {
      message: "generic instructions require arguments".into()
    })?);

    Ok(MiscInstruction {
      instruction, arguments
    })
  }
}
