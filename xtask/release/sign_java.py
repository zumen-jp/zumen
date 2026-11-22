"""検査済みの Maven バンドルの POM と JAR を、同じバイト列のまま署名する。"""

import argparse
import os
import subprocess
import tempfile
import zipfile
from pathlib import Path

from verify_java import verify_java


def sign_java(bundle, output, key):
    if bundle.resolve() == output.resolve():
        raise ValueError("未署名の ZIP と出力は別のファイルを指定してください")
    verify_java(bundle)
    with tempfile.TemporaryDirectory(dir=output.parent) as temporary:
        staging = Path(temporary)
        signed = staging / "signed.zip"
        with zipfile.ZipFile(bundle) as source, zipfile.ZipFile(signed, "w") as dest:
            for member in source.infolist():
                data = source.read(member)
                dest.writestr(member, data)
                if not member.filename.endswith((".jar", ".pom")):
                    continue
                payload = staging / Path(member.filename).name
                payload.write_bytes(data)
                command = ["gpg", "--armor", "--detach-sign", "--output", "-"]
                if key:
                    command.extend(["--local-user", key])
                phrase = os.environ.get("MAVEN_GPG_PASSPHRASE")
                if phrase:
                    command.extend(["--batch", "--pinentry-mode", "loopback", "--passphrase-fd", "0"])
                signature = subprocess.check_output(
                    [*command, str(payload)], input=(phrase + "\n").encode() if phrase else None
                )
                subprocess.run(["gpg", "--batch", "--verify", "-", str(payload)], input=signature,
                               stderr=subprocess.DEVNULL, check=True)
                dest.writestr(member.filename + ".asc", signature)
        signed.replace(output)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("bundle", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--key", help="署名鍵のフィンガープリント（省略時は GnuPG の既定）")
    args = parser.parse_args()
    sign_java(args.bundle, args.output, args.key)
