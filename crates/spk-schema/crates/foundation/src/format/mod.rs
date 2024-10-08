// Copyright (c) Contributors to the SPK project.
// SPDX-License-Identifier: Apache-2.0
// https://github.com/spkenv/spk

mod error;

pub use error::{Error, Result};

use crate::name::{PkgName, RepositoryNameBuf};

/// Helper to hold values that affect the formatting of a request
pub struct FormatChangeOptions {
    pub verbosity: u8,
    pub level: u64,
}

impl Default for FormatChangeOptions {
    fn default() -> Self {
        Self {
            verbosity: 0,
            level: u64::MAX,
        }
    }
}

pub trait FormatBuild {
    fn format_build(&self) -> String;
}

pub trait FormatChange {
    type State;

    fn format_change(
        &self,
        format_settings: &FormatChangeOptions,
        state: Option<&Self::State>,
    ) -> String;
}

pub trait FormatComponents {
    fn format_components(&self) -> String;
}

#[async_trait::async_trait]
pub trait FormatError {
    async fn format_error(&self, verbosity: u8) -> String;
}

pub trait FormatIdent {
    fn format_ident(&self) -> String;
}

pub trait FormatOptionMap {
    fn format_option_map(&self) -> String;
}

pub trait FormatRequest {
    type PkgRequest;

    /// Create a canonical string to describe the combined request for a package.
    fn format_request(
        &self,
        repository_name: Option<&RepositoryNameBuf>,
        name: &PkgName,
        format_settings: &FormatChangeOptions,
    ) -> String;
}

pub trait FormatSolution {
    fn format_solution(&self, verbosity: u8) -> String;
}
