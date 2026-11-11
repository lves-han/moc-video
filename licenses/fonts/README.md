# Application fonts

Noto Sans SC and Noto Serif SC are distributed under SIL OFL 1.1. Copyright notices and complete license texts are stored alongside this file and are accessible in the application font settings.

The exact upstream commits, sizes and SHA256 checksums are pinned in `src/data/fontCatalog.json`. Full variable TTF files are downloaded from Google Fonts' repository through the native fixed-DNS HTTP gateway on first use, checked before saving into the local WebView IndexedDB database `moc-video-fonts`, and subsequently read without network requests. The database belongs to the application's local WebView profile on Windows, macOS and Linux. Deleting that profile also removes the fonts; a missing or damaged entry is downloaded again.

`public/fonts/bootstrap.woff2` is a UI-text subset of the pinned Noto Sans SC variable font (194704 bytes). It is provided for the first-download/retry screen and uncovered UI punctuation; complete fonts remain downloaded on demand. It retains upstream font names, copyright and licensing metadata. The upstream OFL reserves the name “Source”; this subset does not use that reserved name. Generation uses fonttools subset with `--flavor=woff2 --name-IDs=* --name-languages=*` and the UI source text plus ASCII as its character set. Fonttools/Brotli are development utilities, not application runtime dependencies.

Original sources:

- https://github.com/google/fonts/tree/a85815a42757630ce188fdad368c2dfc444d4773/ofl/notosanssc
- https://github.com/google/fonts/tree/8b0a1d0f5983c89bc2b93f1b5fb55f9e252744b5/ofl/notoserifsc

Font files contain the same glyphs and metrics on each platform. Antialiasing and rasterization are still performed by each platform's WebView.
