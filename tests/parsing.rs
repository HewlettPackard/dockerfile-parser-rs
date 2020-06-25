// (C) Copyright 2019-2020 Hewlett Packard Enterprise Development LP

extern crate dockerfile_parser;

use dockerfile_parser::*;

mod common;
use common::*;

#[test]
fn parse_basic() -> Result<(), dockerfile_parser::Error> {
  let dockerfile = Dockerfile::parse(r#"
    FROM alpine:3.10

    RUN apk add --no-cache curl
  "#)?;

  assert_eq!(dockerfile.instructions.len(), 2);

  assert_eq!(
    dockerfile.instructions[0],
    Instruction::From(FromInstruction {
      span: Span { start: 5, end: 21 },
      image: "alpine:3.10".into(),
      image_span: Span { start: 10, end: 21 },
      image_parsed: ImageRef {
        registry: None,
        image: "alpine".into(),
        tag: Some("3.10".into()),
        hash: None
      },
      index: 0,
      alias: None,
      alias_span: None
    })
  );

  assert_eq!(
    dockerfile.instructions[1],
    Instruction::Run(RunInstruction::Shell("apk add --no-cache curl".into()))
  );

  Ok(())
}

#[test]
fn parse_multiline_shell() -> Result<(), dockerfile_parser::Error> {
  let dockerfile = Dockerfile::parse(r#"
    RUN apk add --no-cache \
        curl

    RUN foo
  "#)?;

  assert_eq!(dockerfile.instructions.len(), 2);

  // note: 9 spaces due to 1 before the \ + 8 for indent
  assert_eq!(
    dockerfile.instructions[0],
    Instruction::Run(RunInstruction::Shell(
      "apk add --no-cache         curl".into()
    ))
  );

  assert_eq!(
    dockerfile.instructions[1],
    Instruction::Run(RunInstruction::Shell("foo".into()))
  );

  Ok(())
}

#[test]
fn parse_multiline_exec() -> Result<(), dockerfile_parser::Error> {
  let dockerfile = Dockerfile::parse(r#"
    RUN ["apk", \
         "add", \
         "--no-cache", \
         "curl"]

    RUN foo
  "#)?;

  assert_eq!(dockerfile.instructions.len(), 2);

  // note: 9 spaces due to 1 before the \ + 8 for indent
  assert_eq!(
    dockerfile.instructions[0],
    Instruction::Run(RunInstruction::Exec(strings(&[
      "apk", "add", "--no-cache", "curl"
    ])))
  );

  assert_eq!(
    dockerfile.instructions[1],
    Instruction::Run(RunInstruction::Shell("foo".into()))
  );

  Ok(())
}

#[test]
fn parse_label() -> Result<(), dockerfile_parser::Error> {
  let dockerfile = Dockerfile::parse(r#"
    LABEL foo=bar

    LABEL "foo"="bar"

    LABEL "foo=bar"=bar

    LABEL foo="bar\
          baz"

    RUN foo
  "#)?;

  assert_eq!(dockerfile.instructions.len(), 5);

  assert_eq!(
    dockerfile.instructions[0],
    Instruction::Label(LabelInstruction(vec![
      Label::new("foo", "bar")
    ]))
  );

  assert_eq!(
    dockerfile.instructions[1],
    Instruction::Label(LabelInstruction(vec![
      Label::new("foo", "bar")
    ]))
  );

  assert_eq!(
    dockerfile.instructions[2],
    Instruction::Label(LabelInstruction(vec![
      Label::new("foo=bar", "bar")
    ]))
  );

  assert_eq!(
    dockerfile.instructions[3],
    Instruction::Label(LabelInstruction(vec![
      Label::new("foo", "bar          baz")
    ]))
  );

  assert_eq!(
    dockerfile.instructions[4],
    Instruction::Run(RunInstruction::Shell("foo".into()))
  );

  Ok(())
}

#[test]
fn parse_comment() -> Result<(), dockerfile_parser::Error> {
  let dockerfile = Dockerfile::parse(r#"
    # lorem ipsum
    LABEL foo=bar
    #dolor sit amet
    # consectetur adipiscing elit

    # sed do eiusmod
    # Duis aute irure dolor
    # tempor incididunt ut labore
    LABEL "foo"="bar"
    # et dolore magna aliqua
    LABEL "foo=bar"=bar
    #Ut enim ad minim veniam
    LABEL foo="bar\
          baz"
    # quis nostrud exercitation

    # ullamco laboris nisi

    RUN foo
  "#)?;

  assert_eq!(dockerfile.instructions.len(), 5);

  assert_eq!(
    dockerfile.instructions[4],
    Instruction::Run(RunInstruction::Shell("foo".into()))
  );

  Ok(())
}
