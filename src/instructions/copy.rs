// (C) Copyright 2019-2020 Hewlett Packard Enterprise Development LP

use std::convert::TryFrom;

use snafu::ensure;

use crate::dockerfile_parser::Instruction;
use crate::parser::{Pair, Rule};
use crate::{Span, parse_string};
use crate::SpannedString;
use crate::error::*;

/// A key/value pair passed to a `COPY` instruction as a flag.
///
/// Examples include: `COPY --from=foo /to /from`
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct CopyFlag {
  pub span: Span,
  pub name: SpannedString,
  pub value: SpannedString,
}

impl CopyFlag {
  fn from_record(record: Pair) -> Result<CopyFlag> {
    let span = Span::from_pair(&record);
    let mut name = None;
    let mut value = None;

    for field in record.into_inner() {
      match field.as_rule() {
        Rule::copy_flag_name => name = Some(parse_string(&field)?),
        Rule::copy_flag_value => value = Some(parse_string(&field)?),
        _ => return Err(unexpected_token(field))
      }
    }

    let name = name.ok_or_else(|| Error::GenericParseError {
      message: "copy flags require a key".into(),
    })?;

    // Boolean flags like `--link` parse without a `=value`. BuildKit treats
    // bare `--link` as equivalent to `--link=true`, so we synthesize that here
    // rather than push the Optional all the way through the public CopyFlag API.
    let value = value.unwrap_or_else(|| SpannedString {
      span,
      content: "true".to_string(),
    });

    Ok(CopyFlag {
      span, name, value
    })
  }
}

/// A source that is either a filename or the file contents (heredocs)
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum SourceType {
  FileName(SpannedString),
  FileContents(SpannedString),
}

/// A Dockerfile [`COPY` instruction][copy].
///
/// [copy]: https://docs.docker.com/engine/reference/builder/#copy
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct CopyInstruction {
  pub span: Span,
  pub flags: Vec<CopyFlag>,
  pub sources: Vec<SourceType>,
  pub destination: SpannedString
}

impl CopyInstruction {
  pub(crate) fn from_record(record: Pair) -> Result<CopyInstruction> {
    let span = Span::from_pair(&record);
    let mut flags = Vec::new();
    let mut destination = SpannedString { span: Span::new(0, 0), content: String::new() };

    let mut inner = record.into_inner();
    let field = inner.next().ok_or_else(|| Error::GenericParseError {
      message: "Copy instruction expected a field".into(),
    })?;
    
    match field.as_rule() {
      Rule::copy_standard => {
        let mut paths = Vec::new();
        for inner in field.into_inner() {
          match inner.as_rule() {
            Rule::copy_flag => flags.push(CopyFlag::from_record(inner)?),
            Rule::copy_pathspec => paths.push(parse_string(&inner)?),
            Rule::comment => continue,
            _ => return Err(unexpected_token(inner))
          }
        }
        ensure!(
          paths.len() >= 2,
          GenericParseError {
            message: "copy requires at least one source and a destination"
          }
        );
        destination = paths.pop().unwrap();
        Ok(CopyInstruction {
          span,
          flags,
          sources: paths.into_iter().map(SourceType::FileName).collect(),
          destination
        })
      },
      Rule::copy_heredoc => {
        let mut sources = Vec::new();
        for inner in field.into_inner() {
          match inner.as_rule() {
            Rule::copy_flag => flags.push(CopyFlag::from_record(inner)?),
            Rule::copy_pathspec => destination = parse_string(&inner)?,
            Rule::heredoc_body => sources.push(parse_string(&inner)?),
            _ => return Err(unexpected_token(inner))
          }
        }
        ensure!(
          sources.len() >= 1,
          GenericParseError {
            message: "copy requires at least one source and a destination"
          }
        );
        Ok(CopyInstruction {
          span,
          flags,
          sources: sources.into_iter().map(SourceType::FileContents).collect(),
          destination
        })
      },
      _ => return Err(unexpected_token(field))
    }
  }
}

