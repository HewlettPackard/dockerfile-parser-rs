// (C) Copyright 2019-2020 Hewlett Packard Enterprise Development LP

use std::convert::TryFrom;

use crate::dockerfile_parser::Instruction;
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
  Shell(BreakableString),
  Exec(Vec<String>)
}

impl EntrypointInstruction {
  pub(crate) fn from_exec_record(record: Pair) -> Result<EntrypointInstruction> {
    Ok(EntrypointInstruction::Exec(parse_string_array(record)?))
  }

  pub(crate) fn from_shell_record(record: Pair) -> Result<EntrypointInstruction> {
    Ok(EntrypointInstruction::Shell(parse_any_breakable(record)?))
  }

  pub fn exec<S: Into<String>>(args: Vec<S>) -> EntrypointInstruction {
    EntrypointInstruction::Exec(args.into_iter().map(|s| s.into()).collect())
  }

  /// Unpacks this instruction into its inner value if it is a Shell-form
  /// instruction, otherwise returns None.
  pub fn into_shell(self) -> Option<BreakableString> {
    if let EntrypointInstruction::Shell(s) = self {
      Some(s)
    } else {
      None
    }
  }

  /// Unpacks this instruction into its inner value if it is a Shell-form
  /// instruction, otherwise returns None.
  pub fn as_shell(&self) -> Option<&BreakableString> {
    if let EntrypointInstruction::Shell(s) = self {
      Some(s)
    } else {
      None
    }
  }

  /// Unpacks this instruction into its inner value if it is an Exec-form
  /// instruction, otherwise returns None.
  pub fn into_exec(self) -> Option<Vec<String>> {
    if let EntrypointInstruction::Exec(s) = self {
      Some(s)
    } else {
      None
    }
  }

  /// Unpacks this instruction into its inner value if it is an Exec-form
  /// instruction, otherwise returns None.
  pub fn as_exec(&self) -> Option<&Vec<String>> {
    if let EntrypointInstruction::Exec(s) = self {
      Some(s)
    } else {
      None
    }
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
  use indoc::indoc;

  use super::*;
  use crate::test_util::*;

  #[test]
  fn entrypoint_basic() -> Result<()> {
    assert_eq!(
      parse_single(r#"entrypoint echo "hello world""#, Rule::entrypoint)?
        .as_entrypoint().unwrap()
        .as_shell().unwrap(),
      &BreakableString::new((11, 29))
        .add_string((11, 29), "echo \"hello world\"")
    );

    assert_eq!(
      parse_single(r#"entrypoint ["echo", "hello world"]"#, Rule::entrypoint)?,
      EntrypointInstruction::exec(vec!["echo", "hello world"]).into()
    );

    Ok(())
  }

  #[test]
  fn entrypoint_multiline_exec() -> Result<()> {
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

  #[test]
  fn entrypoint_multiline_shell() -> Result<()> {
    assert_eq!(
      parse_single(indoc!(r#"
        entrypoint echo \
          "hello world"
      "#), Rule::entrypoint)?
        .as_entrypoint().unwrap()
        .as_shell().unwrap(),
      &BreakableString::new((11, 33))
        .add_string((11, 16), "echo ")
        .add_string((18, 33), "  \"hello world\"")
    );

    Ok(())
  }

  #[test]
  fn entrypoint_multiline_large() -> Result<()> {
    // note: the trailing `\` at the end is _almost_ nonsense and generates a
    // warning from docker
    let ins = parse_single(
      indoc!(r#"
        entrypoint set -x && \
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
      Rule::entrypoint
    )?.into_entrypoint().unwrap().into_shell().unwrap();

    assert_eq!(
      ins,
      BreakableString::new((11, 273))
        .add_string((11, 21), "set -x && ")
        .add_comment((27, 40), "# lorem ipsum")
        .add_string((41, 67), "    echo \"hello world\" && ")
        .add_comment((73, 90), "# dolor sit amet,")
        .add_comment((95, 110), "# consectetur \\")
        .add_comment((115, 135), "# adipiscing elit, \\")
        .add_comment((140, 156), "# sed do eiusmod")
        .add_comment((161, 190), "# tempor incididunt ut labore")
        .add_string((191, 207), "    echo foo && ")
        .add_string((209, 224), "    echo 'bar' ")
        .add_string((226, 242), "    && echo baz ")
        .add_comment((248, 273), "# et dolore magna aliqua.")
    );

    assert_eq!(
      ins.to_string(),
      r#"set -x &&     echo "hello world" &&     echo foo &&     echo 'bar'     && echo baz "#
    );

    Ok(())
  }
}
