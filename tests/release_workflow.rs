// ==========================================================================
// File    : tests/release_workflow.rs
// Project : AuraScope
// Layer   : Release
// Purpose : Guards the release workflow against Linux runner regressions that
//           would raise the minimum glibc version of published binaries.
//
// Author  : Ahmed Ashour
// Created : 2026-06-13
// ==========================================================================

// --------------------------------------------------------------------------
// [SECTION] Workflow Compatibility Tests
// --------------------------------------------------------------------------

#[test]
fn test_release_workflow_pins_linux_builds_to_ubuntu_22_04() {
    let workflow = std::fs::read_to_string(".github/workflows/release.yml").unwrap();

    assert!(
        workflow.contains("- os: ubuntu-22.04"),
        "Linux binary builds must target ubuntu-22.04 for older glibc compatibility"
    );
    assert!(
        workflow.contains("build-linux-packages:\n    runs-on: ubuntu-22.04"),
        "Linux package builds must run on ubuntu-22.04 for Debian-compatible glibc output"
    );
    assert!(
        !workflow.contains("- os: ubuntu-latest\n            target: x86_64-unknown-linux-gnu"),
        "Linux binary builds must not use ubuntu-latest"
    );
    assert!(
        !workflow.contains("build-linux-packages:\n    runs-on: ubuntu-latest"),
        "Linux package builds must not use ubuntu-latest"
    );
}