impl<'a> TryFrom<&'a Instruction> for &'a CopyInstruction {
  type Error = Error;

  fn try_from(instruction: &'a Instruction) -> std::result::Result<Self, Self::Error> {
    if let Instruction::Copy(c) = instruction {
      Ok(c)
    } else {
      Err(Error::ConversionError {
        from: format!("{:?}", instruction),
        to: "CopyInstruction".into()
      })
    }
  }
}

#[cfg(test)]
mod tests {
  use indoc::indoc;
  use pretty_assertions::assert_eq;

  use super::*;
  use crate::test_util::*;

  #[test]
  fn copy_basic() -> Result<()> {
    assert_eq!(
      parse_single("copy foo bar", Rule::copy)?,
      CopyInstruction {
        span: Span { start: 0, end: 12 },
        flags: vec![],
        sources: vec![SourceType::FileName(SpannedString {
          span: Span::new(5, 8),
          content: "foo".to_string()
        })],
        destination: SpannedString {
          span: Span::new(9, 12),
          content: "bar".to_string()
        },
      }.into()
    );

    Ok(())
  }

  #[test]
  fn copy_multiple_sources() -> Result<()> {
    assert_eq!(
      parse_single("copy foo bar baz qux", Rule::copy)?,
      CopyInstruction {
        span: Span { start: 0, end: 20 },
        flags: vec![],
        sources: vec![
          SourceType::FileName(SpannedString {
            span: Span::new(5, 8),
            content: "foo".to_string(),
          }),
          SourceType::FileName(SpannedString {
            span: Span::new(9, 12),
            content: "bar".to_string()
          }),
          SourceType::FileName(SpannedString {
            span: Span::new(13, 16),
            content: "baz".to_string()
          })
        ],
        destination: SpannedString {
          span: Span::new(17, 20),
          content: "qux".to_string()
        },
      }.into()
    );

    Ok(())
  }

  #[test]
  fn copy_multiline() -> Result<()> {
    // multiline is okay; whitespace on the next line is optional
    assert_eq!(
      parse_single("copy foo \\\nbar", Rule::copy)?,
      CopyInstruction {
        span: Span { start: 0, end: 14 },
        flags: vec![],
        sources: vec![SourceType::FileName(SpannedString {
          span: Span::new(5, 8),
          content: "foo".to_string(),
        })],
        destination: SpannedString {
          span: Span::new(11, 14),
          content: "bar".to_string(),
        },
      }.into()
    );

    // newlines must be escaped
    assert_eq!(
      parse_single("copy foo\nbar", Rule::copy).is_err(),
      true
    );

    Ok(())
  }

  #[test]
  fn copy_flags() -> Result<()> {
    assert_eq!(
      parse_single(
        "copy --from=alpine:3.10 /usr/lib/libssl.so.1.1 /tmp/",
        Rule::copy
      )?,
      CopyInstruction {
        span: Span { start: 0, end: 52 },
        flags: vec![
          CopyFlag {
            span: Span { start: 5, end: 23 },
            name: SpannedString {
              content: "from".into(),
              span: Span { start: 7, end: 11 },
            },
            value: SpannedString {
              content: "alpine:3.10".into(),
              span: Span { start: 12, end: 23 },
            }
          }
        ],
        sources: vec![SourceType::FileName(SpannedString {
          span: Span::new(24, 46),
          content: "/usr/lib/libssl.so.1.1".to_string(),
        })],
        destination: SpannedString {
          span: Span::new(47, 52),
          content: "/tmp/".into(),
        }
      }.into()
    );

    Ok(())
  }

  #[test]
  fn copy_boolean_flag_bare() -> Result<()> {
    // BuildKit's --link is a boolean flag and may appear without `=value`.
    // The grammar accepts this; the synthesized value is "true".
    let parsed = parse_single("copy --link foo bar", Rule::copy)?
      .into_copy()
      .unwrap();
    assert_eq!(parsed.flags.len(), 1);
    assert_eq!(parsed.flags[0].name.content, "link");
    assert_eq!(parsed.flags[0].value.content, "true");
    assert_eq!(parsed.sources.len(), 1);
    match &parsed.sources[0] {
      SourceType::FileName(s) => assert_eq!(s.content, "foo"),
      _ => panic!("expected FileName source"),
    }
    assert_eq!(parsed.destination.content, "bar");
    Ok(())
  }

