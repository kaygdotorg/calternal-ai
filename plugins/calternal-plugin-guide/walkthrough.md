# Write a Calternal plugin

1. Build the SDK in a Calternal checkout with `npm run build:plugins`.
2. Run `calternal plugin new my-plugin --sdk-root /path/to/Calternal-AI`.
3. Open the generated manifest and worker. Keep the package ID stable. Change
   the operation schema and its handler together.
4. Use a generated view first. Request the narrowest broker calls and declare
   the risk of each operation in the manifest.
5. Run `npm install`, then `calternal plugin check` and `calternal plugin test`
   in the package. Tests run the actual bundled worker against fixture data.
6. With a `dev-plugin-drafts` build running in development mode and an enrolled
   machine, run `calternal plugin dev . --appdata "$APPDATA" --space "$SPACE_ID" --account "$ACCOUNT_ID"`.
   Wait for Ready and invoke the operation in that space. Change the view text,
   wait for Ready again, then invoke it again to verify the new result.
7. Run `calternal plugin bench` to compare fixture transport paths.
8. Show the manifest, operation risks and host calls to the administrator
   who will review and activate the package.
