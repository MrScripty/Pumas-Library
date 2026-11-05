#!/usr/bin/env python3
"""Run the per-host native Rust qualification, one cargo invocation per configuration.

Each group names one cargo configuration (package, features, targets) and the
test filters that must run for it on a native host. A group builds once and
runs all its filters together, instead of one cargo invocation (and one
re-check of every test executable) per filter.

Every filter must match at least one runnable (not ignored) test on the host,
so a renamed, moved or removed test fails the gate with the filter named,
instead of passing with nothing run. Each cargo exit status is checked.
"""

import argparse
import platform
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
MANIFEST = "rust/Cargo.toml"
HOSTS = ("Linux", "Windows", "macOS")
ALL = HOSTS
UNIX = ("Linux", "macOS")
WINDOWS = ("Windows",)
LINUX = ("Linux",)
WINDOWS_AND_MACOS = ("Windows", "macOS")

# `--tests` selects the library's unit tests, binaries and integration tests,
# and leaves out doctests (Rust quality runs those on Linux).
LIB = ("--lib",)
TESTS = ("--tests",)


@dataclass(frozen=True)
class Filter:
    text: str
    hosts: tuple = ALL


@dataclass(frozen=True)
class Group:
    shard: str
    title: str
    package: str
    targets: tuple
    filters: tuple
    features: tuple = ()
    test_args: tuple = ()

    def command(self):
        return [
            "cargo",
            "test",
            "--locked",
            "--manifest-path",
            MANIFEST,
            "-p",
            self.package,
            *self.features,
            *self.targets,
        ]


def f(text, hosts=ALL):
    return Filter(text, hosts)


GROUPS = (
    Group(
        "torch",
        "Torch and version management",
        "pumas-app-manager",
        TESTS,
        (
            f("torch_alternatives"),
            f("version_manager::managed_python::tests"),
            f("version_manager::torch_preview::tests", LINUX),
            f("auto_python_fallback_requires_a_definite_unsupported_attempt", WINDOWS_AND_MACOS),
            f("restart_falls_back_from_invalid_active_torch_to_verified_default", WINDOWS),
            f("locked_stage_stays_owned", WINDOWS),
            # Acquisition workspace cleanup must run on every native host.
            f("native_"),
        ),
    ),
    Group(
        "default-features",
        "Library unit tests",
        "pumas-library",
        LIB,
        (
            f("copied_import"),
            f("model_library::download_recovery::tests"),
            f("model_library::library::tests"),
            f("path_identity_tests"),
            f("display_identity_strips_drive_and_unc_verbatim_prefixes_without_io", WINDOWS),
            f("shard_discovery"),
            f("shard_startup_tests"),
            f("test_recover_incomplete_shards_async_detects_missing_shard_set"),
            f("test_find_interrupted_downloads_async_detects_partial_download_dir"),
        ),
        test_args=("--nocapture",),
    ),
    Group(
        "default-features",
        "Library unit and integration tests",
        "pumas-library",
        TESTS,
        (
            f("retained_location"),
            # Reserved acquisition workspace custody must run on every native host.
            f("acquisition::workspace::tests"),
            f("copied_import_nested_native_publication_rebinds_exact_payload", WINDOWS),
            f("model_library::package_facts::gguf::tests"),
            f("gguf_inspector_revision_invalidates_legacy_fingerprint_only_for_gguf"),
            f(
                "intent_api_tests::gguf_header_quantization_matches_canonical_label_but_filename_is_not_proof"
            ),
            f(
                "package_facts_resolution::stale_gguf_detail_and_summary_reinspect_raw_header_before_becoming_fresh"
            ),
            f("model_library::importer::tests"),
            f("model_library::sharding::tests"),
            f("registry::library_registry::tests"),
            # A failed start keeps its registry claim (formerly
            # startup_guard_cannot_delete_a_successor_or_promoted_instance).
            f("failed_construction_retains_claim_instead_of_asserting_initializer_cessation"),
            f("intent_ipc_tests::"),
            f("test_venv_python_path"),
            # Held-parent renames, including replacing a target another handle has open.
            f("platform::capability_fs::tests", WINDOWS),
            f("platform::linux_group", LINUX),
            f(
                "owned_sse_observes_external_load_unload_reconnect_and_drains_before_replacement",
                LINUX,
            ),
            f("silent_sse_expires_its_lease_and_retains_rows_without_gateway_authority", LINUX),
            # Unix process groups and Windows Jobs share one test module.
            f("platform::managed_child"),
        ),
        test_args=("--nocapture",),
    ),
    Group(
        "inference-disabled",
        "Inference-disabled library",
        "pumas-library",
        TESTS,
        (f("retained_location"),),
        features=("--no-default-features",),
    ),
    Group(
        "default-features",
        "RPC",
        "pumas-rpc",
        TESTS,
        (
            f("http_transport::tests"),
            f(
                "integration_tests::tests::rpc_shutdown_exits_after_delivering_acknowledgement_and_ending_sse"
            ),
            f(
                "integration_tests::tests::http_admission_rejects_untrusted_sources_before_rpc_dispatch"
            ),
            f(
                "integration_tests::tests::unix_termination_signals_complete_the_owned_shutdown",
                UNIX,
            ),
        ),
    ),
    Group(
        "inference-disabled",
        "Inference-disabled RPC",
        "pumas-rpc",
        TESTS,
        (f("http_transport::tests"),),
        features=("--no-default-features",),
    ),
)

