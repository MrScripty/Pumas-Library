"""Fully declared synthetic target fixtures; never infer an inspection host."""


def target(system, *, python, libc="glibc", libc_version="2.17"):
    if system not in {"linux", "windows", "macos"}:
        raise ValueError("Unknown fixture target")
    sys_platform, os_name, platform_system, machine, release, version = {
        "linux": ("linux", "posix", "Linux", "x86_64", "6.1.0", "fixture-linux-6.1.0"),
        "windows": ("win32", "nt", "Windows", "AMD64", "11", "10.0.22631"),
        "macos": ("darwin", "posix", "Darwin", "arm64", "23.0.0", "fixture-darwin-23.0.0"),
    }[system]
    return {"schema": "pumas.wheel-target.v1", "python": python,
            "abi": "cp" + "".join(python.split(".")[:2]), "os": system,
            "arch": "arm64" if system == "macos" else "x86_64",
            "libc": {"family": libc, "version": libc_version} if system == "linux" else None,
            "macos_deployment": "14.0" if system == "macos" else None,
            "native_linux_tag": system == "linux",
            "markers": {"implementation_name": "cpython", "implementation_version": python,
                "platform_python_implementation": "CPython", "python_full_version": python,
                "python_version": ".".join(python.split(".")[:2]), "sys_platform": sys_platform,
                "os_name": os_name, "platform_machine": machine, "platform_system": platform_system,
                "platform_release": release, "platform_version": version}}
