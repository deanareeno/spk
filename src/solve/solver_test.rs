// Copyright (c) 2021 Sony Pictures Imageworks, et al.
// SPDX-License-Identifier: Apache-2.0
// https://github.com/imageworks/spk
use std::{
    collections::HashMap,
    convert::TryFrom,
    sync::{Arc, Mutex},
};

use rstest::{fixture, rstest};
use spfs::encoding::{Digest, EMPTY_DIGEST};

use super::{RequestEnum, Solver};
use crate::{api, io, option_map, solve, spec, storage, Error};

#[fixture]
fn solver() -> Solver {
    Solver::new()
}

macro_rules! make_repo {
    ( [ $( $spec:tt ),+ $(,)? ] ) => {{
        make_repo!([ $( $spec ),* ], options={})
    }};
    ( [ $( $spec:tt ),+ $(,)? ], options={ $($k:expr => $v:expr),* } ) => {{
        let options = crate::option_map!{$($k:expr => $v:expr),*};
        make_repo!([ $( $spec ),* ], options=options)
    }};
    ( [ $( $spec:tt ),+ $(,)? ], options=$options:expr ) => {{
        let mut repo = crate::storage::RepositoryHandle::new_mem();
        $(
            let (s, cmpts) = make_package!(repo, $spec, &$options);
            repo.publish_package(s, cmpts).unwrap();
        )*
        repo
    }};
}

#[macro_export(local_inner_macros)]
macro_rules! make_package {
    ($repo:ident, ($build_spec:expr, $components:expr), $opts:expr) => {{
        ($build_spec, $components)
    }};
    ($repo:ident, $build_spec:ident, $opts:expr) => {{
        let s = $build_spec;
        let cmpts: std::collections::HashMap<_, spfs::encoding::Digest> = s
            .install
            .components
            .iter()
            .map(|c| (c.name.clone(), spfs::encoding::EMPTY_DIGEST.into()))
            .collect();
        (s, cmpts)
    }};
    ($repo:ident, $spec:tt, $opts:expr) => {{
        let json = serde_json::json!($spec);
        let mut spec: crate::api::Spec = serde_json::from_value(json).expect("Invalid spec json");
        let build = spec.clone();
        spec.pkg.set_build(None);
        $repo.force_publish_spec(spec).unwrap();
        make_build_and_components!(build, [], $opts, [])
    }};
}

#[macro_export(local_inner_macros)]
macro_rules! make_build {
    ($spec:tt) => {
        make_build!($spec, [], {})
    };
    ($spec:tt, $deps:tt, $opts:tt) => {{
        let (spec, _) = make_build_and_components!($spec, deps, opts);
        spec
    }};
}

#[macro_export(local_inner_macros)]
macro_rules! make_build_and_components {
    ($spec:tt) => {
        make_build_and_components!($spec, [])
    };
    ($spec:tt, [$($dep:expr),*]) => {
        make_build_and_components!($spec, [$($dep),*], {})
    };
    ($spec:tt, [$($dep:expr),*], $opts:expr) => {
        make_build_and_components!($spec, [$($dep),*], $opts, [])
    };
    ($spec:tt, [$($dep:expr),*], $opts:expr, [$($component:expr),*]) => {{
        let mut spec = crate::spec!($spec);
        let mut components = std::collections::HashMap::new();
        let deps: Vec<api::Spec> = std::vec![$($dep),*];
        if spec.pkg.is_source() {
            components.insert(crate::api::Component::Source, spfs::encoding::EMPTY_DIGEST.into());
            (spec, components)
        } else {
            let mut build_opts = $opts.clone();
            let mut resolved_opts = spec.resolve_all_options(&build_opts).into_iter();
            build_opts.extend(&mut resolved_opts);
            spec.update_for_build(&build_opts, deps)
                .expect("Failed to render build spec");
            let mut names = std::vec![$($component),*];
            if names.is_empty() {
                names = spec.install.components.iter().map(|c| c.name.clone()).collect();
            }
            for name in names {
                components.insert(name, spfs::encoding::EMPTY_DIGEST.into());
            }
            (spec, components)
        }
    }}
}

macro_rules! request {
    ($req:literal) => {
        crate::api::Request::Pkg(crate::api::PkgRequest::new(
            crate::api::parse_ident_range($req).unwrap(),
        ))
    };
    ($req:tt) => {{
        let value = json!($req);
        let req: crate::api::Request = serde_json::from_value(value).unwrap();
        req
    }};
}

#[rstest]
fn test_solver_no_requests(mut solver: Solver) {
    solver.solve().unwrap();
}

#[rstest]
fn test_solver_package_with_no_spec(mut solver: Solver) {
    let mut repo = crate::storage::RepositoryHandle::new_mem();

    let options = option_map! {};
    let mut spec = spec!({"pkg": "my-pkg/1.0.0"});
    spec.pkg
        .set_build(Some(api::Build::Digest(options.digest())));

    // publish package without publishing spec
    let components = vec![(api::Component::Run, EMPTY_DIGEST.into())]
        .into_iter()
        .collect();
    repo.publish_package(spec, components).unwrap();

    solver.update_options(options);
    solver.add_repository(Arc::new(Mutex::new(repo)));
    solver.add_request(request!("my-pkg"));

    let res = io::run_and_print_resolve(&solver, 100);
    assert!(matches!(res, Err(Error::PackageNotFoundError(_))));
}

#[rstest]
fn test_solver_single_package_no_deps(mut solver: Solver) {
    let options = option_map! {};
    let repo = make_repo!([{"pkg": "my-pkg/1.0.0"}], options=options);

    solver.update_options(options);
    solver.add_repository(Arc::new(Mutex::new(repo)));
    solver.add_request(request!("my-pkg"));

    let packages = io::run_and_print_resolve(&solver, 100).unwrap();
    assert_eq!(packages.len(), 1, "expected one resolved package");
    let resolved = packages.get("my-pkg").unwrap();
    assert_eq!(&resolved.spec.pkg.version.to_string(), "1.0.0");
    assert!(resolved.spec.pkg.build.is_some());
    assert_ne!(resolved.spec.pkg.build, Some(api::Build::Source));
}

