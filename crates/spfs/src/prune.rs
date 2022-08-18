// Copyright (c) 2021 Sony Pictures Imageworks, et al.
// SPDX-License-Identifier: Apache-2.0
// https://github.com/imageworks/spk

use chrono::prelude::*;
use tokio_stream::StreamExt;

use crate::{storage, tracking, Result};
use std::{collections::HashSet, convert::TryInto};

#[cfg(test)]
#[path = "./prune_test.rs"]
mod prune_test;

/// Specifies a range of conditions for pruning tags out of a repository.
#[derive(Debug, Default)]
pub struct PruneParameters {
    pub prune_if_older_than: Option<DateTime<Utc>>,
    pub keep_if_newer_than: Option<DateTime<Utc>>,
    pub prune_if_version_more_than: Option<u64>,
    pub keep_if_version_less_than: Option<u64>,
}

impl PruneParameters {
    pub fn should_prune(&self, spec: &tracking::TagSpec, tag: &tracking::Tag) -> bool {
        if let Some(keep_if_version_less_than) = self.keep_if_version_less_than {
            if spec.version() < keep_if_version_less_than {
                return false;
            }
        }
        if let Some(keep_if_newer_than) = self.keep_if_newer_than {
            if tag.time > keep_if_newer_than {
                return false;
            }
        }

        if let Some(prune_if_version_more_than) = self.prune_if_version_more_than {
            if spec.version() > prune_if_version_more_than {
                return true;
            }
        }
        if let Some(prune_if_older_than) = self.prune_if_older_than {
            if tag.time < prune_if_older_than {
                return true;
            }
        }

        false
    }
}

pub async fn get_prunable_tags(
    tags: &storage::RepositoryHandle,
    params: &PruneParameters,
) -> Result<HashSet<tracking::Tag>> {
    let mut to_prune = HashSet::new();
    let mut tag_streams = tags.iter_tag_streams();
    while let Some(res) = tag_streams.next().await {
        let (spec, stream) = res?;
        let mut stream = futures::StreamExt::enumerate(stream);
        tracing::debug!("searching for history to prune in {}", spec.to_string());
        while let Some((version, tag)) = stream.next().await {
            let tag = tag?;
            let versioned_spec = tracking::build_tag_spec(
                spec.org(),
                spec.name(),
                version.try_into().expect("usize fits into u64"),
            )?;
            if params.should_prune(&versioned_spec, &tag) {
                to_prune.insert(tag);
            }
        }
    }

    Ok(to_prune)
}

pub async fn prune_tags(
    repo: &storage::RepositoryHandle,
    params: &PruneParameters,
) -> Result<HashSet<tracking::Tag>> {
    let to_prune = get_prunable_tags(repo, params).await?;
    for tag in to_prune.iter() {
        tracing::trace!(?tag, "removing tag");
        repo.remove_tag(tag).await?;
    }
    Ok(to_prune)
}