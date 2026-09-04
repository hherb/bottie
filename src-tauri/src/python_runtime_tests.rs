//! Focused contracts for opt-in packaged Python runtime discovery.

use std::{fs, path::Path};

use uuid::Uuid;

use crate::python_runtime::{
    PythonBundlePaths, PythonBundlePlatform, resolve_python_bundle_paths,
    windows_profile_arguments, windows_profile_moniker,
};

/// Creates one isolated fixture root below the host temporary directory.
fn fixture_root() -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!("bottie-python-runtime-test-{}", Uuid::new_v4()));
    fs::create_dir_all(&root).expect("fixture root should be created");
    root
}

/// Creates one ordinary file together with its parent directory.
fn write_file(path: &Path) {
    fs::create_dir_all(path.parent().expect("fixture file should have a parent"))
        .expect("fixture parent should be created");
    fs::write(path, b"fixture").expect("fixture file should be written");
}

/// Creates the common opt-in marker and runtime directory.
fn common_bundle(executable: &Path, resources: &Path) {
    fs::create_dir_all(executable).expect("executable directory should be created");
    write_file(&resources.join("python-runtime-evidence.json"));
    fs::create_dir_all(resources.join("python-runtime"))
        .expect("runtime directory should be created");
}

#[test]
fn absent_evidence_marker_keeps_default_packages_disabled() {
    let root = fixture_root();
    let resolved = resolve_python_bundle_paths(
        PythonBundlePlatform::Linux,
        &root.join("bin"),
        &root.join("resources"),
    )
    .expect("an ordinary package without the opt-in marker should be accepted");

    assert_eq!(resolved, None);
    fs::remove_dir_all(root).expect("fixture should be removed");
}

#[test]
fn resolves_each_platform_only_from_fixed_native_bundle_locations() {
    let root = fixture_root();

    let linux_executable = root.join("linux/bin");
    let linux_resources = root.join("linux/lib/bottie");
    common_bundle(&linux_executable, &linux_resources);
    write_file(&linux_executable.join("bottie-python-runner"));
    assert_eq!(
        resolve_python_bundle_paths(
            PythonBundlePlatform::Linux,
            &linux_executable,
            &linux_resources,
        )
        .expect("complete Linux bundle should resolve"),
        Some(PythonBundlePaths::Linux {
            runner: linux_executable.join("bottie-python-runner"),
            runtime: linux_resources.join("python-runtime"),
        })
    );

    let windows_executable = root.join("windows");
    let windows_resources = windows_executable.clone();
    common_bundle(&windows_executable, &windows_resources);
    write_file(&windows_executable.join("bottie-python-appcontainer.exe"));
    write_file(&windows_executable.join("bottie-python-runner.exe"));
    assert_eq!(
        resolve_python_bundle_paths(
            PythonBundlePlatform::Windows,
            &windows_executable,
            &windows_resources,
        )
        .expect("complete Windows bundle should resolve"),
        Some(PythonBundlePaths::Windows {
            controller: windows_executable.join("bottie-python-appcontainer.exe"),
            runner: windows_executable.join("bottie-python-runner.exe"),
            runtime: windows_resources.join("python-runtime"),
        })
    );

    let macos_contents = root.join("Bottie.app/Contents");
    let macos_executable = macos_contents.join("MacOS");
    let macos_resources = macos_contents.join("Resources");
    common_bundle(&macos_executable, &macos_resources);
    write_file(&macos_executable.join("bottie-python-xpc-client"));
    let service = macos_contents.join("XPCServices/com.bottie.python-runner.xpc/Contents");
    write_file(&service.join("Info.plist"));
    write_file(&service.join("MacOS/bottie-python-xpc-service"));
    write_file(&service.join("Helpers/bottie-python-runner"));
    write_file(&service.join("Resources/python-runtime-evidence.json"));
    fs::create_dir_all(service.join("Resources/python-runtime"))
        .expect("nested runtime directory should be created");
    assert_eq!(
        resolve_python_bundle_paths(
            PythonBundlePlatform::Macos,
            &macos_executable,
            &macos_resources,
        )
        .expect("complete macOS bundle should resolve"),
        Some(PythonBundlePaths::Macos {
            client: macos_executable.join("bottie-python-xpc-client"),
        })
    );

    fs::remove_dir_all(root).expect("fixture should be removed");
}

#[test]
fn opt_in_bundle_fails_closed_when_one_native_resource_is_missing() {
    let root = fixture_root();
    let executable = root.join("bin");
    let resources = root.join("resources");
    common_bundle(&executable, &resources);

    let error = resolve_python_bundle_paths(PythonBundlePlatform::Linux, &executable, &resources)
        .expect_err("an incomplete opt-in bundle must fail closed");

    assert_eq!(
        error.message(),
        "The packaged Python runtime is incomplete."
    );
    assert!(!format!("{error:?}").contains(root.to_string_lossy().as_ref()));
    fs::remove_dir_all(root).expect("fixture should be removed");
}

#[test]
fn windows_profile_lifecycle_uses_a_distinct_controller_safe_moniker_per_process() {
    let first = windows_profile_moniker(41);
    let second = windows_profile_moniker(42);

    assert_eq!(first, "com.bottie.python.runner.41");
    assert_eq!(second, "com.bottie.python.runner.42");
    assert_ne!(first, second);
    assert!(first.len() <= 64);
    assert!(first.chars().all(|character| character.is_ascii_lowercase()
        || character.is_ascii_digit()
        || character == '.'));
    assert_eq!(
        windows_profile_arguments(true, &first),
        ["prepare", "com.bottie.python.runner.41"]
    );
    assert_eq!(
        windows_profile_arguments(false, &first),
        ["cleanup", "com.bottie.python.runner.41"]
    );
}
