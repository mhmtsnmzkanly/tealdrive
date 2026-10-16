# TealDrive web UI

React + TypeScript + Tailwind single page application served by the TealDrive
server as a Root-approved UI package. It talks to the server only through the
public SDK at `/sdk/v1/tealdrive.js`.

```sh
npm ci
npm run lint
npm test
npm run build        # writes dist/ and dist/manifest.json
```

Install the built package with the Wizard instead of pointing the server at
this directory:

```sh
sudo tealdrive-wizard ui install --bundle src/ui/frontend/dist
```

The Wizard reads the bundle through trusted paths, so build into a directory
owned by root that is not world-writable (a `/tmp` path is refused).

## Languages

UI text goes through `t()` from `src/i18n`. English strings are the keys and
`src/i18n/tr.ts` holds the Turkish text; placeholders such as `{name}` must be
kept. The language follows the browser and can be switched from the login
screen or the user menu. `npm test` fails when a string has no translation, a
translation changes its placeholders, or the dictionary has unused entries.

## Tests

`npm test` runs pure mapping tests and adapter tests against a fake SDK
(`src/sdk/client.test.ts`); no server or browser is needed.
