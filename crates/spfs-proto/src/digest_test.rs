// Copyright (c) Contributors to the SPK project.
// SPDX-License-Identifier: Apache-2.0
// https://github.com/spkenv/spk

use std::convert::TryInto;

use ring::digest;
use rstest::rstest;

#[rstest]
fn test_empty_digest_bytes() {
    use crate::{DIGEST_SIZE, EMPTY_DIGEST};
    let empty_digest: [u8; DIGEST_SIZE] = digest::digest(&digest::SHA256, b"")
        .as_ref()
        .try_into()
        .unwrap();
    assert_eq!(empty_digest, EMPTY_DIGEST);
}
