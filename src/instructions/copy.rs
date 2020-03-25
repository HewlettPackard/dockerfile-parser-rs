// (C) Copyright 2019 Hewlett Packard Enterprise Development LP

use snafu::ensure;

use crate::parser::{Pair, Rule};
use crate::error::*;

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct CopyFlag {
  pub name: String,
  pub value: String
}

impl CopyFlag {
  fn from_record(record: Pair) -> Result<CopyFlag> {
    let mut name = None;
    let mut value = None;

    for field in record.into_inner() {
      match field.as_rule() {
        Rule::copy_flag_name => name = Some(field.as_str()),
        Rule::copy_flag_value => value = Some(field.as_str()),
        _ => return Err(unexpected_token(field))
      }
    }

    let name = name.ok_or_else(|| Error::GenericParseError {
      message: "copy flags require a key".into(),
    })?.to_string();

    let value = value.ok_or_else(|| Error::GenericParseError {
      message: "copy flags require a value".into()
    })?.to_string();

    Ok(CopyFlag {
      name, value
    })
  }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct CopyInstruction {
  pub flags: Vec<CopyFlag>,
  pub sources: Vec<String>,
  pub destination: String
}

impl CopyInstruction {
  pub(crate) fn from_record(record: Pair) -> Result<CopyInstruction> {
    let mut flags = Vec::new();
    let mut paths = Vec::new();

    for field in record.into_inner() {
      match field.as_rule() {
        Rule::copy_flag => flags.push(CopyFlag::from_record(field)?),
        Rule::copy_pathspec => paths.push(field.as_str().to_string()),
        _ => return Err(unexpected_token(field))
      }
    }

    ensure!(
      paths.len() >= 2,
      GenericParseError {
        message: "copy requires at least one source and a destination"
      }
    );

    // naughty unwrap, but we know there's something to pop
    let destination = paths.pop().unwrap();

    Ok(CopyInstruction {
      flags,
      sources: paths,
      destination
    })
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::test_util::*;

  #[test]
  fn copy_basic() -> Result<()> {
    assert_eq!(
      parse_single("copy foo bar", Rule::copy)?,
      CopyInstruction {
        flags: vec![],
        sources: strings(&["foo"]),
        destination: "bar".to_string()
      }.into()
    );

    Ok(())
  }

  #[test]
  fn copy_multiple_sources() -> Result<()> {
    assert_eq!(
      parse_single("copy foo bar baz qux", Rule::copy)?,
      CopyInstruction {
        flags: vec![],
        sources: strings(&["foo", "bar", "baz"]),
        destination: "qux".into()
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
        flags: vec![],
        sources: strings(&["foo"]),
        destination: "bar".into()
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
        flags: vec![
          CopyFlag { name: "from".into(), value: "alpine:3.10".into() }
        ],
        sources: strings(&["/usr/lib/libssl.so.1.1"]),
        destination: "/tmp/".into()
      }.into()
    );

    Ok(())
  }
}
