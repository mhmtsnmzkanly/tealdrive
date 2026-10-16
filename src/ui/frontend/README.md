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
