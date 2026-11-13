# Application fonts

Noto Sans SC and Noto Serif SC are distributed under SIL OFL 1.1. Copyright notices and complete license texts are stored alongside this file and are accessible in the application font settings.

The exact sizes and SHA256 checksums are recorded in `src/data/fontCatalog.json`. The full variable TTF files are included in `public/fonts` and packaged with the application. They load from local application assets and work offline. The interface uses the system font until the chosen bundled font is ready; reset recovery uses the system font.

Original sources:

- https://github.com/google/fonts/tree/a85815a42757630ce188fdad368c2dfc444d4773/ofl/notosanssc
- https://github.com/google/fonts/tree/8b0a1d0f5983c89bc2b93f1b5fb55f9e252744b5/ofl/notoserifsc

Font files contain the same glyphs and metrics on each platform. Antialiasing and rasterization are still performed by each platform's WebView.
