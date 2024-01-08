// Copyright (c) Sony Pictures Imageworks, et al.
// SPDX-License-Identifier: Apache-2.0
// https://github.com/imageworks/spk

use rstest::rstest;

use super::{parse_build, Build, SRC};
use crate::ident_build::Digest;

#[rstest]
fn test_parse_build_src() {
    // should allow non-digest if it's a special token
    assert!(parse_build(SRC).is_ok());
}

#[rstest]
fn test_parse_build() {
    assert!(parse_build(Digest::default().to_string()).is_ok());
    assert!(parse_build("not eight characters").is_err());
    assert!(parse_build("invalid.").is_err())
}

#[rstest]
fn test_null_is_null() {
    let expected = Build::Digest(
        spfs::Digest::from(spfs::encoding::NULL_DIGEST)
            .to_string()
            .chars()
            .take(Digest::SIZE)
            .collect::<Vec<_>>()
            .try_into()
            .expect("digest string has at least the characters needed"),
    );
    let actual = Build::null();
    assert_eq!(
        actual, &expected,
        "Hard-coded null build digest should be the same when computed"
    )
}

#[rstest]
fn test_empty_is_empty() {
    let expected = Build::Digest(Digest::default());
    let actual = Build::empty();
    assert_eq!(
        actual, &expected,
        "Hard-coded empty build digest should be the same when computed"
    )
}
