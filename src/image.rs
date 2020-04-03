// (C) Copyright 2019-2020 Hewlett Packard Enterprise Development LP

use std::fmt;

/// A parsed docker image reference
///
/// The `Display` impl may be used to convert a parsed image back to a plain
/// string:
/// ```
/// use dockerfile_parser::ImageRef;
///
/// let image = ImageRef::parse("alpine:3.11");
/// assert_eq!(image.registry, None);
/// assert_eq!(image.image, "alpine");
/// assert_eq!(image.tag, Some("3.11".to_string()));
/// assert_eq!(format!("{}", image), "alpine:3.11");
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageRef {
  /// an optional registry, generally Docker Hub if unset
  pub registry: Option<String>,

  /// an image string, possibly including a user or organization name
  pub image: String,

  /// An optional image tag (after the colon, e.g. `:1.2.3`), generally inferred
  /// to mean `:latest` if unset
  pub tag: Option<String>
}

/// Determines if an ImageRef token refers to a registry hostname or not
///
/// Based on rules from https://stackoverflow.com/a/42116190
fn is_registry(token: &str) -> bool {
  token == "localhost" || token.contains('.') || token.contains(':')
}

impl ImageRef {
  /// Parses an `ImageRef` from a string.
  ///
  /// This is not fallible, however malformed image strings may return
  /// unexpected results.
  pub fn parse(s: &str) -> ImageRef {
    // tags may be one of:
    // foo (implies registry.hub.docker.com/library/foo:latest)
    // foo:bar (implies registry.hub.docker.com/library/foo:bar)
    // org/foo:bar (implies registry.hub.docker.com/org/foo:bar)

    // per https://stackoverflow.com/a/42116190, some extra rules are needed to
    // disambiguate external registries
    // localhost/foo:bar is allowed (localhost is special)
    // example.com/foo:bar is allowed
    // host/foo:bar is not allowed (conflicts with docker hub)
    // host:443/foo:bar is allowed (':' or '.' make it unambiguous)

    // we don't attempt to actually validate tags otherwise, so invalid
    // characters could slip through

    let parts: Vec<&str> = s.splitn(2, '/').collect();
    let (registry, image_full) = if parts.len() == 1 {
      (None, parts[0])
    } else if is_registry(parts[0]) {
      (Some(parts[0].to_string()), parts[1])
    } else {
      (None, parts[0])
    };

    // parts length is guaranteed to be at least 1 given an empty string
    let parts: Vec<&str> = image_full.splitn(2, ':').collect();
    let image = parts[0].to_string();
    let tag = parts.get(1).map(|p| String::from(*p));

    ImageRef { registry, image, tag }
  }
}

impl fmt::Display for ImageRef {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    if let Some(registry) = &self.registry {
      write!(f, "{}/", registry)?;
    }

    write!(f, "{}", self.image)?;

    if let Some(tag) = &self.tag {
      write!(f, ":{}", tag)?;
    }

    Ok(())
  }
}
