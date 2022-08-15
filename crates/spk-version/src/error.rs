// Copyright (c) 2022 Sony Pictures Imageworks, et al.
// SPDX-License-Identifier: Apache-2.0
// https://github.com/imageworks/spk

use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    #[error(transparent)]
    InvalidVersionError(#[from] crate::InvalidVersionError),
    #[error(transparent)]
    SpkNameError(#[from] spk_name::Error),
    #[error("Error: {0}")]
    String(String),
}
