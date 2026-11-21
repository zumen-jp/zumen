"""既定の WebGPU 設定の製品ブラウザーで、合成シーンの画素と選択を照らす。"""
# /// script
# requires-python = ">=3.11"
# dependencies = ["selenium==4.53.0"]
# ///

from functools import partial
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
import json
from pathlib import Path
import platform
import sys
from threading import Thread

from selenium import webdriver
from selenium.webdriver.common.by import By
from selenium.webdriver.support.ui import WebDriverWait


def main(browser):
    root = Path(__file__).resolve().parents[5] / "target/browser-webgpu"
    if not (root / "index.html").is_file():
        raise SystemExit("先に desktop/vite.config.mjs で検証用のページをビルドしてください。")
    with ThreadingHTTPServer(("127.0.0.1", 24137), partial(SimpleHTTPRequestHandler, directory=root)) as server:
        Thread(target=server.serve_forever, daemon=True).start()
        try:
            with {"firefox": webdriver.Firefox, "safari": webdriver.Safari}[browser]() as driver:
                driver.get("http://127.0.0.1:24137/")
                output = WebDriverWait(driver, 90).until(lambda d: d.find_elements(By.CSS_SELECTOR, "#observed[data-complete]"))
                observations = json.loads(output[0].text)
                print(platform.platform(), browser, driver.capabilities["browserVersion"], observations, flush=True)
                return all(observations[name]["passed"] for name in ("drawing", "building"))
        finally:
            server.shutdown()


if __name__ == "__main__":
    if sys.argv[1:] not in (["firefox"], ["safari"]):
        raise SystemExit("check.py firefox | safari")
    sys.exit(0 if main(sys.argv[1]) else 1)
