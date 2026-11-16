"""固定した外部 DLL だけを Git の外へ用意する。ソースやスキーマは展開しない。"""

import argparse
import hashlib
import io
import json
from pathlib import Path
import subprocess
import sys
import urllib.request
import zipfile

HERE = Path(__file__).resolve().parent
PACKAGES = [
    ("Smino.Bcf.Toolkit", "3.2.0", "Apache-2.0", "lib/net9.0/bcf-toolkit.dll", "1bdb645172e5c80e6da3f8b1e562cac3fd007a1b7320980738af8b13d7922087"),
    ("Newtonsoft.Json", "13.0.3", "MIT", "lib/net6.0/Newtonsoft.Json.dll", "872fc189e638ab1056555b03aaa38f68bcb54286e221aa646eb1129babf63c77"),
    ("RecursiveDataAnnotationsValidation", "2.0.0", "MIT", "lib/netstandard2.0/RecursiveDataAnnotationsValidation.dll", "383afb3f17fd1dea6273be6a6387aa242684c32eb9592d91341d4a493960a2cf"),
    ("Serilog", "4.0.0", "Apache-2.0", "lib/net8.0/Serilog.dll", "8fc850e5374bd538df7468813bd3f21c914c30fbc04c758dd5db6bad465994dc"),
    ("System.CommandLine", "2.0.0-beta4.22272.1", "MIT", "lib/net6.0/System.CommandLine.dll", "cd23be0989cc1fc75e0470c8f431e108f8fbf427b718ecc70b21f5fd3887c5c7"),
]


def sha(data):
    return hashlib.sha256(data).hexdigest()


def write_json(path, value):
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2, sort_keys=True) + "\n", encoding="utf-8", newline="\n")


def new_folder(path):
    path = path.resolve()
    if path.exists() or any((parent / ".git").exists() for parent in [path, *path.parents]):
        raise ValueError("output.must_be_new_and_outside_git")
    path.mkdir(parents=True)
    return path


def select_sdk(listing):
    versions = []
    for line in listing.splitlines():
        parts = line.split()
        if not parts:
            continue
        version = parts[0].split(".")
        if len(version) == 3 and version[0] == "10" and all(part.isdigit() for part in version):
            versions.append(tuple(int(part) for part in version))
    if not versions:
        raise ValueError("sdk.ten_required")
    return ".".join(str(part) for part in max(versions))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out", required=True, type=Path)
    args = parser.parse_args()
    output = new_folder(args.out)
    (output / "lib").mkdir()
    result = {"complete": False, "packages": [], "oldRegexPackageInstalled": False, "schemaPublished": False}
    try:
        for name, version, license_name, entry, digest in PACKAGES:
            url = f"https://api.nuget.org/v3-flatcontainer/{name.lower()}/{version}/{name.lower()}.{version}.nupkg"
            with urllib.request.urlopen(url, timeout=30) as response:
                data = response.read(32 * 1024 * 1024 + 1)
            if len(data) > 32 * 1024 * 1024 or sha(data) != digest:
                raise ValueError("package.sha256")
            with zipfile.ZipFile(io.BytesIO(data)) as archive:
                if archive.getinfo(entry).file_size > 16 * 1024 * 1024:
                    raise ValueError("assembly.limit")
                assembly = archive.read(entry)
            filename = Path(entry).name
            (output / "lib" / filename).write_bytes(assembly)
            result["packages"].append({"name": name, "version": version, "license": license_name, "url": url,
                                       "packageSha256": digest, "assembly": filename, "assemblySha256": sha(assembly)})
        sources = [HERE / "prepare.py", *sorted((HERE / "external").iterdir())]
        result["sourceSha256"] = {path.relative_to(HERE).as_posix(): sha(path.read_bytes()) for path in sources if path.is_file()}
        listing = subprocess.check_output(["dotnet", "--list-sdks"], cwd=output, text=True, timeout=30)
        selected = select_sdk(listing)
        write_json(output / "global.json", {"sdk": {"version": selected, "rollForward": "disable", "allowPrerelease": False}})
        result["sdk"] = subprocess.check_output(["dotnet", "--version"], cwd=output, text=True, timeout=30).strip()
        if result["sdk"] != selected:
            raise ValueError("sdk.selection")
        command = ["dotnet", "build", str(HERE / "external/ReadBcf.csproj"), "--configfile", str(HERE / "external/NuGet.Config"),
                   "--nologo", "-v", "quiet", "-p:BcfTools=" + str(output), "-p:BaseIntermediateOutputPath=" + str(output / "obj") + "/",
                   "-p:OutputPath=" + str(output / "bin") + "/"]
        with (output / "build.log").open("wb") as log:
            built = subprocess.run(command, stdout=log, stderr=subprocess.STDOUT, cwd=output, timeout=120, check=False)
        if built.returncode != 0:
            raise ValueError("build.failed")
        metadata = subprocess.run(["dotnet", str(output / "bin/ReadBcf.dll"), "metadata"], stdout=subprocess.PIPE,
                                  stderr=subprocess.DEVNULL, cwd=output, timeout=30, check=True)
        result["runtime"] = json.loads(metadata.stdout)
        if result["runtime"]["regexIsFramework"] is not True:
            raise ValueError("regex.not_framework")
        result["binarySha256"] = {path.name: sha(path.read_bytes()) for path in sorted((output / "bin").iterdir()) if path.is_file()}
        result["complete"] = True
    except Exception as error:
        result["errorType"] = type(error).__name__
    write_json(output / "tools.json", result)
    print(json.dumps({"complete": result["complete"], "packages": len(result["packages"])}))
    return 0 if result["complete"] else 2


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except Exception as error:
        print(json.dumps({"complete": False, "errorType": type(error).__name__}))
        raise SystemExit(2) from None