#[rstest]
fn test_solver_single_package_simple_deps(mut solver: Solver) {
    let options = option_map! {};
    let repo = make_repo!(
        [
            {"pkg": "pkg-a/0.9.0"},
            {"pkg": "pkg-a/1.0.0"},
            {"pkg": "pkg-a/1.2.0"},
            {"pkg": "pkg-a/1.2.1"},
            {"pkg": "pkg-a/2.0.0"},
            {"pkg": "pkg-b/1.0.0", "install": {"requirements": [{"pkg": "pkg-a/2.0"}]}},
            {"pkg": "pkg-b/1.1.0", "install": {"requirements": [{"pkg": "pkg-a/1.2"}]}},
        ]
    );

    solver.update_options(options);
    solver.add_repository(Arc::new(Mutex::new(repo)));
    solver.add_request(request!("pkg-b/1.1"));

    let packages = io::run_and_print_resolve(&solver, 100).unwrap();
    assert_eq!(packages.len(), 2, "expected two resolved packages");
    assert_eq!(
        &packages.get("pkg-a").unwrap().spec.pkg.version.to_string(),
        "1.2.1"
    );
    assert_eq!(
        &packages.get("pkg-b").unwrap().spec.pkg.version.to_string(),
        "1.1.0"
    );
}

#[rstest]
fn test_solver_dependency_abi_compat(mut solver: Solver) {
    let options = option_map! {};
    let repo = make_repo!(
        [
            {
                "pkg": "pkg-b/1.1.0",
                "install": {"requirements": [{"pkg": "pkg-a/1.1.0"}]},
            },
            {"pkg": "pkg-a/2.1.1", "compat": "x.a.b"},
            {"pkg": "pkg-a/1.2.1", "compat": "x.a.b"},
            {"pkg": "pkg-a/1.1.1", "compat": "x.a.b"},
            {"pkg": "pkg-a/1.1.0", "compat": "x.a.b"},
            {"pkg": "pkg-a/1.0.0", "compat": "x.a.b"},
            {"pkg": "pkg-a/0.9.0", "compat": "x.a.b"},
        ]
    );

    solver.update_options(options);
    solver.add_repository(Arc::new(Mutex::new(repo)));
    solver.add_request(request!("pkg-b/1.1"));

    let packages = io::run_and_print_resolve(&solver, 100).unwrap();
    assert_eq!(packages.len(), 2, "expected two resolved packages");
    assert_eq!(
        &packages.get("pkg-a").unwrap().spec.pkg.version.to_string(),
        "1.1.1"
    );
    assert_eq!(
        &packages.get("pkg-b").unwrap().spec.pkg.version.to_string(),
        "1.1.0"
    );
}

#[rstest]
fn test_solver_dependency_incompatible(mut solver: Solver) {
    // test what happens when a dependency is added which is incompatible
    // with an existing request in the stack
    let repo = make_repo!(
        [
            {"pkg": "maya/2019.0.0"},
            {"pkg": "maya/2020.0.0"},
            {
                "pkg": "my-plugin/1.0.0",
                "install": {"requirements": [{"pkg": "maya/2020"}]},
            },
        ]
    );

    solver.add_repository(Arc::new(Mutex::new(repo)));
    solver.add_request(request!("my-plugin/1"));
    // this one is incompatible with requirements of my-plugin but the solver doesn't know it yet
    solver.add_request(request!("maya/2019"));

    let res = io::run_and_print_resolve(&solver, 100);
    assert!(matches!(res, Err(Error::Solve(_))));
}

#[rstest]
fn test_solver_dependency_incompatible_stepback(mut solver: Solver) {
    // test what happens when a dependency is added which is incompatible
    // with an existing request in the stack - in this case we want the solver
    // to successfully step back into an older package version with
    // better dependencies
    let repo = make_repo!(
        [
            {"pkg": "maya/2019"},
            {"pkg": "maya/2020"},
            {
                "pkg": "my-plugin/1.1.0",
                "install": {"requirements": [{"pkg": "maya/2020"}]},
            },
            {
                "pkg": "my-plugin/1.0.0",
                "install": {"requirements": [{"pkg": "maya/2019"}]},
            },
        ]
    );

    solver.add_repository(Arc::new(Mutex::new(repo)));
    solver.add_request(request!("my-plugin/1"));
    // this one is incompatible with requirements of my-plugin/1.1.0 but not my-plugin/1.0
    solver.add_request(request!("maya/2019"));

    let packages = io::run_and_print_resolve(&solver, 100).unwrap();
    assert_eq!(
        &packages
            .get("my-plugin")
            .unwrap()
            .spec
            .pkg
            .version
            .to_string(),
        "1.0.0"
    );
    assert_eq!(
        &packages.get("maya").unwrap().spec.pkg.version.to_string(),
        "2019.0.0"
    );
}

#[rstest]
fn test_solver_dependency_already_satisfied(mut solver: Solver) {
    // test what happens when a dependency is added which represents
    // a package which has already been resolved
    // - and the resolved version satisfies the request

    let repo = make_repo!(
        [
            {
                "pkg": "pkg-top/1.0.0",
                // should resolve dep_1 as 1.0.0
                "install": {
                    "requirements": [{"pkg": "dep-1/~1.0.0"}, {"pkg": "dep-2/1"}]
                },
            },
            {"pkg": "dep-1/1.1.0"},
            {"pkg": "dep-1/1.0.0"},
            // when dep_2 gets resolved, it will re-request this but it has already resolved
            {"pkg": "dep-2/1.0.0", "install": {"requirements": [{"pkg": "dep-1/1"}]}},
        ]
    );
    solver.add_repository(Arc::new(Mutex::new(repo)));
    solver.add_request(request!("pkg-top"));
    let packages = io::run_and_print_resolve(&solver, 100).unwrap();

    let mut names: Vec<_> = packages
        .items()
        .into_iter()
        .map(|s| s.spec.pkg.name().to_owned())
        .collect();
    names.sort();
    assert_eq!(
        names,
        vec![
            "pkg-top".to_string(),
            "dep-1".to_string(),
            "dep-2".to_string(),
        ]
    );
    assert_eq!(
        &packages.get("dep-1").unwrap().spec.pkg.version.to_string(),
        "1.0.0"
    );
}

