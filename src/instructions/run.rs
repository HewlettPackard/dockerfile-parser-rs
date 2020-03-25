// (C) Copyright 2019 Hewlett Packard Enterprise Development LP

use crate::error::*;
use crate::util::*;
use crate::parser::*;

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
