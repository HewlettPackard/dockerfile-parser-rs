// (C) Copyright 2019-2020 Hewlett Packard Enterprise Development LP

use std::convert::TryFrom;

use crate::dockerfile::Instruction;
use crate::error::*;
use crate::util::*;
use crate::parser::*;

/// A Dockerfile [`ENTRYPOINT` instruction][entrypoint].
///
/// An entrypoint may be defined as either a single string (to be run in the
/// default shell), or a list of strings (to be run directly).
///
/// [entrypoint]: https://docs.docker.com/engine/reference/builder/#entrypoint
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum EntrypointInstruction {
  Shell(String),
  Exec(Vec<String>)
}

impl EntrypointInstruction {
  pub(crate) fn from_exec_record(record: Pair) -> Result<EntrypointInstruction> {
    Ok(EntrypointInstruction::Exec(parse_string_array(record)?))
  }

  pub(crate) fn from_shell_record(record: Pair) -> Result<EntrypointInstruction> {
    Ok(EntrypointInstruction::Shell(
      clean_escaped_breaks(record.as_str())
    ))
  }

  pub fn shell<S: Into<String>>(s: S) -> EntrypointInstruction {
    EntrypointInstruction::Shell(s.into())
  }

  pub fn exec<S: Into<String>>(args: Vec<S>) -> EntrypointInstruction {
    EntrypointInstruction::Exec(args.into_iter().map(|s| s.into()).collect())
  }
}

impl TryFrom<Instruction> for EntrypointInstruction {
  type Error = Error;

  fn try_from(instruction: Instruction) -> std::result::Result<Self, Self::Error> {
    if let Instruction::Entrypoint(e) = instruction {
      Ok(e)
    } else {
      Err(Error::ConversionError {
        from: format!("{:?}", instruction),
        to: "EntrypointInstruction".into()
      })
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::test_util::*;

  #[test]
  fn entrypoint_basic() -> Result<()> {
    assert_eq!(
      parse_single(r#"entrypoint echo "hello world""#, Rule::entrypoint)?,
      EntrypointInstruction::shell("echo \"hello world\"").into()
    );

    assert_eq!(
      parse_single(r#"entrypoint ["echo", "hello world"]"#, Rule::entrypoint)?,
      EntrypointInstruction::exec(vec!["echo", "hello world"]).into()
    );

    Ok(())
  }

  #[test]
  fn entrypoint_multiline() -> Result<()> {
    assert_eq!(
      parse_single(r#"entrypoint echo \
        "hello world""#, Rule::entrypoint)?,
      EntrypointInstruction::shell("echo         \"hello world\"").into()
    );

    assert_eq!(
      parse_single(r#"entrypoint\
        [\
        "echo", \
        "hello world"\
        ]"#, Rule::entrypoint)?,
      EntrypointInstruction::exec(vec!["echo", "hello world"]).into()
    );

    Ok(())
  }
}
