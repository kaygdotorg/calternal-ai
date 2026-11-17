The worker seccomp allowlist is derived from Moby Profiles `seccomp/default.json`,
Apache-2.0, upstream commit `f862455628f78f84c937e967e4a03902fb1dee11` (2026-11-17):
https://github.com/moby/profiles/blob/f862455628f78f84c937e967e4a03902fb1dee11/seccomp/default.json

Changes: remove `socket`, `socketpair`, `socketcall`, `io_uring_setup`,
`io_uring_enter` and `io_uring_register` from every allow rule. All other rules,
architecture mappings and the default refusal action are retained. This blocks
socket creation, including loopback and Unix sockets, while keeping the default
syscall restrictions. JSON is a runtime-required OCI seccomp format, not userdata.

Upstream license: [Apache-2.0](worker-seccomp.LICENSE).
Licensed under the Apache License, Version 2.0 (the "License");
you may not use this file except in compliance with the License.
You may obtain a copy of the License at http://www.apache.org/licenses/LICENSE-2.0.
Unless required by applicable law or agreed to in writing, software distributed
under the License is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR
CONDITIONS OF ANY KIND, either express or implied. See the License for the
specific language governing permissions and limitations under the License.