#[rstest]
fn test_solver_dependency_reopen_solvable(mut solver: Solver) {
    // test what happens when a dependency is added which represents
    // a package which has already been resolved
    // - and the resolved version does not satisfy the request
    //   - and a version exists for both (solvable)

    let repo = make_repo!(
        [
            {
                "pkg": "my-plugin/1.0.0",
                // should resolve maya as 2019.2 (favoring latest)
                "install": {
                    "requirements": [{"pkg": "maya/2019"}, {"pkg": "some-library/1"}]
                },
            },
            {"pkg": "maya/2019.2.0"},
            {"pkg": "maya/2019.0.0"},
            // when some-library gets resolved, it will enforce an older version
            // of the existing resolve, which is still valid for all requests
            {
                "pkg": "some-library/1.0.0",
                "install": {"requirements": [{"pkg": "maya/~2019.0.0"}]},
            },
        ]
    );
    solver.add_repository(Arc::new(Mutex::new(repo)));
    solver.add_request(request!("my-plugin"));
    let packages = io::run_and_print_resolve(&solver, 100).unwrap();
    let mut names: Vec<_> = packages
        .items()
        .into_iter()
        .map(|s| s.spec.pkg.name().to_owned())
        .collect();
    names.sort();
    assert_eq!(
        names,
        vec![
            "my-plugin".to_string(),
            "some-library".to_string(),
            "maya".to_string(),
        ]
    );
    assert_eq!(
        &packages.get("maya").unwrap().spec.pkg.version.to_string(),
        "2019.0.0"
    );
}

#[rstest]
fn test_solver_dependency_reiterate(mut solver: Solver) {
    // test what happens when a package iterator must be run through twice
    // - walking back up the solve graph should reset the iterator to where it was

    let repo = make_repo!(
        [
            {
                "pkg": "my-plugin/1.0.0",
                "install": {"requirements": [{"pkg": "some-library/1"}]},
            },
            {"pkg": "maya/2019.2.0"},
            {"pkg": "maya/2019.0.0"},
            // asking for a maya version that doesn't exist will run out the iterator
            {
                "pkg": "some-library/1.0.0",
                "install": {"requirements": [{"pkg": "maya/~2018.0.0"}]},
            },
            // the second attempt at some-library will find maya 2019 properly
            {
                "pkg": "some-library/1.0.0",
                "install": {"requirements": [{"pkg": "maya/~2019.0.0"}]},
            },
        ]
    );
    solver.add_repository(Arc::new(Mutex::new(repo)));
    solver.add_request(request!("my-plugin"));
    let packages = io::run_and_print_resolve(&solver, 100).unwrap();
    let mut names: Vec<_> = packages
        .items()
        .into_iter()
        .map(|s| s.spec.pkg.name().to_owned())
        .collect();
    assert_eq!(
        names,
        vec![
            "my-plugin".to_string(),
            "some-library".to_string(),
            "maya".to_string(),
        ]
    );
    assert_eq!(
        &packages.get("maya").unwrap().spec.pkg.version.to_string(),
        "2019.0.0"
    );
}

#[rstest]
fn test_solver_dependency_reopen_unsolvable(mut solver: Solver) {
    // // test what happens when a dependency is added which represents
    // // a package which has already been resolved
    // // - and the resolved version does not satisfy the request
    // //   - and a version does not exist for both (unsolvable)

    // let repo = make_repo!(
    //     [
    //         {
    //             "pkg": "pkg-top/1.0.0",
    //             # must resolve dep_1 as 1.1.0 (favoring latest)
    //             "install": {"requirements": [{"pkg": "dep-1/1.1"}, {"pkg": "dep-2/1"}]},
    //         },
    //         {"pkg": "dep-1/1.1.0"},
    //         {"pkg": "dep-1/1.0.0"},
    //         # when dep_2 gets resolved, it will enforce an older version
    //         # of the existing resolve, which is in conflict with the original
    //         {
    //             "pkg": "dep-2/1.0.0",
    //             "install": {"requirements": [{"pkg": "dep-1/~1.0.0"}]},
    //         },
    //     ]
    // )
    // solver.add_repository(Arc::new(Mutex::new(repo)));
    // solver.add_request(request!("pkg-top"));
    // with pytest.raises(solve.SolverError):
    //     packages = solver.solve()
    //     print(packages)
    todo!()
}

#[rstest]
fn test_solver_pre_release_config(mut solver: Solver) {
    // let repo = make_repo!(
    //     [
    //         {"pkg": "my-pkg/0.9.0"},
    //         {"pkg": "my-pkg/1.0.0-pre.0"},
    //         {"pkg": "my-pkg/1.0.0-pre.1"},
    //         {"pkg": "my-pkg/1.0.0-pre.2"},
    //     ]
    // )

    // solver.add_repository(Arc::new(Mutex::new(repo)));
    // solver.add_request(request!("my-pkg"));

    // solution = solver.solve()
    // assert (
    //     solution.get("my-pkg").spec.pkg.version == "0.9.0"
    // ), "should not resolve pre-release by default"

    // solver.reset()
    // solver.add_repository(Arc::new(Mutex::new(repo)));
    // solver.add_request(
    //     api.request_from_dict({"pkg": "my-pkg", "prereleasePolicy": "IncludeAll"})
    // )

    // solution = solver.solve()
    // assert solution.get("my-pkg").spec.pkg.version == "1.0.0-pre.2"
    todo!()
}

#[rstest]
fn test_solver_constraint_only(mut solver: Solver) {
    // // test what happens when a dependency is marked as a constraint/optional
    // // and no other request is added
    // // - the constraint is noted
    // // - the package does not get resolved into the final env

    // let repo = make_repo!(
    //     [
    //         {
    //             "pkg": "vnp3/2.0.0",
    //             "install": {
    //                 "requirements": [
    //                     {"pkg": "python/3.7", "include": "IfAlreadyPresent"}
    //                 ]
    //             },
    //         }
    //     ]
    // )
    // solver.add_repository(Arc::new(Mutex::new(repo)));
    // solver.add_request(request!("vnp3"));
    // solution = io::run_and_print_resolve(&solver, 100);

    // with pytest.raises(KeyError):
    //     solution.get("python")
    todo!()
}

#[rstest]
fn test_solver_constraint_and_request(mut solver: Solver) {
    // // test what happens when a dependency is marked as a constraint/optional
    // // and also requested by another package
    // // - the constraint is noted
    // // - the constraint is merged with the request

    // let repo = make_repo!(
    //     [
    //         {
    //             "pkg": "vnp3/2.0.0",
    //             "install": {
    //                 "requirements": [
    //                     {"pkg": "python/=3.7.3", "include": "IfAlreadyPresent"}
    //                 ]
    //             },
    //         },
    //         {
    //             "pkg": "my-tool/1.2.0",
    //             "install": {"requirements": [{"pkg": "vnp3"}, {"pkg": "python/3.7"}]},
    //         },
    //         {"pkg": "python/3.7.3"},
    //         {"pkg": "python/3.8.1"},
    //     ]
    // )
    // solver.add_repository(Arc::new(Mutex::new(repo)));
    // solver.add_request(request!("my-tool"));
    // solution = io::run_and_print_resolve(&solver, 100);

    // assert solution.get("python").spec.pkg.version == "3.7.3"
    todo!()
}

