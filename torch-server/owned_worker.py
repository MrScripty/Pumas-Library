"""Private installed worker bootstrap; descriptors are capabilities, not qualification.

Reuse the existing installed interpreter/packages. No listener, installer, model
acquisition or decoded permission receipt is provided here. Shipping admission
stays unavailable. The parent must retain code/interpreter/dependency/source
custody; bootstrap isolation alone does not qualify real ASR or native libraries.
"""

import argparse
import asyncio
import importlib.machinery
import os
from pathlib import Path
import stat
import sys
import sysconfig

REQUIRED_CODE = (
    "owned_worker.py",
    "owned_audio.py",
    "installed_cohere_source.py",
    "model_manager.py",
    "device_manager.py",
    "private_owned_channel.py",
    "owned_model_operations.py",
    "speech_binding.py",
    "speech_operations.py",
    "native_speech_result.py",
    "audio_input.py",
    "audio_contract.py",
    "loaders/__init__.py",
    "loaders/cohere_asr_loader.py",
    "loaders/owned_cohere_source.py",
)
REQUIRED_MODEL = frozenset(
    {
        "config.json",
        "model.safetensors",
        "preprocessor_config.json",
        "tokenizer.json",
        "tokenizer_config.json",
    }
)
OPTIONAL_MODEL = frozenset(
    {
        "added_tokens.json",
        "generation_config.json",
        "processor_config.json",
        "special_tokens_map.json",
    }
)
MAX_MEMBERS = 200_000
# Import roots outlive the channel coroutine: orderly loop shutdown can still
# drain noninterruptible native workers. Only exact process exit closes these.
_BOOTSTRAP_CUSTODY = None


class StartupRefusal(ValueError):
    def __init__(self, code):
        self.code = code
        super().__init__(code)


def _inert_import_member(path):
    parts = Path(path).parts
    return "__pycache__" in parts or any(
        name.endswith((".pth", ".pyc", ".pyo")) or name in {"sitecustomize.py", "usercustomize.py"}
        for name in parts
    )


def _inspect(
    fd, *, prefix="", seen=None, depth=0, count=None, code_root=False, inert_imports=False
):
    seen = set() if seen is None else seen
    count = [0] if count is None else count
    if depth > 64:
        raise StartupRefusal("runtime_member_limit")
    for name in os.listdir(fd):
        count[0] += 1
        if count[0] > MAX_MEMBERS:
            raise StartupRefusal("runtime_member_limit")
        relative = prefix + name
        info = os.stat(name, dir_fd=fd, follow_symlinks=False)
        # The installed sidecar owns code beside its separately retained venv.
        # This fixed namespace is unselected for the code role, never a caller
        # supplied exclusion. Its contents cannot enter the import boundary.
        if code_root and not prefix and name == "venv":
            if not stat.S_ISDIR(info.st_mode):
                raise StartupRefusal("unsupported_runtime_member")
            continue
        if _inert_import_member(relative):
            if not inert_imports or not (stat.S_ISREG(info.st_mode) or stat.S_ISDIR(info.st_mode)):
                raise StartupRefusal("unsupported_runtime_member")
            # Fixed inert namespaces receive no kernel content grants. No caller
            # can configure this list. The source-only loader below also avoids
            # Python's otherwise implicit .pyc read alongside a selected .py.
            continue
        if stat.S_ISDIR(info.st_mode):
            nested = os.open(name, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW, dir_fd=fd)
            try:
                _inspect(
                    nested,
                    prefix=relative + "/",
                    seen=seen,
                    depth=depth + 1,
                    count=count,
                    inert_imports=inert_imports,
                )
            finally:
                os.close(nested)
        elif stat.S_ISREG(info.st_mode):
            seen.add(relative)
            if len(seen) > MAX_MEMBERS:
                raise StartupRefusal("runtime_member_limit")
        else:
            raise StartupRefusal("unsupported_runtime_member")
    return seen


class _SourceOnlyLoader(importlib.machinery.SourceFileLoader):
    def get_code(self, fullname):
        filename = self.get_filename(fullname)
        return self.source_to_code(self.get_data(filename), filename)


class _RetainedImports:
    def __init__(self, roots, excluded):
        self.roots = roots
        self.excluded = excluded

    def admitted(self, location):
        lexical = Path(location).absolute()
        actual = Path(location).resolve(strict=True)
        if _inert_import_member(lexical) or _inert_import_member(actual):
            return False
        if any(lexical.is_relative_to(root) for root in self.excluded):
            return False
        # The package role may physically live inside the installed venv; its
        # separate capability admits only that selected subtree. A code-role
        # /proc/FD/venv traversal above remains refused even into these bytes.
        if actual.is_relative_to(self.roots[1]):
            return True
        return not any(actual.is_relative_to(root) for root in self.excluded) and any(
            actual.is_relative_to(root) for root in self.roots
        )

    def find_spec(self, fullname, path=None, target=None):
        spec = importlib.machinery.PathFinder.find_spec(fullname, path, target)
        if spec is None:
            return None
        locations = list(spec.submodule_search_locations or ())
        if spec.origin is not None:
            locations.append(spec.origin)
        if not locations:
            raise StartupRefusal("ambient_import_refused")
        for location in locations:
            if not self.admitted(location):
                raise StartupRefusal("ambient_import_refused")
        if type(spec.loader) is importlib.machinery.SourceFileLoader:
            spec.loader = _SourceOnlyLoader(fullname, spec.origin)
        return spec


