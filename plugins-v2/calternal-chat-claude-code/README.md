# Claude Code chat harness

The revision 9 manifest declares the program ID
`calternal-provider-claude-code`. Its bundle-local executable is
`programs/calternal-provider-claude-code`; the wrapper delegates to the
existing adapter installed by
`plugins/calternal-claude-code/Containerfile`. This keeps the provider
protocol and Claude stream handling in one place.

The fake-provider acceptance image uses a script that echoes the supplied
prompt as a Claude stream-json text delta. It has no credentials and no
network access. A `hold:` prompt starts a child process only so the Podman
stop test can verify that the process family ends within two seconds.

On the dev VM, run `nix develop -c npm run build:plugins`, then
`nix develop -c plugins-v2/calternal-chat-claude-code/test/build-echo-image.sh`.
Run the `calternal-machine` `runs_harness` integration test with
`CALTERNAL_TEST_PROVIDER_IMAGE=localhost/calternal-chat-claude-code-fake:dev`
under the shared gate lock.

The harness container has `--network=none` in this slice. The approved
mediated path for reaching Anthropic remains a coordinator question because
the host proxy is not proven. No real Claude sign-in or provider binary is
used by the fake test.