#[rstest]
fn test_solver_option_compatibility() {
    // // test what happens when an option is given in the solver
    // // - the options for each build are checked
    // // - the resolved build must have used the option

    // solver = Solver()
    // spec = api.Spec.from_dict(
    //     {
    //         "pkg": "vnp3/2.0.0",
    //         "build": {
    //             # favoritize 2.7, otherwise an option of python=2 doesn't actually
    //             # exclude python 3 from being resolved
    //             "options": [{"pkg": "python/~2.7"}],
    //             "variants": [{"python": "3.7"}, {"python": "2.7"}],
    //         },
    //     }
    // )
    // print(
    //     make_build(spec.to_dict(), [make_build({"pkg": "python/2.7.5"})])
    //     .build.options[0]
    //     .get_value()
    // )
    // let repo = make_repo!(
    //     [
    //         make_build(spec.to_dict(), [make_build({"pkg": "python/2.7.5"})]),
    //         make_build(spec.to_dict(), [make_build({"pkg": "python/3.7.3"})]),
    //     ]
    // )
    // repo.publish_spec(spec)

    // for pyver in ("2", "2.7", "2.7.5", "3", "3.7", "3.7.3"):
    //     solver.reset()
    //     solver.add_request(api.VarRequest("python", pyver))
    //     solver.add_repository(repo)
    //     solver.add_request("vnp3")
    //     solution = io::run_and_print_resolve(&solver, 100);

    //     resolved = solution.get("vnp3")
    //     opt = resolved.spec.build.options[0]
    //     value = opt.get_value()
    //     assert value is not None
    //     assert value.startswith(f"~{pyver}"), f"{value} should start with ~{pyver}"
    todo!()
}

#[rstest]
fn test_solver_option_injection() {
    // // test the options that are defined when a package is resolved
    // // - options are namespaced and added to the environment

    // spec = api.Spec.from_dict(
    //     {
    //         "pkg": "vnp3/2.0.0",
    //         "build": {
    //             "options": [
    //                 {"pkg": "python"},
    //                 {"var": "python.abi/cp27mu"},
    //                 {"var": "debug/on"},
    //                 {"var": "special"},
    //             ],
    //         },
    //     }
    // )
    // pybuild = make_build(
    //     {
    //         "pkg": "python/2.7.5",
    //         "build": {"options": [{"var": "abi/cp27mu"}]},
    //     }
    // )
    // let repo = make_repo!([make_build(spec.to_dict(), [pybuild])])
    // repo.publish_spec(spec)

    // solver = Solver()
    // solver.add_repository(Arc::new(Mutex::new(repo)));
    // solver.add_request(request!("vnp3"));
    // solution = io::run_and_print_resolve(&solver, 100);

    // opts = solution.options()
    // assert opts["vnp3"] == "~2.0.0"
    // assert opts["vnp3.python"] == "~2.7.5"
    // assert opts["vnp3.debug"] == "on"
    // assert opts["python.abi"] == "cp27mu"
    // assert "vnp3.special" not in opts, "should not define empty values"
    // assert len(opts) == 4, "expected no more options"
    todo!()
}

#[rstest]
fn test_solver_build_from_source() {
    // // test when no appropriate build exists but the source is available
    // // - the build is skipped
    // // - the source package is checked for current options
    // // - a new build is created
    // // - the local package is used in the resolve

    // let repo = make_repo!(
    //     [
    //         {
    //             "pkg": "my-tool/1.2.0/src",
    //             "build": {"options": [{"var": "debug"}], "script": "echo BUILD"},
    //         },
    //         {
    //             "pkg": "my-tool/1.2.0",
    //             "build": {"options": [{"var": "debug"}], "script": "echo BUILD"},
    //         },
    //     ],
    //     api.OptionMap(debug="off"),
    // )

    // solver = Solver()
    // solver.add_repository(Arc::new(Mutex::new(repo)));
    // // the new option value should disqulify the existing build
    // // but a new one should be generated for this set of options
    // solver.add_request(api.VarRequest("debug", "on"))
    // solver.add_request(request!("my-tool"));

    // solution = io::run_and_print_resolve(&solver, 100);

    // resolved = solution.get("my-tool")
    // assert (
    //     resolved.is_source_build()
    // ), f"Should set unbuilt spec as source: {resolved.spec.pkg}"

    // solver.reset()
    // solver.add_repository(Arc::new(Mutex::new(repo)));
    // solver.add_request(api.VarRequest("debug", "on"))
    // solver.add_request(request!("my-tool"));
    // solver.set_binary_only(True)
    // with pytest.raises(solve.SolverError):
    //     # Should fail when binary-only is specified
    //     io::run_and_print_resolve(&solver, 100);
    todo!()
}

#[rstest]
fn test_solver_build_from_source_unsolvable(mut solver: Solver) {
    // // test when no appropriate build exists but the source is available
    // // - if the requested pkg cannot resolve a build environment
    // // - this is flagged by the solver as impossible

    // gcc48 = make_build({"pkg": "gcc/4.8"})
    // let repo = make_repo!(
    //     [
    //         gcc48,
    //         make_build(
    //             {
    //                 "pkg": "my-tool/1.2.0",
    //                 "build": {"options": [{"pkg": "gcc"}], "script": "echo BUILD"},
    //             },
    //             [gcc48],
    //         ),
    //         {
    //             "pkg": "my-tool/1.2.0/src",
    //             "build": {"options": [{"pkg": "gcc"}], "script": "echo BUILD"},
    //         },
    //     ],
    //     api.OptionMap(gcc="4.8"),
    // )

    // solver.add_repository(Arc::new(Mutex::new(repo)));
    // // the new option value should disqualify the existing build
    // // and there is no 6.3 that can be resolved for this request
    // solver.add_request(api.VarRequest("gcc", "6.3"))
    // solver.add_request(request!("my-tool"));

    // with pytest.raises(solve.SolverError):
    //     io::run_and_print_resolve(&solver, 100);
    todo!()
}

