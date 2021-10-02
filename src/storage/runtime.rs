// Copyright (c) 2021 Sony Pictures Imageworks, et al.
// SPDX-License-Identifier: Apache-2.0
// https://github.com/imageworks/spk
use std::collections::HashMap;
use std::path::PathBuf;

use spfs::prelude::*;

use super::Repository;
use crate::{api, Error, Result};

#[derive(Clone, Debug)]
pub struct RuntimeRepository {
    root: std::path::PathBuf,
}

impl Default for RuntimeRepository {
    fn default() -> Self {
        Self {
            root: std::path::PathBuf::from("/spfs/spk/pkg"),
        }
    }
}

impl Repository for RuntimeRepository {
    fn list_packages(&self) -> Result<Vec<String>> {
        Ok(get_all_filenames("/spfs/spk/pkg")?
            .into_iter()
            .filter_map(|entry| {
                if entry.ends_with("/") {
                    Some(entry[0..entry.len() - 1].to_string())
                } else {
                    None
                }
            })
            .collect())
    }

    fn list_package_versions(&self, name: &str) -> Result<Vec<api::Version>> {
        Ok(get_all_filenames(format!("/spfs/spk/pkg/{}", name))?
            .into_iter()
            .filter_map(|entry| {
                if entry.ends_with("/") {
                    Some(entry[0..entry.len() - 1].to_string())
                } else {
                    None
                }
            })
            .filter_map(|candidate| match api::parse_version(&candidate) {
                Ok(v) => Some(v),
                Err(err) => {
                    tracing::debug!(
                        "Skipping invalid version in /spfs/spk: [{}], {:?}",
                        candidate,
                        err
                    );
                    None
                }
            })
            .collect())
    }

    fn list_package_builds(&self, pkg: &api::Ident) -> Result<Vec<api::Ident>> {
        let mut base = PathBuf::from("/spfs/spk/pkg");
        base.push(pkg.name());
        base.push(pkg.version.to_string());
        Ok(get_all_filenames(&base)?
            .into_iter()
            .filter_map(|entry| {
                if entry.ends_with("/") {
                    Some(entry[0..entry.len() - 1].to_string())
                } else {
                    None
                }
            })
            .filter(|entry| base.join(entry).join("spec.yaml").exists())
            .filter_map(|candidate| match api::parse_build(&candidate) {
                Ok(b) => Some(pkg.with_build(Some(b))),
                Err(err) => {
                    tracing::debug!(
                        "Skipping invalid build in /spfs/spk: [{}] {:?}",
                        candidate,
                        err
                    );
                    None
                }
            })
            .collect())
    }

    fn read_spec(&self, pkg: &api::Ident) -> Result<api::Spec> {
        let mut path = PathBuf::from("/spfs/spk/pkg");
        path.push(pkg.to_string());
        path.push("spec.yaml");

        match api::read_spec_file(&path) {
            Err(Error::IO(err)) => {
                if err.kind() == std::io::ErrorKind::NotFound {
                    Err(Error::PackageNotFoundError(pkg.clone()))
                } else {
                    Err(err.into())
                }
            }
            err => err,
        }
    }

    fn get_package(&self, pkg: &api::Ident) -> Result<spfs::encoding::Digest> {
        let mut path = relative_path::RelativePathBuf::from("/spk/pkg");
        path.push(pkg.to_string());
        path.push("spec.yaml");

        match find_layer_by_filename(&path) {
            Err(Error::SPFS(spfs::Error::UnknownReference(_))) => {
                Err(Error::PackageNotFoundError(pkg.clone()))
            }
            res => res,
        }
    }

    fn force_publish_spec(&mut self, _spec: api::Spec) -> Result<()> {
        Err(Error::String("Cannot modify a runtime repository".into()))
    }

    fn publish_spec(&mut self, _spec: api::Spec) -> Result<()> {
        Err(Error::String(
            "Cannot publish to a runtime repository".into(),
        ))
    }

    fn remove_spec(&mut self, _pkg: &api::Ident) -> Result<()> {
        Err(Error::String("Cannot modify a runtime repository".into()))
    }

    fn publish_package(&mut self, _spec: api::Spec, _digest: spfs::encoding::Digest) -> Result<()> {
        Err(Error::String(
            "Cannot publish to a runtime repository".into(),
        ))
    }

    fn remove_package(&mut self, _pkg: &api::Ident) -> Result<()> {
        Err(Error::String("Cannot modify a runtime repository".into()))
    }
}

/// Works like ls_tags, returning strings that end with '/' for directories
/// and not for regular files
fn get_all_filenames<P: AsRef<std::path::Path>>(path: P) -> Result<Vec<String>> {
    let entries = match std::fs::read_dir(path) {
        Err(err) => {
            return match err.kind() {
                std::io::ErrorKind::NotFound => Ok(Default::default()),
                _ => Err(err.into()),
            }
        }
        Ok(e) => e.collect::<std::io::Result<Vec<_>>>(),
    };
    Ok(entries?
        .into_iter()
        .map(|entry| {
            let mut name = entry.file_name().to_string_lossy().to_string();
            match entry.file_type() {
                Ok(t) if t.is_dir() => name.push('/'),
                _ => (),
            }
            name
        })
        .collect())
}

fn find_layer_by_filename<S: AsRef<str>>(path: S) -> Result<spfs::encoding::Digest> {
    let runtime = spfs::active_runtime()?;
    let repo = spfs::load_config()?.get_repository()?.into();

    let stack = runtime.get_stack();
    let layers = spfs::resolve_stack_to_layers(stack.iter(), Some(&repo))?;
    for layer in layers.iter().rev() {
        let manifest = repo.read_manifest(&layer.manifest)?.unlock();
        if let Some(_) = manifest.get_path(&path) {
            return Ok(layer.digest()?.into());
        }
    }
    Err(spfs::graph::UnknownReferenceError::new(path).into())
}
