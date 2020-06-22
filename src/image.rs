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
    let (registry, image_full) = if parts.len() == 2 && is_registry(parts[0]) {
      // some 3rd party registry
      (Some(parts[0].to_string()), parts[1])
    } else {
      // some other image on the default registry; return the original string
      (None, s)
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

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_image_parse_dockerhub() {
    assert_eq!(
      ImageRef::parse("alpine:3.10"),
      ImageRef {
        registry: None,
        image: "alpine".into(),
        tag: Some("3.10".into())
      }
    );

    assert_eq!(
      ImageRef::parse("foo/bar"),
      ImageRef {
        registry: None,
        image: "foo/bar".into(),
        tag: None
      }
    );

    assert_eq!(
      ImageRef::parse("clux/muslrust"),
      ImageRef {
        registry: None,
        image: "clux/muslrust".into(),
        tag: None
      }
    );

    assert_eq!(
      ImageRef::parse("clux/muslrust:1.41.0-stable"),
      ImageRef {
        registry: None,
        image: "clux/muslrust".into(),
        tag: Some("1.41.0-stable".into())
      }
    );
  }

  #[test]
  fn test_image_parse_registry() {
    assert_eq!(
      ImageRef::parse("quay.io/prometheus/node-exporter:v0.18.1"),
      ImageRef {
        registry: Some("quay.io".into()),
        image: "prometheus/node-exporter".into(),
        tag: Some("v0.18.1".into())
      }
    );

    assert_eq!(
      ImageRef::parse("gcr.io/fake_project/fake_image:fake_tag"),
      ImageRef {
        registry: Some("gcr.io".into()),
        image: "fake_project/fake_image".into(),
        tag: Some("fake_tag".into())
      }
    );

    assert_eq!(
      ImageRef::parse("gcr.io/fake_project/fake_image"),
      ImageRef {
        registry: Some("gcr.io".into()),
        image: "fake_project/fake_image".into(),
        tag: None
      }
    );

    assert_eq!(
      ImageRef::parse("gcr.io/fake_image"),
      ImageRef {
        registry: Some("gcr.io".into()),
        image: "fake_image".into(),
        tag: None
      }
    );

    assert_eq!(
      ImageRef::parse("gcr.io/fake_image:fake_tag"),
      ImageRef {
        registry: Some("gcr.io".into()),
        image: "fake_image".into(),
        tag: Some("fake_tag".into())
      }
    );
  }

  #[test]
  fn test_image_parse_localhost() {
    assert_eq!(
      ImageRef::parse("localhost/foo"),
      ImageRef {
        registry: Some("localhost".into()),
        image: "foo".into(),
        tag: None
      }
    );

    assert_eq!(
      ImageRef::parse("localhost/foo:bar"),
      ImageRef {
        registry: Some("localhost".into()),
        image: "foo".into(),
        tag: Some("bar".into())
      }
    );

    assert_eq!(
      ImageRef::parse("localhost/foo/bar"),
      ImageRef {
        registry: Some("localhost".into()),
        image: "foo/bar".into(),
        tag: None
      }
    );

    assert_eq!(
      ImageRef::parse("localhost/foo/bar:baz"),
      ImageRef {
        registry: Some("localhost".into()),
        image: "foo/bar".into(),
        tag: Some("baz".into())
      }
    );
  }

  #[test]
  fn test_image_parse_registry_port() {
    assert_eq!(
      ImageRef::parse("example.com:1234/foo"),
      ImageRef {
        registry: Some("example.com:1234".into()),
        image: "foo".into(),
        tag: None
      }
    );

    assert_eq!(
      ImageRef::parse("example.com:1234/foo:bar"),
      ImageRef {
        registry: Some("example.com:1234".into()),
        image: "foo".into(),
        tag: Some("bar".into())
      }
    );

    assert_eq!(
      ImageRef::parse("example.com:1234/foo/bar"),
      ImageRef {
        registry: Some("example.com:1234".into()),
        image: "foo/bar".into(),
        tag: None
      }
    );

    assert_eq!(
      ImageRef::parse("example.com:1234/foo/bar:baz"),
      ImageRef {
        registry: Some("example.com:1234".into()),
        image: "foo/bar".into(),
        tag: Some("baz".into())
      }
    );

    // docker hub doesn't allow it, but other registries can allow arbitrarily
    // nested images
    assert_eq!(
      ImageRef::parse("example.com:1234/foo/bar/baz:qux"),
      ImageRef {
        registry: Some("example.com:1234".into()),
        image: "foo/bar/baz".into(),
        tag: Some("qux".into())
      }
    );
  }
}
