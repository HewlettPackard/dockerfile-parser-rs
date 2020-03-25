// (C) Copyright 2019 Hewlett Packard Enterprise Development LP

use crate::error::*;
use crate::util::*;
use crate::parser::*;


#[derive(Debug, PartialEq, Eq, Clone)]
pub enum CmdInstruction {
  Shell(String),
  Exec(Vec<String>)
}

impl CmdInstruction {
  pub(crate) fn from_exec_record(record: Pair) -> Result<CmdInstruction> {
    Ok(CmdInstruction::Exec(parse_string_array(record)?))
  }

  pub(crate) fn from_shell_record(record: Pair) -> Result<CmdInstruction> {
    Ok(CmdInstruction::Shell(
      clean_escaped_breaks(record.as_str())
    ))
  }

  pub fn shell<S: Into<String>>(s: S) -> CmdInstruction {
    CmdInstruction::Shell(s.into())
  }

  pub fn exec<S: Into<String>>(args: Vec<S>) -> CmdInstruction {
    CmdInstruction::Exec(args.into_iter().map(|s| s.into()).collect())
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::test_util::*;

  #[test]
  fn cmd_basic() -> Result<()> {
    assert_eq!(
      parse_single(r#"cmd echo "hello world""#, Rule::cmd)?,
      CmdInstruction::shell("echo \"hello world\"").into()
    );

    assert_eq!(
      parse_single(r#"cmd ["echo", "hello world"]"#, Rule::cmd)?,
      CmdInstruction::exec(vec!["echo", "hello world"]).into()
    );

    Ok(())
  }

  #[test]
  fn cmd_multiline() -> Result<()> {
    assert_eq!(
      parse_single(r#"cmd echo \
        "hello world""#, Rule::cmd)?,
      CmdInstruction::shell("echo         \"hello world\"").into()
    );

    assert_eq!(
      parse_single(r#"cmd\
        [\
        "echo", \
        "hello world"\
        ]"#, Rule::cmd)?,
      CmdInstruction::exec(vec!["echo", "hello world"]).into()
    );

    Ok(())
  }
}
