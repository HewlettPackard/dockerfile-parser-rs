// (C) Copyright 2019 Hewlett Packard Enterprise Development LP

use pest;

#[derive(Parser)]
#[grammar = "dockerfile.pest"]
pub(crate) struct DockerfileParser;

pub(crate) type Pair<'a> = pest::iterators::Pair<'a, Rule>;
