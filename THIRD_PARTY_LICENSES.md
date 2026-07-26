# Third-party licenses

Nyquist itself is MIT licensed (see [LICENSE](LICENSE)). This file covers third-party code
that was **copied into this repository**, which the licenses require us to reproduce.

Ordinary dependencies fetched by `npm` and `cargo` are not listed here — their licenses ship
with the packages themselves, and none of them are vendored into our source tree. Note the
project's own rule in `AGENTS.md`: any FFI/C dependency's license must be checked before
adding it, and a statically linked LGPL/GPL library would force the MIT decision to be
revisited.

---

## thinking-orbs

The "composing" loading indicator in `src/lib/thinkingOrb.ts` is a port of the `ribbon`
frame painter from [`thinking-orbs`](https://orbs.jakubantalik.com) v0.1.1 by Jakub Antalik.
The animation maths is upstream's; the canvas, timing and theme plumbing in
`src/lib/components/ThinkingOrb.svelte` is ours.

It was ported rather than installed because the package is a React component — it declares
`react`/`react-dom` peer dependencies and its module does a top-level `import from "react"`,
so bundling it would mean adding React and ReactDOM to a Svelte app to draw one 64px
indicator. See the module docs in `src/lib/thinkingOrb.ts` for details, including how the
port was verified against the original.

```
MIT License

Copyright (c) 2026 Jakub Antalik

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```
