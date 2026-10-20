"""Exercise a remotely hosted Topcoat page in a real Tauri webview.

Start the hosted web app and tauri-driver first. Set TOPCOAT_APP_URL on the
DRIVER process, and use an isolated XDG_DATA_HOME for test-only local storage.
"""
import argparse
import base64
import json
from pathlib import Path
import time
import urllib.error
import urllib.request

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--driver-url", default="http://127.0.0.1:4455")
parser.add_argument("--server-url", default="http://127.0.0.1:3002")
parser.add_argument("--binary", type=Path)
parser.add_argument("--screenshot", type=Path)
parser.add_argument("--offline-marker", type=Path, help="Pause until this file exists, so the test runner can stop its web server.")
args = parser.parse_args()
workspace = Path(__file__).resolve().parents[1]
binary = (args.binary or workspace / "target/debug/accountability-desktop").resolve()


def request(method, path, payload=None):
    data = None if payload is None else json.dumps(payload).encode()
    req = urllib.request.Request(args.driver_url + path, data=data, method=method,
                                 headers={"Content-Type": "application/json"})
    try:
        with urllib.request.urlopen(req, timeout=45) as response:
            value = json.load(response)["value"]
    except urllib.error.HTTPError as error:
        raise RuntimeError(error.read().decode()) from error
    if isinstance(value, dict) and "error" in value:
        raise RuntimeError(value)
    return value


def remote(path):
    with urllib.request.urlopen(args.server_url + path, timeout=10) as response:
        return json.load(response)


web_note = remote("/api/hybrid/note")
cloud_count = remote("/api/hybrid/check-ins")["count"]
session = request("POST", "/session", {
    "capabilities": {"alwaysMatch": {"tauri:options": {"application": str(binary)}}}
})["sessionId"]
session_path = f"/session/{session}"


def execute(script):
    return request("POST", session_path + "/execute/sync", {"script": script, "args": []})


def wait_for(expression):
    deadline = time.monotonic() + 25
    while time.monotonic() < deadline:
        if execute("return " + expression):
            return
        time.sleep(0.1)
    raise AssertionError("Timed out: " + expression)


try:
    wait_for("document.querySelector('#reload-note') !== null")
    wait_for("document.readyState === 'complete'")
    execute("document.querySelector('#reload-note').click(); document.querySelector('#refresh-cloud').click();")
    wait_for("document.querySelector('#runtime-mode').textContent.includes('desktop Rust')")
    assert execute("return location.href") == "topcoat://localhost/hybrid"
    assert execute("return typeof window.__TAURI__") == "undefined"
    execute("window.__smokeErrors = []; addEventListener('error', e => __smokeErrors.push(e.message)); addEventListener('unhandledrejection', e => __smokeErrors.push(String(e.reason)));")
    wait_for("document.querySelector('#note-status').textContent.includes('Loaded by desktop Rust')")
    text = "Native note: stored by a local Rust procedure"
    execute("document.querySelector('#note-text').value = " + json.dumps(text) + "; document.querySelector('#note-text').dispatchEvent(new Event('input', {bubbles:true}))")
    execute("document.querySelector('#save-note').click()")
    wait_for("document.querySelector('#note-status').textContent.includes('Saved by desktop Rust')")
    assert remote("/api/hybrid/note") == web_note, "local saves must not modify the web server note"
    execute("document.querySelector('#inspect-host').click()")
    wait_for("document.querySelector('#host-execution').textContent === 'desktop Rust'")
    # WebDriver /title reads document.title. The Rust service reads the actual
    # native title with WebviewWindow::title() after setting it.
    title = execute("return document.querySelector('#host-title').textContent")
    assert title == "Accountability · Desktop Rust API", repr(title)
    assert execute("return document.querySelector('#host-version').textContent") == "0.0.1"
    local_file = Path(execute("return document.querySelector('#host-path').textContent"))
    assert local_file.name == "platform-note.json"
    assert json.loads(local_file.read_text())["text"] == text
    execute("document.querySelector('#cloud-check-in').click()")
    wait_for(f"document.querySelector('#cloud-count').textContent === '{cloud_count + 1}'")
    assert remote("/api/hybrid/check-ins")["count"] == cloud_count + 1
    assert execute("return window.__smokeErrors") == []
    execute("location.reload()")
    wait_for("document.readyState === 'complete' && document.querySelector('#reload-note') !== null")
    execute("document.querySelector('#reload-note').click()")
    wait_for("document.querySelector('#note-status')?.textContent.includes('Loaded by desktop Rust')")
    assert execute("return document.querySelector('#note-text').value") == text
    assert execute("return document.querySelector('#note-error').textContent") == ""
    if args.offline_marker:
        ready = args.offline_marker.with_suffix(".ready")
        ready.write_text("ready")
        print("READY: stop the test web server, then create " + str(args.offline_marker), flush=True)
        deadline = time.monotonic() + 90
        while not args.offline_marker.exists():
            if time.monotonic() > deadline:
                raise AssertionError("Offline test marker was not created")
            time.sleep(0.1)
        offline_text = text + " while the server is stopped"
        execute("document.querySelector('#note-text').value = " + json.dumps(offline_text) + "; document.querySelector('#note-text').dispatchEvent(new Event('input', {bubbles:true})); document.querySelector('#save-note').click()")
        deadline = time.monotonic() + 25
        while json.loads(local_file.read_text())["text"] != offline_text:
            if time.monotonic() > deadline:
                raise AssertionError("Local Rust save did not finish while the server was stopped")
            time.sleep(0.1)
        execute("document.querySelector('#cloud-check-in').click()")
        wait_for("document.querySelector('#cloud-error').textContent.length > 0")
        assert execute("return document.querySelector('#note-error').textContent") == ""
        execute("document.querySelector('#inspect-host').click()")
        wait_for("document.querySelector('#host-execution').textContent === 'desktop Rust'")
    if args.screenshot:
        args.screenshot.write_bytes(base64.b64decode(request("GET", session_path + "/screenshot")))
    print("PASS: hosted UI via Rust transport in real Tauri; no JS Tauri bridge; local persisted note; server note untouched; remote check-in; Rust app metadata, app-data path, and native window title.")
finally:
    request("DELETE", session_path)