#[rstest]
fn test_solver_build_from_source_dependency() {
    // // test when no appropriate build exists but the source is available
    // // - the existing build is skipped
    // // - the source package is checked for current options
    // // - a new build is created of the dependent
    // // - the local package is used in the resolve

    // python36 = make_build({"pkg": "python/3.6.3", "compat": "x.a.b"})
    // build_with_py36 = make_build(
    //     {
    //         "pkg": "my-tool/1.2.0",
    //         "build": {"options": [{"pkg": "python"}]},
    //         "install": {"requirements": [{"pkg": "python/3.6.3"}]},
    //     },
    //     [python36],
    // )

    // let repo = make_repo!(
    //     [
    //         # the source package pins the build environment package
    //         {
    //             "pkg": "my-tool/1.2.0/src",
    //             "build": {"options": [{"pkg": "python"}]},
    //             "install": {
    //                 "requirements": [{"pkg": "python", "fromBuildEnv": "x.x.x"}]
    //             },
    //         },
    //         # one existing build exists that used python 3.6.3
    //         build_with_py36,
    //         # only python 3.7 exists, which is api compatible, but not abi
    //         {"pkg": "python/3.7.3", "compat": "x.a.b"},
    //     ],
    // )

    // solver = Solver()
    // // the new option value should disqulify the existing build
    // // but a new one should be generated for this set of options
    // solver.update_options(api.OptionMap(debug="on"));
    // solver.add_repository(Arc::new(Mutex::new(repo)));
    // solver.add_request(request!("my-tool"));

    // solution = io::run_and_print_resolve(&solver, 100);

    // assert solution.get("my-tool").is_source_build(), "should want to build"
    todo!()
}

#[rstest]
fn test_solver_deprecated_build(mut solver: Solver) {
    // specs = [{"pkg": "my-pkg/0.9.0"}, {"pkg": "my-pkg/1.0.0"}]
    // deprecated = make_build({"pkg": "my-pkg/1.0.0", "deprecated": True})
    // let repo = make_repo!([*specs, deprecated])

    // solver.add_repository(Arc::new(Mutex::new(repo)));
    // solver.add_request(request!("my-pkg"));

    // solution = io::run_and_print_resolve(&solver, 100);
    // assert (
    //     solution.get("my-pkg").spec.pkg.version == "0.9.0"
    // ), "should not resolve deprecated build by default"

    // solver.reset()
    // solver.add_repository(Arc::new(Mutex::new(repo)));
    // solver.add_request(api.request_from_dict({"pkg": str(deprecated.pkg)}))

    // solution = io::run_and_print_resolve(&solver, 100);
    // assert (
    //     solution.get("my-pkg").spec.pkg.version == "1.0.0"
    // ), "should be able to resolve exact deprecated build"
    todo!()
}

#[rstest]
fn test_solver_deprecated_version(mut solver: Solver) {
    // specs = [{"pkg": "my-pkg/0.9.0"}, {"pkg": "my-pkg/1.0.0", "deprecated": True}]
    // deprecated = make_build({"pkg": "my-pkg/1.0.0"})
    // deprecated.deprecated = True
    // let repo = make_repo!(specs + [deprecated])  # type: ignore

    // solver.add_repository(Arc::new(Mutex::new(repo)));
    // solver.add_request(request!("my-pkg"));

    // solution = io::run_and_print_resolve(&solver, 100);
    // assert (
    //     solution.get("my-pkg").spec.pkg.version == "0.9.0"
    // ), "should not resolve build when version is deprecated by default"

    // solver.reset()
    // solver.add_repository(Arc::new(Mutex::new(repo)));
    // solver.add_request(api.request_from_dict({"pkg": str(deprecated.pkg)}))

    // solution = io::run_and_print_resolve(&solver, 100);
    // assert (
    //     solution.get("my-pkg").spec.pkg.version == "1.0.0"
    // ), "should be able to resolve exact build when version is deprecated"
    todo!()
}

#[rstest]
fn test_solver_build_from_source_deprecated(mut solver: Solver) {
    // // test when no appropriate build exists and the main package
    // // has been deprecated, no source build should be allowed

    // let repo = make_repo!(
    //     [
    //         {
    //             "pkg": "my-tool/1.2.0/src",
    //             "build": {"options": [{"var": "debug"}], "script": "echo BUILD"},
    //         },
    //         {
    //             "pkg": "my-tool/1.2.0",
    //             "build": {"options": [{"var": "debug"}], "script": "echo BUILD"},
    //         },
    //     ],
    //     api.OptionMap(debug="off"),
    // )
    // spec = repo.read_spec(api.parse_ident("my-tool/1.2.0"))
    // spec.deprecated = True
    // repo.force_publish_spec(spec)

    // solver.add_repository(Arc::new(Mutex::new(repo)));
    // solver.add_request(api.VarRequest("debug", "on"))
    // solver.add_request(request!("my-tool"));

    // with pytest.raises(solve.SolverError):
    //     io::run_and_print_resolve(&solver, 100);
    todo!()
}

#[rstest]
fn test_solver_embedded_package_adds_request(mut solver: Solver) {
    // // test when there is an embedded package
    // // - the embedded package is added to the solution
    // // - the embedded package is also added as a request in the resolve

    // let repo = make_repo!(
    //     [
    //         {
    //             "pkg": "maya/2019.2",
    //             "build": {"script": "echo BUILD"},
    //             "install": {"embedded": [{"pkg": "qt/5.12.6"}]},
    //         },
    //     ]
    // )

    // solver.add_repository(Arc::new(Mutex::new(repo)));
    // solver.add_request(request!("maya"));

    // solution = io::run_and_print_resolve(&solver, 100);

    // assert solution.get("qt").request.pkg.build == api.EMBEDDED
    // assert solution.get("qt").spec.pkg.version == "5.12.6"
    // assert solution.get("qt").spec.pkg.build == api.EMBEDDED
    todo!()
}

#[rstest]
fn test_solver_embedded_package_solvable(mut solver: Solver) {
    // // test when there is an embedded package
    // // - the embedded package is added to the solution
    // // - the embedded package resolves existing requests
    // // - the solution includes the embedded packages

    // let repo = make_repo!(
    //     [
    //         {
    //             "pkg": "maya/2019.2",
    //             "build": {"script": "echo BUILD"},
    //             "install": {"embedded": [{"pkg": "qt/5.12.6"}]},
    //         },
    //         {
    //             "pkg": "qt/5.13.0",
    //             "build": {"script": "echo BUILD"},
    //         },
    //     ]
    // )

    // solver.add_repository(Arc::new(Mutex::new(repo)));
    // solver.add_request(request!("qt"));
    // solver.add_request(request!("maya"));

    // solution = io::run_and_print_resolve(&solver, 100);

    // assert solution.get("qt").spec.pkg.version == "5.12.6"
    // assert solution.get("qt").spec.pkg.build == api.EMBEDDED
    todo!()
}

