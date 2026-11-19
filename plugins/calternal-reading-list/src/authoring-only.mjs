// The authoring proof has no worker effects. Fail closed if launched.
process.stderr.write("Reading List is an offline authoring example.\n");
process.exitCode = 1;
