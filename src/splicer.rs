// (C) Copyright 2019 Hewlett Packard Enterprise Development LP

use std::convert::TryInto;

use crate::parser::Pair;
use crate::dockerfile::Dockerfile;

/// An offset used to adjust proceeding Spans after content has been spliced
#[derive(Debug)]
struct SpliceOffset {
  position: usize,
  offset: isize
}

/// A byte-index tuple representing a span of characters in a string
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Span {
  pub start: usize,
  pub end: usize
}

impl Span {
  pub(crate) fn from_pair(record: &Pair) -> Span {
    let pest_span = record.as_span();

    Span {
      start: pest_span.start(),
      end: pest_span.end()
    }
  }

  fn adjust_offsets(&self, offsets: &[SpliceOffset]) -> Span {
    let mut start = self.start as isize;
    let mut end = self.end as isize;

    for splice in offsets {
      if splice.position < start as usize {
        start += splice.offset;
        end += splice.offset;
      } else if splice.position < end as usize {
        end += splice.offset;
      }
    }

    Span {
      start: start.try_into().ok().unwrap_or(0),
      end: end.try_into().ok().unwrap_or(0)
    }
  }
}

pub struct Splicer {
  pub content: String,

  splice_offsets: Vec<SpliceOffset>
}

impl Splicer {
  pub(crate) fn from(dockerfile: &Dockerfile) -> Splicer {
    Splicer {
      content: dockerfile.content.clone(),
      splice_offsets: Vec::new()
    }
  }

  pub fn splice(&mut self, span: &Span, replacement: &str) {
    let span = span.adjust_offsets(&self.splice_offsets);

    // determine the splice offset (only used on subsequent splices)
    let prev_len = span.end - span.start;
    let new_len = replacement.len();
    let offset = new_len as isize - prev_len as isize;
    self.splice_offsets.push(
      SpliceOffset { position: span.start, offset }
    );

    // split and rebuild the content with the replacement instead
    let (beginning, rest) = self.content.split_at(span.start);
    let (_, end) = rest.split_at(span.end - span.start);
    self.content = format!("{}{}{}", beginning, replacement, end);
  }
}
