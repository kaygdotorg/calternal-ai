"""Loopback HTTPS OIDC issuer and SMTP sink for the live account tests."""

import base64
import hashlib
import http.server
import json
import pathlib
import secrets
import socketserver
import ssl
import subprocess
import sys
import time
import urllib.parse

root = pathlib.Path(sys.argv[2])
mode = sys.argv[1]


def openssl(*args, **kwargs):
    return subprocess.check_output(
        ["openssl", *args], cwd=root, stderr=subprocess.DEVNULL, **kwargs
    )


openssl("req", "-new", "-x509", "-newkey", "rsa:2048", "-nodes",
        "-keyout", "ca.key", "-out", "ca.pem", "-subj", "/CN=Account test CA", "-days", "1")
openssl("req", "-new", "-newkey", "rsa:2048", "-nodes",
        "-keyout", "leaf.key", "-out", "leaf.csr", "-subj", "/CN=localhost")
(root / "extensions").write_text("subjectAltName=DNS:localhost\nextendedKeyUsage=serverAuth\n")
openssl("x509", "-req", "-in", "leaf.csr", "-CA", "ca.pem", "-CAkey", "ca.key",
        "-CAcreateserial", "-out", "leaf.pem", "-days", "1", "-extfile", "extensions")
context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
context.load_cert_chain(root / "leaf.pem", root / "leaf.key")


def publish(name, value):
    temp = root / (name + ".tmp")
    temp.write_text(json.dumps(value))
    temp.replace(root / name)


class Smtp(socketserver.StreamRequestHandler):
    def handle(self):
        def reply(line):
            self.wfile.write(line + b"\r\n")

        def read():
            return self.rfile.readline().rstrip(b"\r\n")

        if not self.server.starttls:
            self.upgrade()
        count = self.server.count
        reply(b"220 localhost Calternal test SMTP")
        authenticated = False
        while line := read():
            verb, _, arg = line.partition(b" ")
            if verb == b"EHLO":
                reply(b"250-localhost")
                if self.server.starttls and not isinstance(self.connection, ssl.SSLSocket):
                    reply(b"250 STARTTLS")
                else:
                    reply(b"250 AUTH PLAIN")
            elif verb == b"STARTTLS" and self.server.starttls:
                reply(b"220 Ready for TLS")
                self.upgrade()
            elif verb == b"AUTH":
                authenticated = (
                    isinstance(self.connection, ssl.SSLSocket)
                    and arg.startswith(b"PLAIN ")
                    and base64.b64decode(arg.split()[1]) == b"\x00fixture\x00local-test-password"
                )
                reply(b"235 Authenticated" if authenticated else b"535 Authentication failed")
            elif verb == b"QUIT":
                reply(b"221 Goodbye")
                return
            elif not authenticated:
                reply(b"530 Authentication required")
            elif verb in (b"MAIL", b"NOOP", b"RSET"):
                reply(b"250 OK")
            elif verb == b"RCPT":
                recipient = arg.decode()
                if "reject@" in recipient:
                    reply(b"550 Recipient refused")
                else:
                    reply(b"250 OK")
            elif verb == b"DATA":
                reply(b"354 Send message")
                lines = []
                while True:
                    data = self.rfile.readline()
                    if not data:
                        return
                    if data == b".\r\n":
                        break
                    lines.append(data.decode().rstrip("\r\n"))
                self.server.count = count + 1
                publish(f"mail-{count}.json", {"recipient": recipient, "body": "\n".join(lines)})
                count += 1
                reply(b"250 Accepted")
            else:
                reply(b"500 Unsupported command")

    def upgrade(self):
        self.rfile.close()
        self.wfile.close()
        self.connection = context.wrap_socket(self.connection, server_side=True)
        self.rfile = self.connection.makefile("rb")
        self.wfile = self.connection.makefile("wb", buffering=0)


def b64(value):
    return base64.urlsafe_b64encode(value).rstrip(b"=").decode()


def jwt(claims):
    parts = [
        b64(json.dumps({"alg": "RS256", "kid": "fixture"}).encode()),
        b64(json.dumps(claims).encode()),
    ]
    signed = ".".join(parts)
    signature = openssl("dgst", "-sha256", "-sign", "leaf.key", input=signed.encode())
    return signed + "." + b64(signature)


