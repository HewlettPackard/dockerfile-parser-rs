// (C) Copyright 2019-2020 Hewlett Packard Enterprise Development LP

use std::convert::TryFrom;

use crate::dockerfile_parser::Instruction;
use crate::error::*;
use crate::util::*;
use crate::parser::*;

/// A Dockerfile [`RUN` instruction][run].
///
/// An run command may be defined as either a single string (to be run in the
/// default shell), or a list of strings (to be run directly).
///
/// [run]: https://docs.docker.com/engine/reference/builder/#run
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum RunInstruction {
  Shell(String),
  Exec(Vec<String>)
}

impl RunInstruction {
  pub(crate) fn from_exec_record(record: Pair) -> Result<RunInstruction> {
    Ok(RunInstruction::Exec(parse_string_array(record)?))
  }

  pub(crate) fn from_shell_record(record: Pair) -> Result<RunInstruction> {
    Ok(RunInstruction::Shell(
      clean_escaped_breaks(record.as_str())
    ))
  }

  pub fn shell<S: Into<String>>(s: S) -> RunInstruction {
    RunInstruction::Shell(s.into())
  }

  pub fn exec<S: Into<String>>(args: Vec<S>) -> RunInstruction {
    RunInstruction::Exec(args.into_iter().map(|s| s.into()).collect())
  }
}

impl<'a> TryFrom<&'a Instruction> for &'a RunInstruction {
  type Error = Error;

  fn try_from(instruction: &'a Instruction) -> std::result::Result<Self, Self::Error> {
    if let Instruction::Run(r) = instruction {
      Ok(r)
    } else {
      Err(Error::ConversionError {
        from: format!("{:?}", instruction),
        to: "RunInstruction".into()
      })
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::test_util::*;

  #[test]
  fn run_basic() -> Result<()> {
    assert_eq!(
      parse_single(r#"run echo "hello world""#, Rule::run)?,
      RunInstruction::shell("echo \"hello world\"").into()
    );

    assert_eq!(
      parse_single(r#"run ["echo", "hello world"]"#, Rule::run)?,
      RunInstruction::exec(vec!["echo", "hello world"]).into()
    );

    Ok(())
  }

  #[test]
  fn run_multiline() -> Result<()> {
    assert_eq!(
      parse_single(r#"run echo \
        "hello world""#, Rule::run)?,
      RunInstruction::shell("echo         \"hello world\"").into()
    );

    assert_eq!(
      parse_single(r#"run\
        [\
        "echo", \
        "hello world"\
        ]"#, Rule::run)?,
      RunInstruction::exec(vec!["echo", "hello world"]).into()
    );

    Ok(())
  }
}
