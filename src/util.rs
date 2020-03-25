// (C) Copyright 2019 Hewlett Packard Enterprise Development LP

use crate::error::*;
use crate::parser::*;

use enquote::unquote;
use snafu::ResultExt;

/// Given a node ostensibly containing a string array, returns an unescaped
/// array of strings
pub(crate) fn parse_string_array(array: Pair) -> Result<Vec<String>> {
  let mut ret = Vec::new();

  for field in array.into_inner() {
    match field.as_rule() {
      Rule::string => {
        let s = unquote(field.as_str()).context(UnescapeError)?;

        ret.push(s);
      },
      _ => return Err(unexpected_token(field))
    }
  }

  Ok(ret)
}

/// Removes escaped line breaks (\\\n) from a string
///
/// This should be used to clean any input from the any_breakable rule
pub(crate) fn clean_escaped_breaks(s: &str) -> String {
  s.replace("\\\n", "")
}
