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
  Shell(BreakableString),
  Exec(Vec<String>)
}

impl RunInstruction {
  pub(crate) fn from_exec_record(record: Pair) -> Result<RunInstruction> {
    Ok(RunInstruction::Exec(parse_string_array(record)?))
  }

  pub(crate) fn from_shell_record(record: Pair) -> Result<RunInstruction> {
    Ok(RunInstruction::Shell(parse_any_breakable(record)?))
  }

  pub fn exec<S: Into<String>>(args: Vec<S>) -> RunInstruction {
    RunInstruction::Exec(args.into_iter().map(|s| s.into()).collect())
  }

  /// Unpacks this instruction into its inner value if it is a Shell-form
  /// instruction, otherwise returns None.
  pub fn into_shell(self) -> Option<BreakableString> {
    if let RunInstruction::Shell(s) = self {
      Some(s)
    } else {
      None
    }
  }

  /// Unpacks this instruction into its inner value if it is a Shell-form
  /// instruction, otherwise returns None.
  pub fn as_shell(&self) -> Option<&BreakableString> {
    if let RunInstruction::Shell(s) = self {
      Some(s)
    } else {
      None
    }
  }

  /// Unpacks this instruction into its inner value if it is an Exec-form
  /// instruction, otherwise returns None.
  pub fn into_exec(self) -> Option<Vec<String>> {
    if let RunInstruction::Exec(s) = self {
      Some(s)
    } else {
      None
    }
  }

  /// Unpacks this instruction into its inner value if it is an Exec-form
  /// instruction, otherwise returns None.
  pub fn as_exec(&self) -> Option<&Vec<String>> {
    if let RunInstruction::Exec(s) = self {
      Some(s)
    } else {
      None
    }
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
  use indoc::indoc;

  use super::*;
  use crate::test_util::*;

  #[test]
  fn run_basic() -> Result<()> {
    assert_eq!(
      parse_single(r#"run echo "hello world""#, Rule::run)?
        .as_run().unwrap()
        .as_shell().unwrap(),
      &BreakableString::new((4, 22))
        .add_string((4, 22), "echo \"hello world\"")
    );

    assert_eq!(
      parse_single(r#"run ["echo", "hello world"]"#, Rule::run)?,
      RunInstruction::exec(vec!["echo", "hello world"]).into()
    );

    Ok(())
  }

  #[test]
  fn run_multiline_shell() -> Result<()> {
    assert_eq!(
      parse_single(indoc!(r#"
        run echo \
          "hello world"
      "#), Rule::run)?
        .as_run().unwrap()
        .as_shell().unwrap(),
      &BreakableString::new((4, 26))
        .add_string((4, 9), "echo ")
        .add_string((11, 26), "  \"hello world\"")
    );

    assert_eq!(
      parse_single(indoc!(r#"
        run echo \
          "hello world"
      "#), Rule::run)?
        .as_run().unwrap()
        .as_shell().unwrap()
        .to_string(),
      "echo   \"hello world\""
    );

    Ok(())
  }

  #[test]
  fn run_multiline_shell_comment() -> Result<()> {
    assert_eq!(
      parse_single(
        indoc!(r#"
          run foo && \
              # implicitly escaped
              bar && \
              # explicitly escaped \
              baz
        "#),
        Rule::run
      )?
        .into_run().unwrap()
        .into_shell().unwrap(),
      BreakableString::new((4, 85))
        .add_string((4, 11), "foo && ")
        .add_comment((17, 37), "# implicitly escaped")
        .add_string((38, 49), "    bar && ")
        .add_comment((55, 77), "# explicitly escaped \\")
        .add_string((78, 85), "    baz")
    );

    Ok(())
  }

  #[test]
  fn run_multiline_shell_large() -> Result<()> {
    // note: the trailing `\` at the end is _almost_ nonsense and generates a
    // warning from docker
    let ins = parse_single(
      indoc!(r#"
        run set -x && \
            # lorem ipsum
            echo "hello world" && \
            # dolor sit amet,
            # consectetur \
            # adipiscing elit, \
            # sed do eiusmod
            # tempor incididunt ut labore
            echo foo && \
            echo 'bar' \
            && echo baz \
            # et dolore magna aliqua."#),
      Rule::run
    )?.into_run().unwrap().into_shell().unwrap();

    assert_eq!(
      ins,
      BreakableString::new((4, 266))
        .add_string((4, 14), "set -x && ")
        .add_comment((20, 33), "# lorem ipsum")
        .add_string((34, 60), "    echo \"hello world\" && ")
        .add_comment((66, 83), "# dolor sit amet,")
        .add_comment((88, 103), "# consectetur \\")
        .add_comment((108, 128), "# adipiscing elit, \\")
        .add_comment((133, 149), "# sed do eiusmod")
        .add_comment((154, 183), "# tempor incididunt ut labore")
        .add_string((184, 200), "    echo foo && ")
        .add_string((202, 217), "    echo 'bar' ")
        .add_string((219, 235), "    && echo baz ")
        .add_comment((241, 266), "# et dolore magna aliqua.")
    );

    assert_eq!(
      ins.to_string(),
      r#"set -x &&     echo "hello world" &&     echo foo &&     echo 'bar'     && echo baz "#
    );

    Ok(())
  }

  #[test]
  fn run_multline_exec() -> Result<()> {
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

  #[test]
  fn run_multiline_exec_comment() -> Result<()> {
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
