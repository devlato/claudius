# dmgbuild's settings for Claudius' disk image (scripts/package/macos.sh): the app beside a link to Applications, on
# a background that says to drag one onto the other (dmg-background.png and @2x, from scripts/icons.py, which draws
# the arrow between these two places).
#
#   dmgbuild -s packaging/macos/dmg-settings.py -D app=.../Claudius.app -D icon=.../claudius.icns \
#       -D background=packaging/macos/dmg-background.png "Claudius 0.3.0" claudius.dmg
import os.path

app = defines["app"]  # noqa: F821 (dmgbuild runs this file with `defines` set)
files = [app]
symlinks = {"Applications": "/Applications"}
icon = defines["icon"]  # noqa: F821
background = defines["background"]  # noqa: F821
format = "UDZO"
filesystem = "HFS+"

window_rect = ((200, 120), (640, 400))
default_view = "icon-view"
show_status_bar = False
show_tab_view = False
show_toolbar = False
show_pathbar = False
show_sidebar = False
include_icon_view_settings = True
arrange_by = None
icon_size = 128
text_size = 13
icon_locations = {os.path.basename(app): (170, 190), "Applications": (470, 190)}