SHARDS = tuple(dict.fromkeys(group.shard for group in GROUPS))


def host_name(system=None):
    system = system or platform.system()
    return {"Linux": "Linux", "Windows": "Windows", "Darwin": "macOS"}.get(system, system)


def active_filters(group, host):
    return [item.text for item in group.filters if host in item.hosts]


def listed_tests(output):
    suffix = ": test"
    return {line[: -len(suffix)] for line in output.splitlines() if line.endswith(suffix)}


def unmatched_filters(filters, runnable):
    return [text for text in filters if not any(text in name for name in runnable)]


def run_cargo(arguments):
    print("+ " + " ".join(arguments), flush=True)
    return subprocess.run(arguments, cwd=ROOT, text=True, capture_output=True)


def checked_list(command, filters, extra=()):
    listed = run_cargo([*command, "--", "--list", *extra, *filters])
    if listed.returncode != 0:
        sys.stdout.write(listed.stdout)
        sys.stderr.write(listed.stderr)
        raise SystemExit(f"Listing tests failed with exit status {listed.returncode}")
    return listed_tests(listed.stdout)


def run_group(group, host):
    """Run one group's filters; raise SystemExit with the reason it failed."""
    filters = active_filters(group, host)
    if not filters:
        return
    print(f"::group::{group.title} ({group.package} {' '.join(group.features + group.targets)})")
    try:
        command = group.command()
        runnable = checked_list(command, filters) - checked_list(command, filters, ("--ignored",))
        missing = unmatched_filters(filters, runnable)
        if missing:
            raise SystemExit(
                f"{group.title}: no runnable test on {host} matches "
                + ", ".join(repr(text) for text in missing)
                + "; a renamed or removed test must be updated here"
            )
        print(f"{len(runnable)} tests match {len(filters)} filters", flush=True)
        print("+ " + " ".join([*command, "--", *group.test_args, *filters]), flush=True)
        executed = subprocess.run([*command, "--", *group.test_args, *filters], cwd=ROOT)
        if executed.returncode != 0:
            raise SystemExit(f"{group.title}: tests failed with exit status {executed.returncode}")
    finally:
        print("::endgroup::", flush=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--shard", choices=SHARDS, required=True)
    args = parser.parse_args()
    host = host_name()
    if host not in HOSTS:
        raise SystemExit(f"Unsupported native host {platform.system()!r}")
    # A CI run is long: run every group and report every failure together.
    failures = []
    for group in GROUPS:
        if group.shard == args.shard:
            try:
                run_group(group, host)
            except SystemExit as failure:
                print(f"::error::{failure}", flush=True)
                failures.append(str(failure))
    if failures:
        raise SystemExit("Native qualification failed:\n  " + "\n  ".join(failures))


if __name__ == "__main__":
    main()
