#!/usr/bin/env python3
"""Exercise saved threads over the disposable server's authenticated HTTP API."""
import json
import subprocess
import sys
import time
import uuid
from contextlib import contextmanager
from getpass import getpass
from http.client import HTTPConnection, HTTPException
from pathlib import Path

ROOT = Path.home() / "calternal-e2e"
PORT = 34135
BROWSER_SESSION = None


@contextmanager
def request(path, body=None):
    token = BROWSER_SESSION
    if token is None:
        token = (ROOT / "appdata/dev-tokens/owner.token").read_text().strip()
    headers = {"Authorization": f"Bearer {token}", "Content-Type": "application/json"}
    data = None if body is None else json.dumps(body).encode()
    connection = HTTPConnection("127.0.0.1", PORT, timeout=25)
    try:
        connection.request("GET" if data is None else "POST", f"/api/v1/{path}", data, headers)
        with connection.getresponse() as response:
            if not 200 <= response.status < 300:
                raise RuntimeError(f"{path}: HTTP {response.status}")
            yield response
    finally:
        connection.close()


def call(path, body=None):
    with request(path, body) as response:
        return json.load(response)


def op(operation, **inputs):
    return call("operations", {
        "operation": operation, "request_id": str(uuid.uuid4()), "input": inputs,
    })["data"]


def stream(job_id):
    deadline = time.monotonic() + 120
    label = None
    with request(f"jobs/{job_id}/events") as response:
        for line in response:
            if time.monotonic() > deadline:
                raise TimeoutError("Job stream exceeded 120 seconds")
            if line.startswith(b"event: "):
                label = line[7:].decode().strip()
            elif line.startswith(b"data: "):
                yield label, json.loads(line[6:])
    if label != "end":
        raise RuntimeError("Job stream closed before an end frame")


def verify(condition, message):
    if not condition:
        raise RuntimeError(message)


def output(event, payload):
    if event == "chunk":
        data = payload["data"].get("event", {})
        if data.get("type") == "text_delta":
            return data["text"]
    return None


def require_image(mode):
    command = subprocess.check_output(
        ["systemctl", "--user", "show", "-p", "ExecStart", "--value",
         "calternal-e2e-machine.service"],
        text=True,
    )
    verify(f"localhost/calternal-e2e-{mode}:dev" in command,
           f"Start the machine with the {mode} image first")


def create(title):
    spaces = op("spaces.list")["spaces"]
    space, = (s["id"] for s in spaces if s["role"] == "owner" and s["name"] == "personal")
    thread = op("threads.create", space_id=space, title=title)["thread"]["id"]
    return space, thread


def browser_machine(account, mode):
    service = Path(__file__).with_name("service.sh")
    machine = subprocess.check_output([service, "browser-machine", account, mode], text=True).strip()
    deadline = time.monotonic() + 30
    while time.monotonic() < deadline:
        if any(m["id"] == machine and m["state"] == "online" for m in call("machines")["machines"]):
            return
        time.sleep(0.25)
    raise TimeoutError("Browser machine did not come online")


def after_owner_setup():
    global PORT, BROWSER_SESSION
    PORT = 34136
    BROWSER_SESSION = (getpass("Preview session cookie value: ") if sys.stdin.isatty()
                       else sys.stdin.readline()).strip()
    verify(BROWSER_SESSION and BROWSER_SESSION.isascii(), "Enter the browser's session cookie value")
    account = str(uuid.UUID(call("auth/me")["user"]["id"]))
    home = ROOT / "browser/machine/sandboxes/accounts" / account / "home"
    proof = home.parent / "fixture-verified"
    if not proof.is_file():
        verify(not any(home.iterdir()) if home.is_dir() else True,
               "Finish the fixture proof before placing provider sign-in files")
        browser_machine(account, "fixture")
        owned = [s["id"] for s in op("spaces.list")["spaces"] if s["role"] == "owner"]
        space = owned[0] if owned else op("spaces.create", name="Personal")["space"]["id"]
        title = "Morning machine fixture"
        thread = op("threads.create", space_id=space, title=title)["thread"]["id"]
        job = op("threads.send", id=thread, message="Verify the fixture machine")["job"]["id"]
        text = ""
        for event, payload in stream(job):
            text += output(event, payload) or ""
            if event == "end":
                verify(payload["job"]["state"] == "done", "Browser fixture turn did not finish")
        verify(text == "Hello 🌏\n", "Browser fixture turn emitted unexpected text")
        op("threads.archive", id=thread)
        proof.touch()
        print("Browser fixture turn streamed and completed", flush=True)
    browser_machine(account, "claude")
    print(f"Sandbox home  {home}")


