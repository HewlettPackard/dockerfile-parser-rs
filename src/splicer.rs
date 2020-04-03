// (C) Copyright 2019-2020 Hewlett Packard Enterprise Development LP

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

/// A utility to repeatedly replace spans of text within a larger document.
///
/// Each subsequent call to `Splicer::splice(...)` rewrites the `content` buffer
/// and appends to the list of internal offsets. `Splicer::splice(...)` then
/// adjusts span bounds at call-time to ensures repeated calls to `splice(...)`
/// continue to work even if one or both of the span bounds have shifted.
///
/// # Example
/// ```
/// use dockerfile_parser::*;
///
/// let dockerfile: Dockerfile = r#"
///   FROM alpine:3.10
/// "#.parse()?;
///
/// let from = match &dockerfile.instructions[0] {
///   Instruction::From(f) => f,
///   _ => panic!("invalid")
/// };
///
/// let mut splicer = dockerfile.splicer();
/// splicer.splice(&from.image_span, "alpine:3.11");
///
/// assert_eq!(splicer.content, r#"
///   FROM alpine:3.11
/// "#);
/// # Ok::<(), dockerfile_parser::Error>(())
/// ```
pub struct Splicer {
  /// The current content of the splice buffer.
  pub content: String,

  splice_offsets: Vec<SpliceOffset>
}

impl Splicer {
  /// Creates a new Splicer from the given Dockerfile.
  pub(crate) fn from(dockerfile: &Dockerfile) -> Splicer {
    Splicer {
      content: dockerfile.content.clone(),
      splice_offsets: Vec::new()
    }
  }

  /// Replaces a Span with the given replacement string, mutating the `content`
  /// string.
  ///
  /// Sections may be deleted by replacing them with an empty string (`""`).
  ///
  /// Note that spans are always relative to the *original input document*.
  /// Span offsets are recalculated at call-time to account for previous calls
  /// to `splice(...)` that may have shifted one or both of the span bounds.
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
