# Third-party notices

Magic Circle Maker's application code is MIT licensed; that license does not replace the licenses of bundled fonts or dependencies.

## Bundled artwork fonts

Unmodified Cinzel, IM FELL English, and Inter are embedded in the executable under the SIL Open Font License 1.1. Copyright statements and complete terms are retained in `assets/fonts/*-OFL.txt` and in release archives. `assets/fonts/SOURCES.md` records their upstream sources and SHA-256 hashes.

## Rust dependencies

`docs/dependency-licenses.json` lists the locked dependency versions, declared licenses, and upstream repositories. `docs/licenses/` preserves license and notice files from the Windows dependency graph, including the font licenses for egui's fallback UI fonts. Dependencies with alternative licenses are used under their permissive MIT, Apache-2.0, BSD, ISC, Zlib, BSL, or Unlicense option where available; additional Unicode and font terms are retained. Dependencies used only for building may also appear in the notice collection.

The MPL-2.0 `option-ext` dependency is unmodified. Its complete published source files are provided in `docs/licenses/option-ext-0.2.0/source/`, together with the MPL notice. These source files remain under MPL-2.0; the application code remains MIT. Its upstream repository is https://github.com/soc/option-ext.

Rust standard library and compiler runtime components are distributed under their applicable MIT/Apache-2.0 licenses. Windows system components remain subject to Microsoft's terms.

## User imports

System and custom fonts, images, symbols, and SVG assets supplied by a user retain their original licenses. Loading a custom font embeds its bytes in a saved project; check permission before distributing that project. No personal-use-only DaFont download is bundled.
