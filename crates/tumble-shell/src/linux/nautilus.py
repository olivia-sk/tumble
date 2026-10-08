# Tumble's "Convert to" submenu for GNOME Files (Nautilus), loaded by
# nautilus-python. Written by `tumble menu install` and removed by
# `tumble menu uninstall`; edits here are lost on the next install.
#
# It asks Tumble which formats the selected files can become, so the menu
# matches the one on Windows and in the other file managers.

import json
import subprocess

from gi import require_version

try:
    require_version("Nautilus", "4.0")
except ValueError:
    require_version("Nautilus", "3.0")
from gi.repository import GObject, Nautilus  # noqa: E402

TUMBLE = @TUMBLE@


def common_targets(paths):
    try:
        out = subprocess.run(
            [TUMBLE, "targets", "--menu", "--json", "--", *paths],
            capture_output=True,
            timeout=5,
            check=False,
        )
        return json.loads(out.stdout or b"[]")
    except (OSError, ValueError, subprocess.SubprocessError):
        return []


class TumbleMenu(GObject.GObject, Nautilus.MenuProvider):
    def get_file_items(self, *args):
        # Nautilus 4 passes (files), Nautilus 3 passes (window, files).
        files = args[-1]
        paths = []
        for f in files:
            if f.get_uri_scheme() != "file" or f.is_directory():
                return []
            location = f.get_location()
            path = location.get_path() if location else None
            if not path:
                return []
            paths.append(path)
        if not paths:
            return []
        targets = common_targets(paths)
        if not targets:
            return []
        top = Nautilus.MenuItem(name="Tumble::convert", label="Convert to")
        submenu = Nautilus.Menu()
        top.set_submenu(submenu)
        for t in targets:
            item = Nautilus.MenuItem(name="Tumble::to-" + t["id"], label=t["name"])
            item.connect("activate", self.convert, t["id"], paths)
            submenu.append_item(item)
        return [top]

    def convert(self, _item, target, paths):
        subprocess.Popen(
            [TUMBLE, "convert", "--to", target, "--", *paths],
            stdin=subprocess.DEVNULL,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            start_new_session=True,
        )
