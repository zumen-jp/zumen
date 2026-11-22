"""Maven Central に送る前の ZIP を開き、座標・内容・チェックサムを照合する。"""

import argparse
import hashlib
import io
import zipfile
from pathlib import Path

from package import OWN_LICENSES, ROOT, VERSION, require


def verify_java(path):
    source = ROOT / "bindings/java"
    api = source / "src/main/java"
    sources = {p.relative_to(api).as_posix(): p for p in api.rglob("*.java")}
    prefix = f"dev/zumen/zumen/{VERSION}/zumen-{VERSION}"
    pom = (source / "pom.xml").read_bytes()
    metadata = {
        "META-INF/MANIFEST.MF",
        "META-INF/maven/dev.zumen/zumen/pom.xml",
        "META-INF/maven/dev.zumen/zumen/pom.properties",
    }
    shared = {"README.md": (source / "README.md").read_bytes()}
    shared.update({f"META-INF/{name}": (ROOT / name).read_bytes() for name in OWN_LICENSES})
    with zipfile.ZipFile(path) as bundle:
        require(bundle.read(prefix + ".pom") == pom, "配布用の POM が元のファイルと違う")
        expected = set()
        for suffix in [".pom", ".jar", "-sources.jar", "-javadoc.jar"]:
            name = prefix + suffix
            expected.add(name)
            data = bundle.read(name)
            for algorithm in ["md5", "sha1", "sha256", "sha512"]:
                checksum = f"{name}.{algorithm}"
                expected.add(checksum)
                require(bundle.read(checksum).decode().strip() == hashlib.new(algorithm, data).hexdigest(),
                        f"{name} の {algorithm} が違う")
            if suffix == ".pom":
                continue
            with zipfile.ZipFile(io.BytesIO(data)) as jar:
                files = [member.filename for member in jar.infolist() if not member.is_dir()]
                require(len(files) == len(set(files)), f"{name} に同名のファイルがある")
                for member, content in shared.items():
                    require(jar.read(member) == content, f"{name} の {member} が元のファイルと違う")
                contents = set(files) - metadata - set(shared)
                if suffix == ".jar":
                    require(all(p.endswith(".class") for p in contents), "本体 JAR に想定外のファイルがある")
                    classes = {p.removesuffix(".class").split("$")[0] + ".java" for p in contents}
                    require(classes == set(sources), "本体 JAR のクラスとソースの組が違う")
                elif suffix == "-sources.jar":
                    require(contents == set(sources), "ソース JAR のファイルの組が違う")
                    for member, original in sources.items():
                        require(jar.read(member) == original.read_bytes(), f"配布用の {member} が元と違う")
                else:
                    require(not contents, "文書 JAR に想定外のファイルがある")
        require(sorted(bundle.namelist()) == sorted(expected), "未署名の配布物のファイルの組が違う")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("bundle", type=Path)
    verify_java(parser.parse_args().bundle)
    print(f"dev.zumen:zumen:{VERSION} の配布物が一致した")
