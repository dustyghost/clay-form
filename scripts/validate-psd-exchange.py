"""GIMP 3 fixture used by the ignored layered PSD round-trip test.

The Rust test injects config (source PSD, operation, result path). This operates
only on the temporary working PSD that the test generated.
"""
from pathlib import Path
from gi.repository import Gimp, Gio, Gegl

try:
    file = Gio.File.new_for_path(config["xcf"])
    image = Gimp.file_load(Gimp.RunMode.NONINTERACTIVE, file)
    assert image
    guide, paint = image.get_layers()
    assert guide.get_name() == "Clay UV guide - excluded on reload"
    assert guide.get_visible()
    assert image.get_selected_layers() == [paint], "Open on the editable paint layer"

    def export(name, path):
        proc = Gimp.get_pdb().lookup_procedure(name)
        settings = proc.create_config()
        settings.set_property("run-mode", Gimp.RunMode.NONINTERACTIVE)
        settings.set_property("image", image)
        settings.set_property("file", Gio.File.new_for_path(path))
        assert proc.run(settings).index(0) == Gimp.PDBStatusType.SUCCESS

    if config["edit"] == "paint":
        assert Gimp.context_set_foreground(Gegl.Color.new("rgba(0.8, 0.1, 0.2, 1.0)"))
        assert paint.fill(Gimp.FillType.FOREGROUND)
        # Exercise recursive guide exclusion after moving it into a group.
        group = Gimp.GroupLayer.new(image, "References")
        assert image.insert_layer(group, None, 0)
        guide.set_lock_position(False)
        assert image.reorder_item(guide, group, 0)
        export("file-png-export", "/tmp/clay-psd-guide-preview.png")
    elif config["edit"] == "remove-guide":
        guide.set_lock_content(False)
        guide.set_lock_position(False)
        assert image.remove_layer(guide)
    # GIMP's exporter reduces float to 16-bit; the Rust test verifies rejection.
    export("file-psd-export", config["xcf"])
    image.delete()
    result = {"ok": True}
except Exception as error:
    result = {"ok": False, "error": str(error)}
Path(config["result"]).write_text(json.dumps(result), encoding="utf-8")