#[rstest]
fn test_solver_embedded_package_unsolvable(mut solver: Solver) {
    // // test when there is an embedded package
    // // - the embedded package is added to the solution
    // // - the embedded package conflicts with existing requests

    // let repo = make_repo!(
    //     [
    //         {
    //             "pkg": "my-plugin",
    //             # the qt/5.13 requirement is available but conflits with maya embedded
    //             "install": {"requirements": [{"pkg": "maya/2019"}, {"pkg": "qt/5.13"}]},
    //         },
    //         {
    //             "pkg": "maya/2019.2",
    //             "build": {"script": "echo BUILD"},
    //             "install": {"embedded": [{"pkg": "qt/5.12.6"}]},
    //         },
    //         {
    //             "pkg": "qt/5.13.0",
    //             "build": {"script": "echo BUILD"},
    //         },
    //     ]
    // )

    // solver.add_repository(Arc::new(Mutex::new(repo)));
    // solver.add_request(request!("my-plugin"));

    // with pytest.raises(solve.SolverError):
    //     io::run_and_print_resolve(&solver, 100);
    todo!()
}

#[rstest]
fn test_solver_some_versions_conflicting_requests(mut solver: Solver) {
    // // test when there is a package with some version that have a conflicting dependency
    // // - the solver passes over the one with conflicting
    // // - the solver logs compat info for versions with conflicts

    // let repo = make_repo!(
    //     [
    //         {
    //             "pkg": "my-lib",
    //             "install": {
    //                 # python 2.7 requirement will conflict with the first (2.1) build of dep
    //                 "requirements": [{"pkg": "python/=2.7.5"}, {"pkg": "dep/2"}]
    //             },
    //         },
    //         {
    //             "pkg": "dep/2.1.0",
    //             "install": {"requirements": [{"pkg": "python/=3.7.3"}]},
    //         },
    //         {
    //             "pkg": "dep/2.0.0",
    //             "install": {"requirements": [{"pkg": "python/=2.7.5"}]},
    //         },
    //         {"pkg": "python/2.7.5"},
    //         {"pkg": "python/3.7.3"},
    //     ]
    // )

    // solver.add_repository(Arc::new(Mutex::new(repo)));
    // solver.add_request(request!("my-lib"));

    // solution = io::run_and_print_resolve(&solver, 100);

    // assert solution.get("dep").spec.pkg.version == "2.0.0"
    todo!()
}

#[rstest]
fn test_solver_embedded_request_invalidates(mut solver: Solver) {
    // // test when a package is resolved with an incompatible embedded pkg
    // // - the solver tries to resolve the package
    // // - there is a conflict in the embedded request

    // let repo = make_repo!(
    //     [
    //         {
    //             "pkg": "my-lib",
    //             "install": {
    //                 # python 2.7 requirement will conflict with the maya embedded one
    //                 "requirements": [{"pkg": "python/3.7"}, {"pkg": "maya/2020"}]
    //             },
    //         },
    //         {
    //             "pkg": "maya/2020",
    //             "install": {"embedded": [{"pkg": "python/2.7.5"}]},
    //         },
    //         {"pkg": "python/2.7.5"},
    //         {"pkg": "python/3.7.3"},
    //     ]
    // )

    // solver.add_repository(Arc::new(Mutex::new(repo)));
    // solver.add_request(request!("python"));
    // solver.add_request(request!("my-lib"));

    // with pytest.raises(solve.SolverError):
    //     io::run_and_print_resolve(&solver, 100);
    todo!()
}

#[rstest]
fn test_solver_unknown_package_options(mut solver: Solver) {
    // // test when a package is requested with specific options (eg: pkg.opt)
    // // - the solver ignores versions that don't define the option
    // // - the solver resolves versions that do define the option

    // let repo = make_repo!([{"pkg": "my-lib/2.0.0"}])
    // solver.add_repository(Arc::new(Mutex::new(repo)));

    // // this option is specific to the my-lib package and is not known by the package
    // solver.add_request(api.VarRequest("my-lib.something", "value"))
    // solver.add_request(request!("my-lib"));

    // with pytest.raises(solve.SolverError):
    //     io::run_and_print_resolve(&solver, 100);

    // // this time we don't request that option, and it should be ok
    // solver.reset()
    // solver.add_repository(Arc::new(Mutex::new(repo)));
    // solver.add_request(request!("my-lib"));
    // io::run_and_print_resolve(&solver, 100);
    todo!()
}

#[rstest]
fn test_solver_var_requirements(mut solver: Solver) {
    // // test what happens when a dependency is added which is incompatible
    // // with an existing request in the stack
    // let repo = make_repo!(
    //     [
    //         {
    //             "pkg": "python/2.7.5",
    //             "build": {"options": [{"var": "abi", "static": "cp27mu"}]},
    //         },
    //         {
    //             "pkg": "python/3.7.3",
    //             "build": {"options": [{"var": "abi", "static": "cp37m"}]},
    //         },
    //         {
    //             "pkg": "my-app/1.0.0",
    //             "install": {
    //                 "requirements": [{"pkg": "python"}, {"var": "python.abi/cp27mu"}]
    //             },
    //         },
    //         {
    //             "pkg": "my-app/2.0.0",
    //             "install": {
    //                 "requirements": [{"pkg": "python"}, {"var": "python.abi/cp37m"}]
    //             },
    //         },
    //     ]
    // )

    // solver.add_repository(Arc::new(Mutex::new(repo)));
    // solver.add_request(request!("my-app/2"));

    // solution = io::run_and_print_resolve(&solver, 100);

    // assert solution.get("my-app").spec.pkg.version == "2.0.0"
    // assert solution.get("python").spec.pkg.version == "3.7.3"

    // // requesting the older version of my-app should force old python abi
    // solver.reset()
    // solver.add_repository(Arc::new(Mutex::new(repo)));
    // solver.add_request(request!("my-app/1"));

    // solution = io::run_and_print_resolve(&solver, 100);

    // assert solution.get("python").spec.pkg.version == "2.7.5"
    todo!()
}