def parse_args(argv=None):
    parser = argparse.ArgumentParser(description="Private inherited owned worker")
    for root in ("code", "packages", "model"):
        parser.add_argument(f"--{root}-root-fd", required=True, type=int)
    parser.add_argument("--experimental-local-cohere", action="store_true")
    parser.add_argument("--model-id")
    parser.add_argument("--selected-artifact-id")
    return parser.parse_args(argv)


async def bootstrap(args, *, channel_factory=None):
    """Internal factory injection is for controlled tests; CLI cannot select it."""
    global _BOOTSTRAP_CUSTODY
    if _BOOTSTRAP_CUSTODY is not None:
        raise StartupRefusal("bootstrap_already_installed")
    if (
        sys.platform != "linux"
        or not sys.flags.isolated
        or not sys.flags.no_site
        or not sys.dont_write_bytecode
    ):
        raise StartupRefusal("unsupported_bootstrap")
    supplied = (args.code_root_fd, args.packages_root_fd, args.model_root_fd)
    if len(set(supplied)) != 3 or any(type(fd) is not int or fd < 0 for fd in supplied):
        raise StartupRefusal("invalid_bootstrap_descriptor")
    owned = []
    output = None
    previous_path, previous_meta = sys.path[:], sys.meta_path[:]
    try:
        for fd in supplied:
            copy = os.dup(fd)
            owned.append(copy)
            if not stat.S_ISDIR(os.fstat(copy).st_mode):
                raise StartupRefusal("invalid_bootstrap_descriptor")
        code, packages, model = owned
        if not set(REQUIRED_CODE).issubset(_inspect(code, code_root=True, inert_imports=True)):
            raise StartupRefusal("missing_bootstrap_member")
        _inspect(packages, inert_imports=True)
        members = _inspect(model)
        if not REQUIRED_MODEL.issubset(members) or not members.issubset(
            REQUIRED_MODEL | OPTIONAL_MODEL
        ):
            raise StartupRefusal("unsupported_model_read_set")
        # -I -S starts with interpreter-owned stdlib paths only; never admit cwd,
        # environment search paths, automatic site directories or .pth hooks.
        stdlib = Path(sysconfig.get_path("stdlib")).resolve()
        standard = [
            value
            for value in previous_path
            if value
            and Path(value).resolve().is_relative_to(stdlib)
            and not any(part in {"site-packages", "dist-packages"} for part in Path(value).parts)
        ]
        code_path, packages_path = (f"/proc/self/fd/{fd}" for fd in (code, packages))
        roots = [Path(value).resolve() for value in (code_path, packages_path, *standard)]
        imports = _RetainedImports(roots, (Path(code_path) / "venv", roots[0] / "venv"))
        if Path(__file__).resolve().parent != roots[0]:
            raise StartupRefusal("bootstrap_source_replaced")
        for module in tuple(sys.modules.values()):
            origin = getattr(getattr(module, "__spec__", None), "origin", None)
            if origin and origin not in {"built-in", "frozen"} and not imports.admitted(origin):
                raise StartupRefusal("ambient_import_refused")
        sys.path[:] = [code_path, packages_path, *standard]
        sys.meta_path[:] = [
            importlib.machinery.BuiltinImporter,
            importlib.machinery.FrozenImporter,
            imports,
        ]
        # Reserve the original inherited pipe for complete frames. Native and
        # Python import/worker diagnostics use stderr even if they write FD 1.
        sys.stdout.flush()
        output = os.fdopen(os.dup(sys.stdout.fileno()), "wb", buffering=0)
        os.dup2(sys.stderr.fileno(), sys.stdout.fileno())
        _BOOTSTRAP_CUSTODY = (tuple(owned), output)
        if channel_factory is None:
            from device_manager import DeviceManager
            from model_manager import ModelManager
            from private_owned_channel import _create_bootstrap_private_owned_channel

            try:
                channel = _create_bootstrap_private_owned_channel(
                    ModelManager(DeviceManager()),
                    model,
                    args.model_id,
                    args.selected_artifact_id,
                    experimental=getattr(args, "experimental_local_cohere", False),
                )
            except (ValueError, OSError) as error:
                raise StartupRefusal("installed_runtime_unready") from error
        else:
            channel = channel_factory(model)
        await channel.serve_stdio(output=output)
    except (OSError, ImportError) as error:
        raise StartupRefusal("installed_runtime_unready") from error
    finally:
        if _BOOTSTRAP_CUSTODY is None:
            sys.path[:], sys.meta_path[:] = previous_path, previous_meta
            for fd in owned:
                os.close(fd)
            if output is not None:
                output.close()


def main():
    try:
        asyncio.run(bootstrap(parse_args()))
    except StartupRefusal as error:
        print(error.code, file=sys.stderr)
        return 2
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