  #[test]
  fn copy_boolean_flag_with_explicit_value() -> Result<()> {
    // The explicit `--link=true` / `--link=false` forms still work and the
    // value round-trips literally.
    let parsed = parse_single("copy --link=false foo bar", Rule::copy)?
      .into_copy()
      .unwrap();
    assert_eq!(parsed.flags.len(), 1);
    assert_eq!(parsed.flags[0].name.content, "link");
    assert_eq!(parsed.flags[0].value.content, "false");
    Ok(())
  }

  #[test]
  fn copy_boolean_flag_mixed_with_valued_flag() -> Result<()> {
    // Bare boolean flags compose with valued flags in any order.
    let parsed = parse_single("copy --link --chmod=755 foo bar", Rule::copy)?
      .into_copy()
      .unwrap();
    assert_eq!(parsed.flags.len(), 2);
    assert_eq!(parsed.flags[0].name.content, "link");
    assert_eq!(parsed.flags[0].value.content, "true");
    assert_eq!(parsed.flags[1].name.content, "chmod");
    assert_eq!(parsed.flags[1].value.content, "755");
    assert_eq!(parsed.sources.len(), 1);
    assert_eq!(parsed.destination.content, "bar");
    Ok(())
  }

  #[test]
  fn copy_comments() -> Result<()> {
    assert_eq!(
      parse_single(
        indoc!(r#"
          copy \
            --from=alpine:3.10 \

            # hello

            /usr/lib/libssl.so.1.1 \
            # world
            /tmp/
        "#),
        Rule::copy
      )?.into_copy().unwrap(),
      CopyInstruction {
        span: Span { start: 0, end: 86 },
        flags: vec![
          CopyFlag {
            span: Span { start: 9, end: 27 },
            name: SpannedString {
              span: Span { start: 11, end: 15 },
              content: "from".into(),
            },
            value: SpannedString {
              span: Span { start: 16, end: 27 },
              content: "alpine:3.10".into(),
            },
          }
        ],
        sources: vec![SourceType::FileName(SpannedString {
          span: Span::new(44, 66),
          content: "/usr/lib/libssl.so.1.1".to_string(),
        })],
        destination: SpannedString {
          span: Span::new(81, 86),
          content: "/tmp/".into(),
        },
      }.into()
    );

    Ok(())
  }

  #[test]
  fn copy_heredoc() -> Result<()> {
    assert_eq!(
      parse_single(
        indoc!(r#"
          COPY <<EOF /usr/share/nginx/html/index.html
          <!DOCTYPE html>
          <html>
          <head>
              <title>Welcome to nginx!</title>
          </head>
          <body>
              <h1>Welcome to nginx!</h1>
          </body>
          </html>
          EOF
        "#),
        Rule::copy
      )?.into_copy().unwrap(),
      CopyInstruction {
        span: Span { start: 0, end: 176 },
        flags: vec![],
        sources: vec![SourceType::FileContents(SpannedString {
          span: Span::new(44, 173),
          content: indoc!(r#"
          <!DOCTYPE html>
          <html>
          <head>
              <title>Welcome to nginx!</title>
          </head>
          <body>
              <h1>Welcome to nginx!</h1>
          </body>
          </html>
          "#).to_string(),
        })],
        destination: SpannedString {
          span: Span::new(11, 43),
          content: "/usr/share/nginx/html/index.html".to_string(),
        },
      }.into()
    );

    Ok(())
  }
  
  #[test]
  fn copy_heredoc_simple() -> Result<()> {
    assert_eq!(
      parse_single(
        indoc!(r#"
          COPY <<EOF /tmp/test.txt
          hello
          EOF
        "#),
        Rule::copy
      )?.into_copy().unwrap(),
      CopyInstruction {
        span: Span { start: 0, end: 34 },
        flags: vec![],
        sources: vec![SourceType::FileContents(SpannedString {
          span: Span::new(25, 31),
          content: "hello\n".to_string(),
        })],
        destination: SpannedString {
          span: Span::new(11, 24),
          content: "/tmp/test.txt".to_string(),
        },
      }.into()
    );

    Ok(())
  }

  #[test]
  fn copy_heredoc_incorrect() -> Result<()> {
    assert!(parse_single(
      indoc!(r#"
        COPY <<EOF /usr/share/nginx/html/index.html
        <!DOCTYPE html>
        <html>
        <head>
            <title>Welcome to nginx!</title>
        </head>
        <body>
            <h1>Welcome to nginx!</h1>
        </body>
        </html>
        WRONGTERMINATOR
      "#),
      Rule::copy
    ).is_err());

    Ok(())
  }

  #[test]
  fn copy_heredoc_with_comments() -> Result<()> {
    // Comments inside heredoc body should be treated as literal content, not parsed as comments
    assert_eq!(
      parse_single(
        indoc!(r#"
          COPY <<EOF /tmp/script.sh
          #!/bin/bash
          # This is a comment inside the heredoc
          echo "hello world"
          # Another comment
          EOF
        "#),
        Rule::copy
      )?.into_copy().unwrap(),
      CopyInstruction {
        span: Span { start: 0, end: 117 },
        flags: vec![],
        sources: vec![SourceType::FileContents(SpannedString {
          span: Span::new(26, 114),
          content: indoc!(r#"
            #!/bin/bash
            # This is a comment inside the heredoc
            echo "hello world"
            # Another comment
            "#).to_string(),
        })],
        destination: SpannedString {
          span: Span::new(11, 25),
          content: "/tmp/script.sh".to_string(),
        },
      }.into()
    );

    Ok(())
  }

  #[test]
  fn copy_heredoc_empty() -> Result<()> {
    // Empty heredoc should parse successfully with empty content
    assert_eq!(
      parse_single(
        indoc!(r#"
          COPY <<EOF /tmp/empty.txt
          EOF
        "#),
        Rule::copy
      )?.into_copy().unwrap(),
      CopyInstruction {
        span: Span { start: 0, end: 29 },
        flags: vec![],
        sources: vec![SourceType::FileContents(SpannedString {
          span: Span::new(26, 26),
          content: "".to_string(),
        })],
        destination: SpannedString {
          span: Span::new(11, 25),
          content: "/tmp/empty.txt".to_string(),
        },
      }.into()
    );

    Ok(())
  }

  #[test]
  fn copy_heredoc_with_flags() -> Result<()> {
    // Heredoc with copy flags should work
    assert_eq!(
      parse_single(
        indoc!(r#"
          COPY --from=builder <<EOF /tmp/config.json
          {
            "version": "1.0",
            "env": "production"
          }
          EOF
        "#),
        Rule::copy
      )?.into_copy().unwrap(),
      CopyInstruction {
        span: Span { start: 0, end: 92 },
        flags: vec![
          CopyFlag {
            span: Span { start: 5, end: 19 },
            name: SpannedString {
              span: Span { start: 7, end: 11 },
              content: "from".to_string(),
            },
            value: SpannedString {
              span: Span { start: 12, end: 19 },
              content: "builder".to_string(),
            },
          }
        ],
        sources: vec![SourceType::FileContents(SpannedString {
          span: Span::new(43, 89),
          content: indoc!(r#"
            {
              "version": "1.0",
              "env": "production"
            }
            "#).to_string(),
        })],
        destination: SpannedString {
          span: Span::new(26, 42),
          content: "/tmp/config.json".to_string(),
        },
      }.into()
    );

    Ok(())
  }

  #[test]
  fn copy_heredoc_special_characters() -> Result<()> {
    // Test heredoc with special characters that might interfere with parsing
    assert_eq!(
      parse_single(
        indoc!(r#"
          COPY <<EOF /tmp/special.txt
          Line with "quotes" and 'apostrophes'
          Line with $variables and ${braces}
          Line with \backslashes\ and /forward/slashes/
          Line with <>brackets<> and (parentheses)
          EOF
        "#),
        Rule::copy
      )?.into_copy().unwrap(),
      CopyInstruction {
        span: Span { start: 0, end: 190 },
        flags: vec![],
        sources: vec![SourceType::FileContents(SpannedString {
          span: Span::new(28, 187),
          content: indoc!(r#"
            Line with "quotes" and 'apostrophes'
            Line with $variables and ${braces}
            Line with \backslashes\ and /forward/slashes/
            Line with <>brackets<> and (parentheses)
            "#).to_string(),
        })],
        destination: SpannedString {
          span: Span::new(11, 27),
          content: "/tmp/special.txt".to_string(),
        },
      }.into()
    );

    Ok(())
  }

  #[test]
  fn copy_heredoc_like_content() -> Result<()> {
    // Content that looks like Dockerfile instructions should be treated as literal
    assert_eq!(
      parse_single(
        indoc!(r#"
          COPY <<EOF /tmp/dockerfile-content.txt
          FROM alpine:latest
          RUN apk add --no-cache curl
          COPY . /app
          CMD ["echo", "hello"]
          EOF
        "#),
        Rule::copy
      )?.into_copy().unwrap(),
      CopyInstruction {
        span: Span { start: 0, end: 123 },
        flags: vec![],
        sources: vec![SourceType::FileContents(SpannedString {
          span: Span::new(39, 120),
          content: indoc!(r#"
            FROM alpine:latest
            RUN apk add --no-cache curl
            COPY . /app
            CMD ["echo", "hello"]
            "#).to_string(),
        })],
        destination: SpannedString {
          span: Span::new(11, 38),
          content: "/tmp/dockerfile-content.txt".to_string(),
        },
      }.into()
    );

    Ok(())
  }

  #[test]
  fn copy_heredoc_case_sensitive() -> Result<()> {
    // Terminator should be case sensitive - wrong case should fail
    assert!(parse_single(
      indoc!(r#"
        COPY <<EOF /tmp/test.txt
        content here
        eof
      "#),
      Rule::copy
    ).is_err());

    Ok(())
  }

  #[test]
  fn copy_heredoc_whitespace_delimiter() -> Result<()> {
    // Test that whitespace around delimiter is handled correctly
    assert_eq!(
      parse_single(
        indoc!(r#"
          COPY <<   DELIMITER   /tmp/test.txt
          some content
          DELIMITER
        "#),
        Rule::copy
      )?.into_copy().unwrap(),
      CopyInstruction {
        span: Span { start: 0, end: 58 },
        flags: vec![],
        sources: vec![SourceType::FileContents(SpannedString {
          span: Span::new(36, 49),
          content: "some content\n".to_string(),
        })],
        destination: SpannedString {
          span: Span::new(22, 35),
          content: "/tmp/test.txt".to_string(),
        },
      }.into()
    );

    Ok(())
  }

  #[test]
  fn copy_heredoc_consecutive() -> Result<()> {
    // Test two consecutive COPY heredoc instructions with different delimiters
    use crate::Dockerfile;
    
    let dockerfile_content = indoc!(r#"
      FROM alpine
      COPY <<EOF1 /tmp/first.txt
      first content
      EOF1
      COPY <<EOF2 /tmp/second.txt
      second content
      EOF2
    "#);
    
    let dockerfile = Dockerfile::parse(dockerfile_content)?;
    
    // Check first COPY instruction
    let first_copy = dockerfile.instructions[1].clone().into_copy().unwrap();
    assert_eq!(first_copy.sources.len(), 1);
    match &first_copy.sources[0] {
      SourceType::FileContents(content) => {
        assert_eq!(content.content, "first content\n");
      }
      _ => panic!("Expected FileContents for first COPY"),
    }
    assert_eq!(first_copy.destination.content, "/tmp/first.txt");
    
    // Check second COPY instruction
    let second_copy = dockerfile.instructions[2].clone().into_copy().unwrap();
    assert_eq!(second_copy.sources.len(), 1);
    match &second_copy.sources[0] {
      SourceType::FileContents(content) => {
        assert_eq!(content.content, "second content\n");
      }
      _ => panic!("Expected FileContents for second COPY"),
    }
    assert_eq!(second_copy.destination.content, "/tmp/second.txt");
    
    Ok(())
  }
}