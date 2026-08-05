# Animated icons

Vendored from the [Moving Icons](https://movingicons.dev) registry
(`https://movingicons.dev/r/<name>.json`), by `jis3r <jis3r@protonmail.com>`.

## Why they are vendored rather than installed

The registry is published in shadcn-svelte format, and
`npx shadcn-svelte@latest add …` wants to initialise shadcn-svelte — which
means Tailwind, a `components.json`, and a second styling system alongside the
design tokens in `$lib/styles/tokens.css`.

The components themselves need none of that: each is a self-contained Svelte 5
file with no `registryDependencies` and no Tailwind classes (`class` is just
forwarded to the wrapper). So they are copied in directly and the project stays
on one styling system.

## Local modification

One change from upstream, applied to every file: the wrapper `<div>` is
`inline-flex` instead of `inline-block`, so it adds no line-height gap when it
sits inside a flex button row. Everything else is verbatim.

## Adding another icon

Icons are named after their Lucide equivalents. Not every Lucide icon exists in
the registry — `moon`, `network` and `files` did not at the time of writing, so
those usages stay on `lucide-svelte`.

```sh
node -e '
const name = process.argv[1];
const r = await fetch(`https://movingicons.dev/r/${name}.json`, { redirect: "follow" });
require("fs").writeFileSync(`${name}.svelte`, (await r.json()).files[0].content);
' <icon-name>
```

Then apply the `inline-flex` change above and add an export to `index.ts`.

## Usage

Same props as the Lucide components for the cases margin uses (`size`), plus
`animate` to drive the animation externally instead of on hover:

```svelte
import {Search} from "$lib/components/movingicons";

<Search size={20} />
```

Reserve them for interactive controls. A status indicator that animates when
the pointer happens to cross it is noise, not feedback.
