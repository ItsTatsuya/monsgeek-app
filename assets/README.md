# Bundled UI assets

The interface is set in Inter, with its tabular figures for numbers so live values don't
shift as they change. JetBrains Mono is the GPUI Kit monospace font. Fonts are embedded in the
executable, so the UI does not depend on installed fonts or a network connection.

- `fonts/Inter.ttf`: Inter variable font from the
  [Google Fonts Inter directory](https://github.com/google/fonts/tree/main/ofl/inter).
  License: `fonts/Inter-OFL.txt`.
- `fonts/JetBrainsMono-Medium.ttf`: JetBrains Mono Medium from
  [JetBrains Mono v2.304](https://github.com/JetBrains/JetBrainsMono/tree/v2.304).
  License: `fonts/JetBrainsMono-OFL.txt`.
- `icons/*.svg`: sidebar icons, and `icons/logo.svg`, the logo mark.
- `icon/app.ico`, `icon/logo.png`: the exe icon and a preview. `icon/make_logo.py` generates
  these and `icons/logo.svg` from one geometry; edit the script, not the files.
- `monsgeek.json`: the 52 supported models (id, name, key matrix), embedded in the executable.

The app uses a single monochrome colour scheme.
