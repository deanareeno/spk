// Copyright (c) 2021 Sony Pictures Imageworks, et al.
// SPDX-License-Identifier: Apache-2.0
// https://github.com/imageworks/spk
use rstest::rstest;

use super::DecisionBuilder;
use crate::{api, solve};

#[rstest]
fn test_resolve_build_same_result() {
    // building a package and resolving an binary build
    // should both result in the same final state... this
    // ensures that builds are not attempted when one already exists

    // base = graph.State.default()

    // spec = api.Spec.from_dict({"pkg": "test/1.0.0"})
    // build_spec = spec.copy()
    // build_spec.update_spec_for_build(api.OptionMap(), [])

    // resolve = graph.ResolvePackage(build_spec, base, set(), build_spec)
    // build = graph.BuildPackage(spec, base, set(), Solution())

    // with_binary = resolve.apply(base)
    // with_build = build.apply(base)

    // print("resolve")
    // for change in resolve.iter_changes():
    //     print(io.format_change(change, 100))
    // print("build")
    // for change in build.iter_changes():
    //     print(io.format_change(change, 100))

    // assert (
    //     with_binary.id == with_build.id
    // ), "Build and resolve package should create the same final state"
    todo!()
}

#[rstest]
fn test_empty_options_do_not_unset() {
    // state = graph.State.default()

    // assign_empty = graph.SetOptions(api.OptionMap({"something": ""}))
    // assign_value = graph.SetOptions(api.OptionMap({"something": "value"}))

    // new_state = assign_empty.apply(state)
    // opts = new_state.get_option_map()
    // assert opts["something"] == "", "should assign empty option of no current value"

    // new_state = assign_value.apply(new_state)
    // new_state = assign_empty.apply(new_state)
    // opts = new_state.get_option_map()
    // assert opts["something"] == "value", "should not unset value when one exists"
    todo!()
}

#[rstest]
fn test_request_default_component() {
    let spec: api::Spec = serde_yaml::from_str(
        r#"{
        pkg: parent,
        install: {
          requirements: [
            {pkg: dependency/1.0.0}
          ]
        }
    }"#,
    )
    .unwrap();
    let spec = std::sync::Arc::new(spec);
    let base = super::State::default();

    let resolve_state = DecisionBuilder::new(spec.clone(), &base)
        .resolve_package(solve::solution::PackageSource::Spec(spec.clone()))
        .apply(base.clone());
    let request = resolve_state.get_merged_request("dependency").unwrap();
    assert!(
        request.pkg.components.contains(&api::Component::Run),
        "default run component should be injected when none specified"
    );

    let build_state = DecisionBuilder::new(spec, &base)
        .build_package(&solve::solution::Solution::new(None))
        .unwrap()
        .apply(base.clone());
    let request = build_state.get_merged_request("dependency").unwrap();
    assert!(
        request.pkg.components.contains(&api::Component::Run),
        "default run component should be injected when none specified"
    );
}
