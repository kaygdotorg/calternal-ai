# Prepare the independent author trial

On a development VM, from the repository root:

```sh
npm run build -w @calternal/plugin-sdk
node docs/reviews/plugin-sdk-trial/prepare.mjs /absolute/path/to/new/trial
```

Hand the participant the resulting directory. It contains the guide, task,
installable offline SDK archive, build command, and artifact checker. The
SDK archive carries the compiled SDK, TypeScript, the validation dependencies
and the sealed contract files. Node 24, npm, and Python 3 must be available.

The participant follows `GUIDE.md` and runs `sh build-and-check.sh`. To
score existing artifacts without authored source, run `node check.mjs dist`
from the trial directory after installing the archive. PASS covers the
required declarations and does not exercise worker effects.
