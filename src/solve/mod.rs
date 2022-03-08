// Copyright (c) 2021 Sony Pictures Imageworks, et al.
// SPDX-License-Identifier: Apache-2.0
// https://github.com/imageworks/spk
mod errors;
pub mod graph;
mod package_iterator;
mod python;
mod solution;
mod solver;
mod validation;

pub(crate) use errors::SolverError; // python integration only
pub use errors::{Error, OutOfOptions};
pub use graph::Graph;
pub use python::init_module;
pub use solution::{PackageSource, Solution};
pub use solver::{Solver, SolverRuntime};