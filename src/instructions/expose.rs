use std::convert::TryFrom;

use crate::error::*;
use crate::{parse_short, Instruction, Pair, Rule, Span, SpannedShort, SpannedString};

use snafu::ResultExt;

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct ExposePort {
    pub span: Span,
    pub port: SpannedShort,
    pub protocol: Option<SpannedString>,
}

impl ExposePort {
    pub fn new(span: Span, port: SpannedShort, protocol: Option<SpannedString>) -> Self {
        ExposePort {
            span,
            port,
            protocol,
        }
    }
}

/// https://docs.docker.com/reference/dockerfile/#expose
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct ExposeInstruction {
    pub span: Span,
    pub vars: Vec<ExposePort>,
}

fn parse_expose_port(record: Pair) -> Result<ExposePort> {
    let span = Span::from_pair(&record);
    let mut port = None;
    let mut protocol = None;

    for field in record.into_inner() {
        match field.as_rule() {
            Rule::expose_port_number => port = Some(parse_short(&field)?),
            Rule::expose_protocol => {
                protocol = Some(SpannedString {
                    span: Span::from_pair(&field),
                    content: field.as_str().to_owned(),
                });
            }
            _ => return Err(unexpected_token(field)),
        }
    }

    let port = port.ok_or_else(|| Error::GenericParseError {
        message: "env pair requires a key".into(),
    })?;

    Ok(ExposePort {
        span,
        port,
        protocol,
    })
}

impl ExposeInstruction {
    pub(crate) fn from_record(record: Pair) -> Result<ExposeInstruction> {
        let span = Span::from_pair(&record);
        let mut vars = Vec::new();

        for field in record.into_inner() {
            match field.as_rule() {
                Rule::expose_port => vars.push(parse_expose_port(field)?),
                _ => return Err(unexpected_token(field)),
            }
        }

        Ok(ExposeInstruction { span, vars })
    }
}

impl<'a> TryFrom<&'a Instruction> for &'a ExposeInstruction {
    type Error = Error;

    fn try_from(instruction: &'a Instruction) -> std::result::Result<Self, Self::Error> {
        if let Instruction::Expose(e) = instruction {
            Ok(e)
        } else {
            Err(Error::ConversionError {
                from: format!("{:?}", instruction),
                to: "ExposeInstruction".into(),
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
    use crate::Dockerfile;

    #[test]
    fn expose() -> Result<()> {
        assert_eq!(
            parse_single(r#"expose 8000"#, Rule::expose)?
                .into_expose()
                .unwrap(),
            ExposeInstruction {
                span: Span::new(0, 11),
                vars: vec![ExposePort::new(
                    Span::new(7, 11),
                    SpannedShort {
                        span: Span::new(7, 11),
                        content: 8000,
                    },
                    None,
                )],
            }
        );

        assert_eq!(
            parse_single(r#"expose 8000/udp"#, Rule::expose)?
                .into_expose()
                .unwrap(),
            ExposeInstruction {
                span: Span::new(0, 15),
                vars: vec![ExposePort::new(
                    Span::new(7, 15),
                    SpannedShort {
                        span: Span::new(7, 11),
                        content: 8000,
                    },
                    Some(SpannedString {
                        span: Span::new(12, 15),
                        content: "udp".to_owned()
                    }),
                )],
            }
        );

        Ok(())
    }

    #[test]
    fn test_multiline_single_env() -> Result<()> {
        assert_eq!(
            parse_single(
                r#"expose 80 8000/udp \
            8080/tcp 8096"#,
                Rule::expose
            )?
            .into_expose()
            .unwrap(),
            ExposeInstruction {
                span: Span::new(0, 46),
                vars: vec![
                    ExposePort::new(
                        Span::new(7, 9),
                        SpannedShort {
                            span: Span::new(7, 9),
                            content: 80,
                        },
                        None
                    ),
                    ExposePort::new(
                        Span::new(10, 18),
                        SpannedShort {
                            span: Span::new(10, 14),
                            content: 8000,
                        },
                        Some(SpannedString {
                            span: Span::new(15, 18),
                            content: "udp".to_owned()
                        })
                    ),
                    ExposePort::new(
                        Span::new(33, 41),
                        SpannedShort {
                            span: Span::new(33, 37),
                            content: 8080,
                        },
                        Some(SpannedString {
                            span: Span::new(38, 41),
                            content: "tcp".to_owned()
                        })
                    ),
                    ExposePort::new(
                        Span::new(42, 46),
                        SpannedShort {
                            span: Span::new(42, 46),
                            content: 8096,
                        },
                        None
                    ),
                ],
            }
        );

        Ok(())
    }
}
