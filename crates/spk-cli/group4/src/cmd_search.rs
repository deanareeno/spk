// Copyright (c) Contributors to the SPK project.
// SPDX-License-Identifier: Apache-2.0
// https://github.com/spkenv/spk

use clap::Args;
use colored::Colorize;
use miette::Result;
use spk_cli_common::{flags, CommandArgs, Run};
use spk_schema::foundation::format::FormatIdent;
use spk_schema::{Deprecate, VersionIdent};

/// Search for packages by name/substring
#[derive(Args)]
pub struct Search {
    #[clap(flatten)]
    pub repos: flags::Repositories,

    #[clap(short, long, global = true, action = clap::ArgAction::Count)]
    pub verbose: u8,

    /// Show deprecated packages in the output
    #[clap(long, short)]
    deprecated: bool,

    /// The text/substring to search for in package names
    term: String,
}

#[async_trait::async_trait]
impl Run for Search {
    type Output = i32;

    async fn run(&mut self) -> Result<Self::Output> {
        let repos = self.repos.get_repos_for_non_destructive_operation().await?;

        let width = repos
            .iter()
            .map(|(n, _)| n)
            .map(String::len)
            .max()
            .unwrap_or_default();
        let mut exit = 1;
        for (repo_name, repo) in repos.iter() {
            for name in repo.list_packages().await? {
                if !name.as_str().contains(&self.term) {
                    continue;
                }
                let versions = repo.list_package_versions(&name).await?;
                let mut ident = VersionIdent::new_zero(name);
                for v in versions.iter() {
                    ident.set_version((**v).clone());

                    let builds = repo.list_package_builds(&ident).await?;
                    if builds.is_empty() {
                        // A version with no builds is treated as if
                        // it does not really exist. This can happen
                        // when a previously published package is
                        // deleted by 'spk rm'.
                        continue;
                    }

                    // Check recipe exists and for deprecation
                    let mut deprecation_status = "".black();
                    match repo.read_recipe(&ident).await {
                        Ok(recipe) => {
                            if recipe.is_deprecated() {
                                if self.deprecated {
                                    deprecation_status = " DEPRECATED".red();
                                } else {
                                    // Hide the deprecated ones
                                    continue;
                                }
                            }
                        }
                        Err(_) => {
                            // It doesn't have a recipe, but it does
                            // have builds, so unless all the builds
                            // are deprecated, show it. This can
                            // happen when there is a version of a
                            // package that only exists as embedded builds.
                            let mut all_builds_deprecated = true;
                            for build in builds {
                                if let Ok(spec) = repo.read_package(&build).await {
                                    if !spec.is_deprecated() {
                                        all_builds_deprecated = false;
                                        break;
                                    }
                                }
                            }
                            if all_builds_deprecated {
                                if self.deprecated {
                                    deprecation_status = " DEPRECATED".red();
                                } else {
                                    continue;
                                }
                            }
                        }
                    };

                    exit = 0;
                    println!(
                        "{repo_name: <width$} {}{deprecation_status}",
                        ident.format_ident()
                    );
                }
            }
        }
        Ok(exit)
    }
}

impl CommandArgs for Search {
    fn get_positional_args(&self) -> Vec<String> {
        // The important positional arg for a search is the search term
        vec![self.term.clone()]
    }
}
