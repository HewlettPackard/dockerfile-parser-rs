// (C) Copyright 2019-2020 Hewlett Packard Enterprise Development LP

use std::convert::TryFrom;
use std::io::{Read, BufReader};
use std::str::FromStr;

use pest::Parser;
use snafu::ResultExt;

pub use crate::image::*;
pub use crate::error::*;
pub use crate::parser::*;
pub use crate::instructions::*;
pub use crate::splicer::*;
pub use crate::stage::*;

/// A single Dockerfile instruction.
///
/// Individual instructions structures may be unpacked with pattern matching or
/// via the `TryFrom` impls on each instruction type.
///
/// # Example
///
/// ```
/// use std::convert::TryInto;
/// use dockerfile_parser::*;
///
/// let dockerfile = Dockerfile::parse("FROM alpine:3.11").unwrap();
/// let from: &FromInstruction = dockerfile.instructions
///   .get(0).unwrap()
///   .try_into().unwrap();
///
/// assert_eq!(from.image_parsed.tag, Some("3.11".to_string()));
/// ```
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Instruction {
  From(FromInstruction),
  Arg(ArgInstruction),
  Label(LabelInstruction),
  Run(RunInstruction),
  Entrypoint(EntrypointInstruction),
  Cmd(CmdInstruction),
  Copy(CopyInstruction),
  Env(EnvInstruction),
  Misc(MiscInstruction)
}

/// Maps an instruction struct to its enum variant, implementing From<T> on
/// Instruction for it.
macro_rules! impl_from_instruction {
  ($struct:ident, $enum:expr) => {
    impl From<$struct> for Instruction {
      fn from(ins: $struct) -> Self {
        $enum(ins)
      }
    }
  };
}

impl_from_instruction!(FromInstruction, Instruction::From);
impl_from_instruction!(ArgInstruction, Instruction::Arg);
impl_from_instruction!(LabelInstruction, Instruction::Label);
impl_from_instruction!(RunInstruction, Instruction::Run);
impl_from_instruction!(EntrypointInstruction, Instruction::Entrypoint);
impl_from_instruction!(CmdInstruction, Instruction::Cmd);
impl_from_instruction!(CopyInstruction, Instruction::Copy);
impl_from_instruction!(EnvInstruction, Instruction::Env);
impl_from_instruction!(MiscInstruction, Instruction::Misc);

impl TryFrom<Pair<'_>> for Instruction {
  type Error = Error;

  fn try_from(record: Pair) -> std::result::Result<Self, Self::Error> {
    let instruction: Instruction = match record.as_rule() {
      Rule::from => FromInstruction::from_record(record, 0)?.into(),
      Rule::arg => ArgInstruction::from_record(record)?.into(),
      Rule::label => LabelInstruction::from_record(record)?.into(),

      Rule::run_exec => RunInstruction::from_exec_record(record)?.into(),
      Rule::run_shell => RunInstruction::from_shell_record(record)?.into(),

      Rule::entrypoint_exec =>
        EntrypointInstruction::from_exec_record(record)?.into(),
      Rule::entrypoint_shell =>
        EntrypointInstruction::from_shell_record(record)?.into(),

      Rule::cmd_exec => CmdInstruction::from_exec_record(record)?.into(),
      Rule::cmd_shell => CmdInstruction::from_shell_record(record)?.into(),

      Rule::copy => Instruction::Copy(CopyInstruction::from_record(record)?),

      Rule::env_single => EnvInstruction::from_single_record(record)?.into(),
      Rule::env_pairs => EnvInstruction::from_pairs_record(record)?.into(),

      Rule::misc => MiscInstruction::from_record(record)?.into(),
      _ => return Err(unexpected_token(record))
    };

    Ok(instruction)
  }
}

/// A parsed Dockerfile.
///
/// An ordered list of all instructions is available via `instructions`, and
/// individual stages in a multi-stage build may be iterated over using
/// `Dockerfile::iter_stages()`.
///
/// # Example
/// ```
/// use dockerfile_parser::Dockerfile;
/// use std::io::Read;
///
/// let s = r#"
///   FROM alpine:3.11
///   RUN echo "hello world"
/// "#;
///
/// assert_eq!(
///   Dockerfile::parse(&s).unwrap(),
///   s.parse::<Dockerfile>().unwrap()
/// );
/// assert_eq!(
///   Dockerfile::parse(&s).unwrap(),
///   Dockerfile::from_reader(s.as_bytes()).unwrap()
/// );
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct Dockerfile {
  /// The raw content of the Dockerfile
  pub content: String,

  /// An ordered list of parsed ARG instructions preceding the first FROM
  pub global_args: Vec<ArgInstruction>,

  /// An ordered list of all parsed instructions, including global_args
  pub instructions: Vec<Instruction>
}

fn parse_dockerfile(input: &str) -> Result<Dockerfile> {
  let dockerfile = DockerfileParser::parse(Rule::dockerfile, input)
    .context(ParseError)?
    .next()
    .ok_or(Error::UnknownParseError)?;

  let mut instructions = Vec::new();
  let mut global_args = Vec::new();
  let mut from_found = false;
  let mut from_index = 0;

  for record in dockerfile.into_inner() {
    if let Rule::EOI = record.as_rule() {
      continue;
    }

    let mut instruction = Instruction::try_from(record)?;
    match &mut instruction {
      Instruction::From(ref mut from) => {
        // fix the from index since we can't know that in parse_instruction()
        from.index = from_index;
        from_index += 1;
        from_found = true;
      },
      Instruction::Arg(ref arg) => {
        // args preceding the first FROM instruction may be substituted into
        // all subsequent FROM image refs
        if !from_found {
          global_args.push(arg.clone());
        }
      },
      _ => ()
    };

    instructions.push(instruction);
  }

  Ok(Dockerfile {
    content: input.into(),
    global_args, instructions
  })
}

impl Dockerfile {
  /// Parses a Dockerfile from a string.
  pub fn parse(input: &str) -> Result<Dockerfile> {
    parse_dockerfile(input)
  }

  /// Parses a Dockerfile from a reader.
  pub fn from_reader<R>(reader: R) -> Result<Dockerfile>
  where
    R: Read
  {
    let mut buf = String::new();
    let mut buf_reader = BufReader::new(reader);
    buf_reader.read_to_string(&mut buf).context(ReadError)?;

    Dockerfile::parse(&buf)
  }

  /// Returns a `Stages`, which splits this Dockerfile into its build stages.
  pub fn stages(&self) -> Stages {
    Stages::new(self)
  }

  pub fn iter_stages(&self) -> std::vec::IntoIter<Stage<'_>> {
    self.stages().into_iter()
  }

  /// Creates a `Splicer` for this Dockerfile.
  ///
  /// Note that the original input string is needed to actually perform any
  /// splicing.
  pub fn splicer(&self) -> Splicer {
    Splicer::from(self)
  }
}

impl FromStr for Dockerfile {
  type Err = Error;

  fn from_str(s: &str) -> Result<Self, Self::Err> {
    Dockerfile::parse(s)
  }
}
