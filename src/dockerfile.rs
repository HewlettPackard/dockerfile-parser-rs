// (C) Copyright 2019 Hewlett Packard Enterprise Development LP

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

#[derive(Debug, Clone)]
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
        // args preceeding the first FROM instruction may be substituted into
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

pub struct Stage<'a> {
  pub index: usize,
  pub instructions: Vec<&'a Instruction>
}

/// Iterates over build stages, aka a FROM instruction and all proceeding
/// instructions until the next FROM
pub struct StageIterator<'a> {
  dockerfile: &'a Dockerfile,
  stage_index: usize,
  instruction_index: usize
}

impl<'a> Iterator for StageIterator<'a> {
  type Item = Stage<'a>;

  fn next(&mut self) -> Option<Stage<'a>> {
    let mut instructions = Vec::new();

    // instructions before the first FROM are not part of any stage and should
    // be skipped
    // to simplify things we generalize this and skip all instructions from
    // `instruction_index` until the first FROM, regardless of whether or not
    // this is the beginning of the entire Dockerfile
    let mut preamble = true;

    let mut iter = self.dockerfile.instructions.iter()
      .skip(self.instruction_index)
      .peekable();

    while let Some(ins) = iter.next() {
      self.instruction_index += 1;

      // skip until the first FROM
      if preamble {
        if let Instruction::From(_) = ins {
          preamble = false;
        } else {
          continue;
        }
      }

      instructions.push(ins);

      // this stage ends before the next FROM
      if let Some(Instruction::From(_)) = iter.peek() {
        break;
      }
    }

    if instructions.is_empty() {
      None
    } else {
      let stage = Stage {
        index: self.stage_index,
        instructions
      };

      self.stage_index += 1;

      Some(stage)
    }
  }
}

impl Dockerfile {
  pub fn parse(input: &str) -> Result<Dockerfile> {
    parse_dockerfile(input)
  }

  pub fn from_reader<R>(reader: R) -> Result<Dockerfile>
  where
    R: Read
  {
    let mut buf = String::new();
    let mut buf_reader = BufReader::new(reader);
    buf_reader.read_to_string(&mut buf).context(ReadError)?;

    Dockerfile::parse(&buf)
  }

  pub fn iter_stages(&self) -> StageIterator {
    StageIterator {
      dockerfile: &self,
      stage_index: 0,
      instruction_index: 0
    }
  }

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
