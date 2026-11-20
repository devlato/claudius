# Themes

A theme is a look of your own, or one someone made and shared: the studio's colours, written in a small file. The
studio comes with two looks, **Dark** and **Light**, and wears any theme you add beside them.

## Wearing one

**?** > **Look** > **Add a theme…** chooses a theme file (a `.json`). The studio wears it at once and keeps it: in the app,
as a file in the `themes` folder of claudius's settings (`~/Library/Application Support/claudius/themes` on a Mac,
`~/.config/claudius/themes` on Linux, `%APPDATA%\claudius\themes` on Windows); in a browser, in the page's storage. The
themes you added are listed under **Look**, and in the look's menu at the right end of the transport bar, beside
Dark, Light and As the system is. The studio wears the theme you chose the next time it opens, too. A theme of the same
name as one already added takes its place.

## Making one

**?** > **Look** > **Copy this look as a theme** copies the look you wear now as a theme file, every colour in it, named
"Dark (mine)" or after the theme. Paste it into a file ending in `.json`, change its name and the colours you want,
and add it. A theme need not name every colour: those it leaves out are its base look's. So a theme can be as small as

```json
{
  "claudius_theme": 1,
  "name": "Ember",
  "base": "dark",
  "colors": { "accent": "#ff5500", "playhead": "#ffd0a0" }
}
```

- **`claudius_theme`** is the theme format, 1 today. Every later studio reads every earlier format.
- **`name`** is how the theme is listed (up to 60 characters).
- **`base`** is the look it is drawn on, `dark` or `light`: the colours it leaves out are that look's, and it decides
  how the tracks' and the sections' colours are shaded (at a clean middle shade on a light theme, as the light look
  draws them).
- **`colors`** holds the colours, each as `#rrggbb`, or `#rrggbbaa` to be see-through, and two numbers. A name the studio
  does not know (a colour a later studio added) is left out and said when the theme is added, never refused.

| colour | what it colours |
|---|---|
| `bg` | the window behind everything |
| `canvas` | what the arrangement draws the music on |
| `panel` | the panels |
| `raised`, `raised_hi` | what stands on a panel (a card, a menu), and the same under the pointer |
| `well` | what sits sunk in a panel: a fader's track, a meter, a knob's arc, the roll, a block of code |
| `field` | a field you type in |
| `cell`, `cell_alt` | an empty cell of a grid (a step that does not play), and every other beat's |
| `control`, `control_hi` | a control, and a control under the pointer |
| `line`, `line_soft` | the lines that divide, the strong ones and the soft ones |
| `text`, `text_dim`, `text_faint` | text, quieter text, and the quietest (keep it readable: at least 4.5:1 against the panels) |
| `accent` | the accent: what is chosen, the main button |
| `good`, `warn`, `bad` | the verdict's colours, and what went well, needs a look, or failed |
| `lcd`, `lcd_text`, `info` | the transport's display, its figures, and its colour off the display (the genre, the verdict's notes) |
| `playhead` | the playhead |
| `solo` | a track soloed |
| `strong` | the strongest ink, which a control under the pointer is drawn in |
| `edge` | the edge of a quiet control, a field above all (none on the dark look) |
| `faint_bg`, `hover_line`, `pressed` | what egui draws itself: the faintest fill, the outline under the pointer, a control pressed |
| `lcd_hold` | how much ink the display's quieter figures keep, 0 to 1 |
| `shadow` | how dark the shadows are, 0 to 255 |

The tracks' and the sections' colours (a kick's, a chorus's) are the studio's in every theme, so that the same
instrument reads the same everywhere.

The studio's dark look, as a theme file, every colour named:

```json
{
  "claudius_theme": 1,
  "name": "Dark",
  "base": "dark",
  "colors": {
    "bg": "#101217",
    "canvas": "#101217",
    "panel": "#171a20",
    "raised": "#20242c",
    "raised_hi": "#2a2f39",
    "well": "#0b0d11",
    "field": "#0b0d11",
    "cell": "#171a20",
    "cell_alt": "#1f232b",
    "control": "#272c36",
    "control_hi": "#323844",
    "line": "#2e333e",
    "line_soft": "#1f232b",
    "text": "#dde1e8",
    "text_dim": "#8f97a4",
    "text_faint": "#868d9b",
    "accent": "#ffa348",
    "good": "#54d486",
    "warn": "#f6c04a",
    "bad": "#f05e56",
    "lcd": "#090f11",
    "lcd_text": "#84e8ff",
    "info": "#84e8ff",
    "playhead": "#ffecd6",
    "solo": "#ffd440",
    "strong": "#ffffff",
    "edge": "#00000000",
    "faint_bg": "#15181e",
    "hover_line": "#424958",
    "pressed": "#3c4351",
    "lcd_hold": 0.0,
    "shadow": 150
  }
}
```
