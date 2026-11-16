# Rust Download Manager website

Static download page built with Astro, Bun and Tailwind CSS.

```sh
cd website
bun install --frozen-lockfile
bun run dev
```

Run `bun run build` to generate `dist/`, or `bun run preview` to preview the build.

In the repository's Settings → Pages, select GitHub Actions as the source.
The Pages workflow publishes changes on `dev` and can also be run manually.
The site is configured for https://rdm.moicr.dev/.

Application buttons resolve assets from the latest GitHub release in the browser.
If the API is unavailable or an asset is missing, they open the latest release page.
Extension buttons download the ZIP assets from that release.

## Structure

- `src/pages/`: page composition.
- `src/layouts/`: document metadata and shared layout.
- `src/components/`: navigation, hero, Ferris, download cards and sections.
- `src/data/`: release links, platforms and asset patterns.
- `src/scripts/`: operating system detection and release resolution.
- `src/css/`: Tailwind theme, hero layout and Ferris animation.
- `public/images/`: local mascot and platform/browser logos, with attribution.

The Tailwind color tokens match `crates/app/src/theme/dark.rs`.
Ferris animation respects the visitor's reduced-motion preference.
