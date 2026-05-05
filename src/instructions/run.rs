// (C) Copyright 2019-2020 Hewlett Packard Enterprise Development LP

use std::convert::TryFrom;
use std::fmt;

use crate::Span;
use crate::dockerfile_parser::Instruction;
use crate::error::*;
use crate::util::*;
use crate::parser::*;
use crate::parse_string;

/// A Dockerfile [`RUN` instruction][run].
///
/// An run command may be defined as either a single string (to be run in the
/// default shell), or a list of strings (to be run directly).
///
/// [run]: https://docs.docker.com/engine/reference/builder/#run
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct RunInstruction {
  pub span: Span,
  pub options: Vec<RunOption>,
  pub expr: ShellOrExecExpr,
}

/// A key-value OPTION passed to a `RUN` instruction.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct RunOption {
  pub span: Span,
  pub name: SpannedString,
  pub value: SpannedString,
  pub original: String,
}

impl RunOption {
  fn from_record(record: Pair) -> Result<RunOption> {
    let span = Span::from_pair(&record);
    let original = record.as_str().to_string();
    let mut name = None;
    let mut value = None;

    for field in record.into_inner() {
      match field.as_rule() {
        Rule::run_option_name => name = Some(parse_string(&field)?),
        Rule::run_option_value => value = Some(parse_string(&field)?),
        _ => return Err(unexpected_token(field))
      }
    }

    let name = name.ok_or_else(|| Error::GenericParseError {
      message: "run options require a key".into(),
    })?;

    let value = value.ok_or_else(|| Error::GenericParseError {
      message: "run options require a value".into()
    })?;

    Ok(RunOption { span, name, value, original })
  }
}

impl fmt::Display for RunOption {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{}", self.original)
  }
}

impl RunInstruction {
  pub(crate) fn from_record(record: Pair) -> Result<RunInstruction> {
    let span = Span::from_pair(&record);

    // Collect any RUN options and capture the expression pair (exec or shell)
    let mut options: Vec<RunOption> = Vec::new();
    let mut expr_pair: Option<Pair> = None;
    for field in record.into_inner() {
      match field.as_rule() {
        Rule::run_option => options.push(RunOption::from_record(field)?),
        Rule::run_exec | Rule::run_shell => {
          expr_pair = Some(field);
          break;
        },
        Rule::comment => continue,
        _ => return Err(unexpected_token(field)),
      }
    }

    let field = expr_pair.ok_or_else(|| Error::GenericParseError {
      message: "missing run expression".into()
    })?;

    match field.as_rule() {
      Rule::run_exec => Ok(RunInstruction {
        span,
        options,
        expr: ShellOrExecExpr::Exec(parse_string_array(field)?),
      }),
      Rule::run_shell => {
        let mut field_iter = field.into_inner();
        let first_field = field_iter.next().ok_or_else(|| Error::GenericParseError {
          message: "missing run shell expression".into()
        })?;
        
        match first_field.as_rule() {
          Rule::run_heredoc => {
            let heredoc = parse_heredoc(first_field)?;
            Ok(RunInstruction {
              span,
              options,
              expr: ShellOrExecExpr::ShellWithHeredoc(BreakableString::new((4, 4)), heredoc),
            })
          },
          Rule::any_breakable => {
            let breakable = parse_any_breakable(first_field)?;
            
            if let Some(heredoc_field) = field_iter.next() {
              let heredoc = parse_heredoc(heredoc_field)?;
              Ok(RunInstruction {
                span,
                options,
                expr: ShellOrExecExpr::ShellWithHeredoc(breakable, heredoc),
              })
            } else {
              Ok(RunInstruction {
                span,
                options,
                expr: ShellOrExecExpr::Shell(breakable),
              })
            }
          },
          _ => Err(unexpected_token(first_field))
        }
      },
      _ => Err(unexpected_token(field))
    }
  }

  /// Unpacks this instruction into its inner value if it is a Shell-form
  /// instruction, otherwise returns None.
  pub fn into_shell(self) -> Option<BreakableString> {
    self.expr.into_shell()
  }

  /// Unpacks this instruction into its inner value if it is a Shell-form
  /// instruction, otherwise returns None.
  pub fn as_shell(&self) -> Option<&BreakableString> {
    self.expr.as_shell()
  }

  /// Unpacks this instruction into its inner value if it is an Exec-form
  /// instruction, otherwise returns None.
  pub fn into_exec(self) -> Option<StringArray> {
    self.expr.into_exec()
  }

