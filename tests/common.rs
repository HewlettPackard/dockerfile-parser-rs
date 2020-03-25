// (C) Copyright 2019 Hewlett Packard Enterprise Development LP

pub fn strings(strs: &[&str]) -> Vec<String> {
  strs.iter().map(|s| String::from(*s)).collect()
}