#[rstest]
fn test_solver_var_requirements_unresolve(mut solver: Solver) {
    // // test when a package is resolved that conflicts in var requirements
    // //  - the solver should unresolve the solved package
    // //  - the solver should resolve a new version of the package with the right version
    // let repo = make_repo!(
    //     [
    //         {
    //             "pkg": "python/2.7.5",
    //             "build": {"options": [{"var": "abi", "static": "cp27"}]},
    //         },
    //         {
    //             "pkg": "python/3.7.3",
    //             "build": {"options": [{"var": "abi", "static": "cp37"}]},
    //         },
    //         {
    //             "pkg": "my-app/1.0.0",
    //             "install": {
    //                 "requirements": [{"pkg": "python"}, {"var": "python.abi/cp27"}]
    //             },
    //         },
    //         {
    //             "pkg": "my-app/2.0.0",
    //             "install": {"requirements": [{"pkg": "python"}, {"var": "abi/cp27"}]},
    //         },
    //     ]
    // )

    // solver.add_repository(Arc::new(Mutex::new(repo)));
    // // python is resolved first to get 3.7
    // solver.add_request(request!("python"));
    // // the addition of this app constrains the python.abi to 2.7
    // solver.add_request(request!("my-app/1"));

    // solution = io::run_and_print_resolve(&solver, 100);

    // assert solution.get("my-app").spec.pkg.version == "1.0.0"
    // assert (
    //     solution.get("python").spec.pkg.version == "2.7.5"
    // ), "should re-resolve python"

    // solver.reset()
    // solver.add_repository(Arc::new(Mutex::new(repo)));
    // // python is resolved first to get 3.7
    // solver.add_request(request!("python"));
    // // the addition of this app constrains the global abi to 2.7
    // solver.add_request(request!("my-app/2"));

    // solution = io::run_and_print_resolve(&solver, 100);

    // assert solution.get("my-app").spec.pkg.version == "2.0.0"
    // assert (
    //     solution.get("python").spec.pkg.version == "2.7.5"
    // ), "should re-resolve python"
    todo!()
}

#[rstest]
fn test_solver_build_options_dont_affect_compat(mut solver: Solver) {
    // // test when a package is resolved with some build option
    // //  - that option can conflict with another packages build options
    // //  - as long as there is no explicit requirement on that option's value

    // dep_v1 = api.Spec.from_dict({"pkg": "build-dep/1.0.0"})
    // dep_v2 = api.Spec.from_dict({"pkg": "build-dep/2.0.0"})

    // a_spec = {
    //     "pkg": "pkga/1.0.0",
    //     "build": {"options": [{"pkg": "build-dep/=1.0.0"}, {"var": "debug/on"}]},
    // }

    // b_spec = {
    //     "pkg": "pkgb/1.0.0",
    //     "build": {"options": [{"pkg": "build-dep/=2.0.0"}, {"var": "debug/off"}]},
    // }

    // let repo = make_repo!(
    //     [
    //         make_build(a_spec.copy(), [dep_v1]),
    //         make_build(b_spec.copy(), [dep_v2]),
    //     ]
    // )
    // repo.publish_spec(api.Spec.from_dict(a_spec))
    // repo.publish_spec(api.Spec.from_dict(b_spec))

    // solver.add_repository(Arc::new(Mutex::new(repo)));
    // // a gets resolved and adds options for debug/on and build-dep/1
    // // to the set of options in the solver
    // solver.add_request(request!("pkga"));
    // // b is not affected and can still be resolved
    // solver.add_request(request!("pkgb"));

    // solution = io::run_and_print_resolve(&solver, 100);

    // solver.reset()
    // solver.add_repository(Arc::new(Mutex::new(repo)));
    // solver.add_repository(Arc::new(Mutex::new(repo)));
    // solver.add_request(request!("pkga"));
    // solver.add_request(request!("pkgb"));
    // // this time the explicit request will cause a failure
    // solver.add_request(api.VarRequest("build-dep", "=1.0.0"))
    // with pytest.raises(solve.SolverError):
    //     solution = io::run_and_print_resolve(&solver, 100);
    todo!()
}

#[rstest]
fn test_solver_components() {
    // // test when a package is requested with specific components
    // // - all the aggregated components are selected in the resolve
    // // - the final build has published layers for each component

    // let repo = make_repo!(
    //     [
    //         {
    //             "pkg": "python/3.7.3",
    //             "install": {
    //                 "components": [
    //                     {"name": "interpreter"},
    //                     {"name": "lib"},
    //                     {"name": "doc"},
    //                 ]
    //             },
    //         },
    //         {
    //             "pkg": "pkga",
    //             "install": {
    //                 "requirements": [{"pkg": "python:lib/3.7.3"}, {"pkg": "pkgb"}]
    //             },
    //         },
    //         {
    //             "pkg": "pkgb",
    //             "install": {"requirements": [{"pkg": "python:{doc,interpreter,run}"}]},
    //         },
    //     ]
    // )

    // solver = Solver()
    // solver.add_repository(Arc::new(Mutex::new(repo)));
    // solver.add_request(request!("pkga"));
    // solver.add_request(request!("pkgb"));

    // solution = io::run_and_print_resolve(&solver, 100);

    // assert solution.get("python").request.pkg.components == {
    //     "interpreter",
    //     "doc",
    //     "lib",
    //     "run",
    // }
    todo!()
}

#[rstest]
fn test_solver_all_component() {
    // // test when a package is requested with the 'all' component
    // // - all the specs components are selected in the resolve
    // // - the final build has published layers for each component

    // let repo = make_repo!(
    //     [
    //         {
    //             "pkg": "python/3.7.3",
    //             "install": {
    //                 "components": [
    //                     {"name": "bin", "uses": ["lib"]},
    //                     {"name": "lib"},
    //                     {"name": "doc"},
    //                     {"name": "dev", "uses": ["doc"]},
    //                 ]
    //             },
    //         },
    //     ]
    // )

    // solver = Solver()
    // solver.add_repository(Arc::new(Mutex::new(repo)));
    // solver.add_request(request!("python:all"));

    // solution = io::run_and_print_resolve(&solver, 100);

    // resolved = solution.get("python")
    // assert resolved.request.pkg.components == set(["all"])
    // expected = ["bin", "build", "dev", "doc", "lib", "run"]
    // source = resolved.source
    // assert isinstance(source, tuple)
    // assert sorted(source[1].keys()) == expected
    todo!()
}