  /// Unpacks this instruction into its inner value if it is an Exec-form
  /// instruction, otherwise returns None.
  pub fn as_exec(&self) -> Option<&StringArray> {
    self.expr.as_exec()
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
  use pretty_assertions::assert_eq;

  use super::*;
  use crate::Span;
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
      RunInstruction {
        span: Span::new(0, 27),
        options: vec![],
        expr: ShellOrExecExpr::Exec(StringArray {
          span: Span::new(4, 27),
          elements: vec![SpannedString {
            span: Span::new(5, 11),
            content: "echo".to_string(),
          }, SpannedString {
            span: Span::new(13, 26),
            content: "hello world".to_string(),
          }]
        }),
      }.into()
    );

    Ok(())
  }

  #[test]
  fn run_with_mount_option_shell() -> Result<()> {
    assert_eq!(
      parse_single(r#"run --mount=type=cache,target=/root/.cache echo hello"#, Rule::run)?
        .as_run().unwrap()
        .as_shell().unwrap()
        .to_string(),
      "echo hello"
    );
    Ok(())
  }

  #[test]
  fn run_with_network_option_exec() -> Result<()> {
    assert_eq!(
      parse_single(r#"run --network=host ["echo","hi"]"#, Rule::run)?,
      RunInstruction {
        span: Span::new(0, 32),
        options: vec![RunOption {
          span: Span::new(4, 18),
          name: SpannedString { span: Span::new(6, 13), content: "network".into() },
          value: SpannedString { span: Span::new(14, 18), content: "host".into() },
          original: "--network=host".into(),
        }],
        expr: ShellOrExecExpr::Exec(StringArray {
          span: Span::new(19, 32),
          elements: vec![SpannedString {
            span: Span::new(20, 26),
            content: "echo".to_string(),
          }, SpannedString {
            span: Span::new(27, 31),
            content: "hi".to_string(),
          }],
        }),
      }.into()
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

    // whitespace should be allowed, but my editor removes trailing whitespace
    // :)
    assert_eq!(
      parse_single("run echo \\    \t  \t\t\n  \"hello world\"", Rule::run)?
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

    assert_eq!(
      parse_single(
        indoc!(r#"
          run \
              # hey
              foo && \
              # implicitly escaped
              bar
        "#),
        Rule::run
      )?
        .into_run().unwrap()
        .into_shell().unwrap(),
      BreakableString::new((4, 61))
        .add_comment((10, 15), "# hey")
        .add_string((16, 27), "    foo && ")
        .add_comment((33, 53), "# implicitly escaped")
        .add_string((54, 61), "    bar")
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
      RunInstruction {
        span: Span::new(0, 66),
        options: vec![],
        expr: ShellOrExecExpr::Exec(StringArray {
          span: Span::new(13, 66),
          elements: vec![SpannedString {
            span: Span::new(24, 30),
            content: "echo".to_string(),
          }, SpannedString {
            span: Span::new(42, 55),
            content: "hello world".to_string(),
          }],
        }),
      }.into()
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
      RunInstruction {
        span: Span::new(0, 66),
        options: vec![],
        expr: ShellOrExecExpr::Exec(StringArray {
          span: Span::new(13, 66),
          elements: vec![SpannedString {
            span: Span::new(24, 30),
            content: "echo".to_string(),
          }, SpannedString {
            span: Span::new(42, 55),
            content: "hello world".to_string(),
          }],
        })
      }.into()
    );

    Ok(())
  }

  #[test]
  fn run_heredoc() -> Result<()> {
    assert_eq!(
      parse_single(indoc!(r#"RUN <<EOF
        echo "hello world"
        EOF
      "#), Rule::run)?,
      RunInstruction {
        span: Span::new(0, 32),
        options: vec![],
        expr: ShellOrExecExpr::ShellWithHeredoc(
          BreakableString::new((4, 4)),
          Heredoc {
            span: Span::new(4, 32),
            content: "<<EOF\necho \"hello world\"\nEOF".to_string(),
          }
        ),
      }.into()
    );

    Ok(())
  }

  #[test]
  fn run_heredoc_simple() -> Result<()> {
    assert_eq!(
      parse_single(indoc!(r#"RUN <<EOF
        echo
        EOF
      "#), Rule::run)?,
      RunInstruction {
        span: Span::new(0, 18),
        options: vec![],
        expr: ShellOrExecExpr::ShellWithHeredoc(
          BreakableString::new((4, 4)),
          Heredoc {
            span: Span::new(4, 18),
            content: "<<EOF\necho\nEOF".to_string(),
          }
        ),
      }.into()
    );

    Ok(())
  }

  #[test]
  fn run_shell_with_heredoc() -> Result<()> {
    assert_eq!(
      parse_single(indoc!(r#"RUN python3 <<EOF
      with open("/hello", "w") as f:
          print("Hello", file=f)
          print("World", file=f)
      EOF
      "#), Rule::run)?,
      RunInstruction {
        span: Span::new(0, 106),
        options: vec![],
        expr: ShellOrExecExpr::ShellWithHeredoc(
          BreakableString::new((4, 12))
            .add_string((4, 12), "python3 "),
          Heredoc {
            span: Span::new(12, 106),
            content: "<<EOF\nwith open(\"/hello\", \"w\") as f:\n    print(\"Hello\", file=f)\n    print(\"World\", file=f)\nEOF".to_string(),
          }
        ),
      }.into()
    );

    Ok(())
  }

  #[test]
  fn run_shell_no_heredoc() -> Result<()> {
    assert_eq!(
      parse_single(r#"run echo "<<EOF EOF""#, Rule::run)?
        .as_run().unwrap()
        .as_shell().unwrap(),
      &BreakableString::new((4, 20))
        .add_string((4, 20), "echo \"<<EOF EOF\"")
    );

    Ok(())
  }

  #[test]
  fn run_heredoc_empty() -> Result<()> {
    // Empty heredoc should parse successfully
    assert_eq!(
      parse_single(indoc!(r#"RUN <<EOF
        EOF
      "#), Rule::run)?,
      RunInstruction {
        span: Span::new(0, 13),
        options: vec![],
        expr: ShellOrExecExpr::ShellWithHeredoc(
          BreakableString::new((4, 4)),
          Heredoc {
            span: Span::new(4, 13),
            content: "<<EOF\nEOF".to_string(),
          }
        ),
      }.into()
    );

    Ok(())
  }

  #[test]
  fn run_heredoc_with_comments() -> Result<()> {
    // Comments inside heredoc should be treated as literal content
    assert_eq!(
      parse_single(indoc!(r#"RUN <<EOF
        # This is a comment
        echo "hello"
        EOF
      "#), Rule::run)?,
      RunInstruction {
        span: Span::new(0, 46),
        options: vec![],
        expr: ShellOrExecExpr::ShellWithHeredoc(
          BreakableString::new((4, 4)),
          Heredoc {
            span: Span::new(4, 46),
            content: "<<EOF\n# This is a comment\necho \"hello\"\nEOF".to_string(),
          }
        ),
      }.into()
    );

    Ok(())
  }

  #[test]
  fn run_heredoc_special_characters() -> Result<()> {
    // Special characters should be preserved literally
    assert_eq!(
      parse_single(indoc!(r#"RUN <<EOF
        echo "quotes" && echo 'apostrophes'
        echo $VAR ${BRACE} \backslash
        EOF
      "#), Rule::run)?,
      RunInstruction {
        span: Span::new(0, 79),
        options: vec![],
        expr: ShellOrExecExpr::ShellWithHeredoc(
          BreakableString::new((4, 4)),
          Heredoc {
            span: Span::new(4, 79),
            content: "<<EOF\necho \"quotes\" && echo 'apostrophes'\necho $VAR ${BRACE} \\backslash\nEOF".to_string(),
          }
        ),
      }.into()
    );

    Ok(())
  }

  #[test]
  fn run_heredoc_whitespace_delimiter() -> Result<()> {
    // Whitespace around delimiter should be handled
    assert_eq!(
      parse_single(indoc!(r#"RUN <<   DELIM   
        content
        DELIM
      "#), Rule::run)?,
      RunInstruction {
        span: Span::new(0, 31),
        options: vec![],
        expr: ShellOrExecExpr::ShellWithHeredoc(
          BreakableString::new((4, 4)),
          Heredoc {
            span: Span::new(4, 31),
            content: "<<   DELIM   \ncontent\nDELIM".to_string(),
          }
        ),
      }.into()
    );

    Ok(())
  }

  #[test]
  fn run_heredoc_with_destination() -> Result<()> {
    assert_eq!(
      parse_single(indoc!(r#"RUN tee <<EOF /file
      hello world
      EOF
      "#), Rule::run)?,
      RunInstruction {
        span: Span::new(0, 35),
        options: vec![],
        expr: ShellOrExecExpr::ShellWithHeredoc(
          BreakableString::new((4, 8))
            .add_string((4, 8), "tee "),
          Heredoc {
            span: Span::new(8, 35),
            content: "<<EOF /file\nhello world\nEOF".to_string(),
          }
        ),
      }.into()
    );

    Ok(())
  }

  #[test]
  fn run_heredoc_single_quoted() -> Result<()> {
    assert_eq!(
      parse_single(indoc!(r#"RUN cat <<'EOF'
        hello
        EOF
      "#), Rule::run)?,
      RunInstruction {
        span: Span::new(0, 25),
        options: vec![],
        expr: ShellOrExecExpr::ShellWithHeredoc(
          BreakableString::new((4, 8))
            .add_string((4, 8), "cat "),
          Heredoc {
            span: Span::new(8, 25),
            content: "<<'EOF'\nhello\nEOF".to_string(),
          }
        ),
      }.into()
    );

    Ok(())
  }

  #[test]
  fn run_heredoc_double_quoted() -> Result<()> {
    assert_eq!(
      parse_single(indoc!(r#"RUN cat <<"EOF"
        hello
        EOF
      "#), Rule::run)?,
      RunInstruction {
        span: Span::new(0, 25),
        options: vec![],
        expr: ShellOrExecExpr::ShellWithHeredoc(
          BreakableString::new((4, 8))
            .add_string((4, 8), "cat "),
          Heredoc {
            span: Span::new(8, 25),
            content: "<<\"EOF\"\nhello\nEOF".to_string(),
          }
        ),
      }.into()
    );

    Ok(())
  }

  #[test]
  fn run_heredoc_dash() -> Result<()> {
    assert_eq!(
      parse_single("RUN cat <<-EOF\n\thello\n\tEOF\n", Rule::run)?,
      RunInstruction {
        span: Span::new(0, 26),
        options: vec![],
        expr: ShellOrExecExpr::ShellWithHeredoc(
          BreakableString::new((4, 8))
            .add_string((4, 8), "cat "),
          Heredoc {
            span: Span::new(8, 26),
            content: "<<-EOF\n\thello\n\tEOF".to_string(),
          }
        ),
      }.into()
    );

    Ok(())
  }

  #[test]
  fn run_heredoc_dash_quoted() -> Result<()> {
    assert_eq!(
      parse_single("RUN cat <<-'EOF'\n\thello\n\tEOF\n", Rule::run)?,
      RunInstruction {
        span: Span::new(0, 28),
        options: vec![],
        expr: ShellOrExecExpr::ShellWithHeredoc(
          BreakableString::new((4, 8))
            .add_string((4, 8), "cat "),
          Heredoc {
            span: Span::new(8, 28),
            content: "<<-'EOF'\n\thello\n\tEOF".to_string(),
          }
        ),
      }.into()
    );

    Ok(())
  }

  #[test]
  fn run_heredoc_quoted_preserves_dollar_signs() -> Result<()> {
    let ins = parse_single(indoc!(r#"RUN cat > /f <<'EOF'
      echo $VAR ${BRACE}
      EOF
    "#), Rule::run)?.into_run().unwrap();

    let (_, heredoc) = ins.expr.as_shell_with_heredoc().unwrap();
    assert_eq!(heredoc.content, "<<'EOF'\necho $VAR ${BRACE}\nEOF");

    Ok(())
  }

  #[test]
  fn run_heredoc_repro_from_bug_report() -> Result<()> {
    let ins = parse_single(indoc!(r#"RUN cat > /test.conf << 'EOF'
      server {
          listen 80;
      }
      EOF
    "#), Rule::run)?.into_run().unwrap();

    let (breakable, heredoc) = ins.expr.as_shell_with_heredoc().unwrap();
    assert_eq!(breakable.to_string(), "cat > /test.conf ");
    assert_eq!(
      heredoc.content,
      "<< 'EOF'\nserver {\n    listen 80;\n}\nEOF"
    );

    Ok(())
  }

  #[test]
  fn run_option_display() -> Result<()> {
    let ins = parse_single(r#"run --security=insecure --mount=type=cache,target=/root echo hi"#, Rule::run)?
      .into_run().unwrap();
    assert_eq!(ins.options.len(), 2);
    assert_eq!(ins.options[0].to_string(), "--security=insecure");
    assert_eq!(ins.options[1].to_string(), "--mount=type=cache,target=/root");
    Ok(())
  }
}
