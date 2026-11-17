#!/usr/bin/env python3
"""Exercise saved threads over the disposable server's authenticated HTTP API."""
import json
import subprocess
import sys
import time
import uuid
from contextlib import contextmanager
from http.client import HTTPConnection, HTTPException
from pathlib import Path

ROOT = Path.home() / "calternal-e2e"
PORT = 34135


@contextmanager
def request(path, body=None):
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
    personal = [s["id"] for s in spaces if s["role"] == "owner" and s["personal"]]
    verify(len(personal) == 1, "Expected one server-created personal space for this account")
    space = personal[0]
    thread = op("threads.create", space_id=space, title=title)["thread"]["id"]
    return space, thread


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


def real_turn(thread, prompt, expected):
    start = time.monotonic()
    first_text = None
    text_prefix = ""
    text_size = 0
    state = "stream_failed"
    job = session = None
    try:
        # Take one attempt, then restore the defaults for later turns.
        settings = op("agent.settings")["retry_settings"]
        op("agent.settings", retry_settings={"provider_errors": False, "subscription_limits": False})
        try:
            start = time.monotonic()
            job = op("threads.send", id=thread, message=prompt)["job"]["id"]
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
                session = (payload["job"].get("checkpoint") or {}).get("session_id")
    except (OSError, HTTPException, RuntimeError, ValueError, KeyError, subprocess.CalledProcessError):
        if job is not None:
            try:
                op("threads.stop", id=thread)
            except (OSError, HTTPException, RuntimeError, ValueError, KeyError):
                state = "stop_failed"
    outcome = state
    if state == "done":
        outcome = "matched" if text_size <= 128 and text_prefix.strip() == expected else "unexpected_reply"
    return {
        "outcome": outcome,
        "first_text_ms": first_text,
        "elapsed_ms": round((time.monotonic() - start) * 1000),
    }, session


def run_real(continuation=False):
    try:
        require_image("claude")
        _, thread = create(f"Claude e2e {uuid.uuid4().hex[:12]}")
    except (OSError, HTTPException, RuntimeError, ValueError, KeyError, subprocess.CalledProcessError):
        print(json.dumps({"outcome": "setup_failed"}))
        return False

    word = uuid.uuid4().hex[:12]
    prompt = (f"Remember this code for my next message: {word}. Reply with the single word ready."
              if continuation else "reply with the single word ready")
    first, session = real_turn(thread, prompt, "ready")
    if first["outcome"] != "matched" or not continuation:
        if first["outcome"] == "matched":
            first["outcome"] = "ready"
        print(json.dumps(first))
        return first["outcome"] == "ready"

    second, next_session = real_turn(
        thread, "Reply with only the code I asked you to remember.", word,
    )
    resumed = bool(session) and session == next_session
    first["outcome"] = "ready"
    if second["outcome"] == "matched":
        second["outcome"] = "continued" if resumed else "session_changed"
    print(json.dumps({"first_turn": first, "second_turn": second}))
    return second["outcome"] == "continued"


if __name__ == "__main__":
    if sys.argv[1:] == ["fixture"]:
        try:
            run_fixture()
        except (OSError, HTTPException, RuntimeError, ValueError, KeyError, subprocess.CalledProcessError) as error:
            sys.exit(str(error))
    elif sys.argv[1:] in (["claude"], ["claude-continuation"]):
        sys.exit(0 if run_real(continuation=sys.argv[1] == "claude-continuation") else 1)
    else:
        sys.exit("Usage: prove.py fixture|claude|claude-continuation")
