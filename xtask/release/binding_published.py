"""タグ公開の再実行では、配布先の同じ版の中身がすべて一致する場合だけ省く。"""

import argparse
import tarfile
import urllib.error
import urllib.request
import zipfile
from pathlib import Path

from dart_package import check_contents, dart_files, verify_dart
from package import VERSION, require
from verify_java import verify_java


def fetch(url):
    try:
        return urllib.request.urlopen(url, timeout=30)
    except urllib.error.HTTPError as error:
        if error.code != 404:
            raise
        error.close()
        return None


def maven_published(bundle):
    verify_java(bundle)
    available = []
    with zipfile.ZipFile(bundle) as archive:
        for name in archive.namelist():
            if not name.endswith((".jar", ".pom")):
                continue
            response = fetch("https://repo.maven.apache.org/maven2/" + name)
            available.append(response is not None)
            if response is not None:
                expected = archive.read(name)
                with response:
                    require(response.read(len(expected) + 1) == expected, f"公開済みの {name} がタグと違う")
    require(all(available) or not any(available), "Maven Central で一部のファイルだけが見つかった")
    return all(available)


def dart_published(bundle):
    expected = dart_files()
    verify_dart(bundle, expected)
    response = fetch(f"https://pub.dev/api/archives/zumen-{VERSION}.tar.gz")
    if response is None:
        return False
    with response, tarfile.open(fileobj=response, mode="r|gz") as archive:
        check_contents(archive, expected, published=True)
    return True


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("registry", choices=["maven", "dart"])
    parser.add_argument("bundle", type=Path)
    args = parser.parse_args()
    check = maven_published if args.registry == "maven" else dart_published
    print("true" if check(args.bundle) else "false")