#[rstest]
fn test_solver_component_availability() {
    // // test when a package is requested with some component
    // // - all the specs components are selected in the resolve
    // // - the final build has published layers for each component

    // spec373 = {
    //     "pkg": "python/3.7.3",
    //     "install": {
    //         "components": [
    //             {"name": "bin", "uses": ["lib"]},
    //             {"name": "lib"},
    //         ]
    //     },
    // }
    // spec372 = spec373.copy()
    // spec372["pkg"] = "python/3.7.2"
    // spec371 = spec373.copy()
    // spec371["pkg"] = "python/3.7.1"

    // let repo = make_repo!(
    //     [
    //         # the first pkg has what we want on paper, but didn't actually publish
    //         # the components that we need (missing bin)
    //         make_build_and_components(spec373, components=["lib"]),
    //         # the second pkg has what we request, but is missing a dependant component (lib)
    //         make_build_and_components(spec372, components=["bin"]),
    //         # but the last/lowest version number has a publish for all components
    //         # and should be the one that is selected because of this
    //         make_build_and_components(spec371, components=["bin", "lib"]),
    //     ]
    // )
    // repo.publish_spec(api.Spec.from_dict(spec373))
    // repo.publish_spec(api.Spec.from_dict(spec372))
    // repo.publish_spec(api.Spec.from_dict(spec371))

    // solver = Solver()
    // solver.add_repository(Arc::new(Mutex::new(repo)));
    // solver.add_request(request!("python:bin"));

    // solution = io::run_and_print_resolve(&solver, 100);

    // resolved = solution.get("python")
    // assert resolved.spec.pkg.version == "3.7.1", (
    //     "should resolve the only version with all "
    //     "the components we need actually published"
    // )
    // source = resolved.source
    // assert isinstance(source, tuple)
    // assert sorted(source[1].keys()) == ["bin", "lib"]
    todo!()
}

#[rstest]
fn test_solver_component_requirements() {
    // // test when a component has it's own list of requirements
    // // - the requirements are added to the existing set of requirements
    // // - the additional requirements are resolved
    // // - even if it's a component that's only used by the one that was requested

    // let repo = make_repo!(
    //     [
    //         {
    //             "pkg": "mypkg/1.0.0",
    //             "install": {
    //                 "requirements": [{"pkg": "dep"}],
    //                 "components": [
    //                     {"name": "build", "uses": ["build2"]},
    //                     {"name": "build2", "requirements": [{"pkg": "depb"}]},
    //                     {"name": "run", "requirements": [{"pkg": "depr"}]},
    //                 ],
    //             },
    //         },
    //         {"pkg": "dep"},
    //         {"pkg": "depb"},
    //         {"pkg": "depr"},
    //     ]
    // )

    // solver = Solver()
    // solver.add_repository(Arc::new(Mutex::new(repo)));
    // solver.add_request(request!("mypkg:build"));

    // solution = io::run_and_print_resolve(&solver, 100);

    // solution.get("dep")  # should exist
    // solution.get("depb")  # should exist
    // with pytest.raises(KeyError):
    //     solution.get("depr")

    // solver = Solver()
    // solver.add_repository(Arc::new(Mutex::new(repo)));
    // solver.add_request(request!("mypkg:run"));

    // solution = io::run_and_print_resolve(&solver, 100);

    // solution.get("dep")  # should exist
    // solution.get("depr")  # should exist
    // with pytest.raises(KeyError):
    //     solution.get("depb")
    todo!()
}

#[rstest]
fn test_solver_component_requirements_extending() {
    // // test when an additional component is requested after a package is resolved
    // // - the new components requirements are still added and resolved

    // let repo = make_repo!(
    //     [
    //         {
    //             "pkg": "depa",
    //             "install": {
    //                 "components": [
    //                     {"name": "run", "requirements": [{"pkg": "depc"}]},
    //                 ],
    //             },
    //         },
    //         {"pkg": "depb", "install": {"requirements": [{"pkg": "depa:run"}]}},
    //         {"pkg": "depc"},
    //     ]
    // )

    // solver = Solver()
    // solver.add_repository(Arc::new(Mutex::new(repo)));
    // // the initial resolve of this component will add no new requirements
    // solver.add_request(request!("depa:build"));
    // // depb has its own requirement on depa:run, which, also
    // // has a new requirement on depc
    // solver.add_request(request!("depb"));

    // solution = io::run_and_print_resolve(&solver, 100);

    // solution.get("depc")  # should exist
    todo!()
}

#[rstest]
fn test_solver_component_embedded() {
    // // test when a component has it's own list of embedded packages
    // // - the embedded package is immediately selected
    // // - it must be compatible with any previous requirements

    // let repo = make_repo!(
    //     [
    //         {
    //             "pkg": "mypkg/1.0.0",
    //             "install": {
    //                 "components": [
    //                     {"name": "build", "embedded": [{"pkg": "dep-e1/1.0.0"}]},
    //                     {"name": "run", "embedded": [{"pkg": "dep-e2/1.0.0"}]},
    //                 ],
    //             },
    //         },
    //         {"pkg": "dep-e1/1.0.0"},
    //         {"pkg": "dep-e1/2.0.0"},
    //         {"pkg": "dep-e2/1.0.0"},
    //         {"pkg": "dep-e2/2.0.0"},
    //         {
    //             "pkg": "downstream1",
    //             "install": {
    //                 "requirements": [{"pkg": "dep-e1"}, {"pkg": "mypkg:build"}]
    //             },
    //         },
    //         {
    //             "pkg": "downstream2",
    //             "install": {
    //                 "requirements": [{"pkg": "dep-e2/2.0.0"}, {"pkg": "mypkg:run"}]
    //             },
    //         },
    //     ]
    // )

    // solver = Solver()
    // solver.add_repository(Arc::new(Mutex::new(repo)));
    // solver.add_request(request!("downstream1"));

    // solution = io::run_and_print_resolve(&solver, 100);

    // assert solution.get("dep-e1").spec.pkg.build == "embedded"

    // solver = Solver()
    // solver.add_repository(Arc::new(Mutex::new(repo)));
    // solver.add_request(request!("downstream2"));

    // with pytest.raises(SolverError):
    //     # should fail because the one embedded package
    //     # does not meet the requirements in downstream spec
    //     solution = io::run_and_print_resolve(&solver, 100);
    todo!()
}

#[rstest]
fn test_request_default_component() {
    let mut solver = Solver::new();
    solver
        .py_add_request(RequestEnum::String("python/3.7.3".into()))
        .unwrap();
    let state = solver.get_initial_state();
    let request = state
        .pkg_requests
        .get(0)
        .expect("solver should have a request");
    assert_eq!(
        request.pkg.components,
        vec![api::Component::Run].into_iter().collect(),
        "solver should inject a default run component if not otherwise given"
    )
}