def run_fixture():
    require_image("fixture")
    title = f"Fixture e2e {uuid.uuid4().hex[:12]}"
    space, thread = create(title)
    account = call("describe")["principal"]["id"]
    container = f"calternal-account-{account}-thread-{thread}"
    first = op("threads.send", id=thread, message="Fixture first turn")["job"]["id"]
    first_text = False
    for event, payload in stream(first):
        first_text |= output(event, payload) is not None
        if event == "end":
            verify(payload["job"]["state"] == "done", "Fixture first turn did not finish")
            session = payload["job"]["checkpoint"]["session_id"]
    verify(first_text and session, "Fixture first turn lacked streaming text or a session")
    print("Fixture first turn streamed and completed", flush=True)

    second = op("threads.send", id=thread, message="Fixture second turn hang")["job"]["id"]
    live = False
    resumed = False
    cancelled = False
    try:
        for event, payload in stream(second):
            if output(event, payload) is not None and not live:
                # The fixture holds here until Stop. Inspect only command arguments,
                # and expose only whether the actual provider was passed --resume.
                top = subprocess.run(
                    ["podman", "top", container, "args"], capture_output=True,
                    text=True, check=True, timeout=10,
                )
                resumed = f"--resume {session}" in top.stdout
                active = op("jobs.get", job_id=second)["job"]["state"] == "running"
                verify(active, "Fixture output was not observed while running")
                live = True
                op("threads.stop", id=thread)
            if event == "end":
                cancelled = payload["job"]["state"] == "cancelled"
    finally:
        if not cancelled:
            op("threads.stop", id=thread)
    verify(live and cancelled, "Fixture second turn did not stream then cancel")

    saved = ROOT / "userdata-ssd/spaces" / space / "AI/Chats" / f"{title}.md"
    markdown = saved.read_text()
    for expected in (thread, "Fixture first turn", "Fixture second turn hang", "Hello 🌏"):
        verify(expected in markdown, "Saved Markdown is missing expected fixture content")
    print(f"Fixture second turn streamed while running and cancelled; Markdown at {saved}")
    verify(resumed, "Second send started a fresh Claude session: --resume was absent")
    print("Fixture second turn resumed the first Claude session")


def run_real():
    start = time.monotonic()
    first_text = None
    text_prefix = ""
    text_size = 0
    state = "stream_failed"
    job = None
    try:
        require_image("claude")
        _, thread = create(f"Claude e2e {uuid.uuid4().hex[:12]}")
        # Take one attempt, then restore the defaults for later turns.
        settings = op("agent.settings")["retry_settings"]
        op("agent.settings", retry_settings={"provider_errors": False, "subscription_limits": False})
        try:
            start = time.monotonic()
            job = op("threads.send", id=thread, message="reply with the single word ready")["job"]["id"]
        finally:
            op("agent.settings", retry_settings=settings)
        for event, payload in stream(job):
            text = output(event, payload)
            if text is not None:
                if first_text is None:
                    first_text = round((time.monotonic() - start) * 1000)
                text_prefix = (text_prefix + text)[:128]
                text_size += len(text)
            if event == "end":
                state = payload["job"]["state"]
    except (OSError, HTTPException, RuntimeError, ValueError, KeyError, subprocess.CalledProcessError):
        if job is not None:
            try:
                op("threads.stop", id=thread)
            except (OSError, HTTPException, RuntimeError, ValueError, KeyError):
                state = "stop_failed"
    outcome = state
    if state == "done":
        outcome = "ready" if text_size <= 128 and text_prefix.strip() == "ready" else "unexpected_reply"
    print(json.dumps({
        "outcome": outcome,
        "first_text_ms": first_text,
        "elapsed_ms": round((time.monotonic() - start) * 1000),
    }))
    return outcome == "ready"


if __name__ == "__main__":
    if sys.argv[1:] in (["fixture"], ["after-owner-setup"]):
        try:
            if sys.argv[1] == "fixture":
                run_fixture()
            else:
                after_owner_setup()
        except (OSError, HTTPException, RuntimeError, ValueError, KeyError, subprocess.CalledProcessError) as error:
            sys.exit(str(error))
    elif sys.argv[1:] == ["claude"]:
        sys.exit(0 if run_real() else 1)
    else:
        sys.exit("Usage: prove.py fixture|claude|after-owner-setup")
