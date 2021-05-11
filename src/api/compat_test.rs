// Copyright (c) 2021 Sony Pictures Imageworks, et al.
// SPDX-License-Identifier: Apache-2.0
// https://github.com/imageworks/spk

use rstest::rstest;

use super::parse_compat;
use crate::api::parse_version;

#[rstest]
#[case("x.x.x", "1.0.0", "1.0.0", true)]
#[case("x.x.a", "1.0.0", "1.0.2", true)]
#[case("x.x.x", "1.0.0", "1.0.2", false)]
// all prior numbers must be equal
#[case("x.b.a", "1.0.0", "1.1.0", false)]
// compatible regardless of abi specification
#[case("x.a", "1.0.0", "1.1.0", true)]
// not compatible if api compat is missing
#[case("x.b", "1.0.0", "1.1.0", false)]
// compatible if both are provided
#[case("x.ba", "1.0.0", "1.1.0", true)]
fn test_compat_api(#[case] compat: &str, #[case] a: &str, #[case] b: &str, #[case] expected: bool) {
    let actual = parse_compat(compat)
        .unwrap()
        .is_api_compatible(&parse_version(a).unwrap(), &parse_version(b).unwrap());
    assert_eq!(actual.is_ok(), expected);
}

#[rstest]
#[case("x.x.x", "1.0.0", "1.0.0", true)]
#[case("x.x.b", "1.0.0", "1.0.2", true)]
#[case("x.x.x", "1.0.0", "1.0.2", false)]
#[case("x.b", "1.0.0", "1.1.0", true)]
#[case("x.a", "1.0.0", "1.1.0", false)]
#[case("x.a.b", "3.6.5", "3.7.1", false)]
#[case("x.a.b", "3.7.1", "3.7.5", true)]
fn test_compat_abi(#[case] compat: &str, #[case] a: &str, #[case] b: &str, #[case] expected: bool) {
    let actual = parse_compat(compat)
        .unwrap()
        .is_binary_compatible(&parse_version(a).unwrap(), &parse_version(b).unwrap());
    println!("{}", actual);
    assert_eq!(actual.is_ok(), expected);
}