class Oidc(http.server.BaseHTTPRequestHandler):
    def log_message(self, *_args):
        pass

    def respond(self, status, value):
        data = json.dumps(value).encode()
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def do_GET(self):
        parsed = urllib.parse.urlsplit(self.path)
        if parsed.path == "/.well-known/openid-configuration":
            self.respond(200, {
                "issuer": issuer,
                "authorization_endpoint": issuer + "/authorize",
                "token_endpoint": issuer + "/token",
                "jwks_uri": issuer + "/keys",
                "response_types_supported": ["code"],
                "subject_types_supported": ["public"],
                "id_token_signing_alg_values_supported": ["RS256"],
                "code_challenge_methods_supported": ["S256"],
            })
        elif parsed.path == "/keys":
            self.respond(200, {"keys": [{
                "kty": "RSA", "kid": "fixture", "alg": "RS256", "use": "sig",
                "n": modulus, "e": "AQAB",
            }]})
        elif parsed.path == "/authorize":
            query = dict(urllib.parse.parse_qsl(parsed.query))
            if (query.get("client_id") != "fixture-client" or query.get("response_type") != "code"
                    or query.get("code_challenge_method") != "S256"
                    or not all(query.get(k) for k in ("nonce", "state", "code_challenge"))
                    or "openid" not in query.get("scope", "").split()):
                return self.respond(400, {"error": "invalid_request"})
            code = secrets.token_urlsafe()
            self.server.codes[code] = query
            location = query["redirect_uri"] + "?" + urllib.parse.urlencode({
                "code": code, "state": query["state"],
            })
            self.send_response(302)
            self.send_header("Location", location)
            self.end_headers()
        else:
            self.respond(404, {})

    def do_POST(self):
        if self.path != "/token":
            return self.respond(404, {})
        data = self.rfile.read(int(self.headers["Content-Length"]))
        params = dict(urllib.parse.parse_qsl(data.decode()))
        query = self.server.codes.pop(params.get("code"), None)
        if (query is None or self.headers.get("Authorization") != "Basic "
                + base64.b64encode(b"fixture-client:local-client-secret").decode()
                or params.get("redirect_uri") != query["redirect_uri"]
                or b64(hashlib.sha256(params.get("code_verifier", "").encode()).digest())
                != query["code_challenge"] or params.get("grant_type") != "authorization_code"):
            return self.respond(400, {"error": "invalid_grant"})
        case = query.get("test_case")
        now = int(time.time())
        claims = {
            "iss": issuer, "sub": "fixture-subject", "aud": query["client_id"],
            "nonce": query["nonce"], "iat": now, "exp": now + 300,
            "email": "owner@example.test", "email_verified": True,
        }
        if case != "missing-auth" and query.get("prompt") == "login" and query.get("max_age") == "0":
            claims["auth_time"] = now
        if case == "old-auth":
            claims["auth_time"] = now - 300
        if case == "wrong-nonce":
            claims["nonce"] = "unexpected-nonce"
        if case == "wrong-audience":
            claims["aud"] = "different-client"
        token = jwt(claims)
        if case == "bad-signature":
            header, payload, signature = token.split(".")
            signature = ("A" if signature[0] != "A" else "B") + signature[1:]
            token = ".".join((header, payload, signature))
        self.respond(200, {"access_token": "fixture-access", "token_type": "Bearer",
                           "id_token": token})


if mode == "oidc":
    modulus = b64(bytes.fromhex(
        openssl("rsa", "-in", "leaf.key", "-noout", "-modulus").decode().strip().split("=")[1]
    ))
    server = http.server.HTTPServer(("127.0.0.1", 0), Oidc)
    issuer = f"https://localhost:{server.server_port}"
    server.codes = {}
    server.socket = context.wrap_socket(server.socket, server_side=True)
else:
    server = socketserver.TCPServer(("127.0.0.1", 0), Smtp)
    server.starttls = mode == "starttls"
    server.count = 0
publish("ready.json", server.server_address[1])
server.serve_forever()
