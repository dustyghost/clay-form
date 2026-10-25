"""Executed by GIMP 3's Python batch interpreter; config is supplied by Rust."""
from pathlib import Path
from gi.repository import Gimp, Gio

MARKER = "clay-form-uv-guide-v1"


def require(value, message):
    if not value:
        raise RuntimeError(message)
    return value


def procedure(name, **properties):
    proc = require(Gimp.get_pdb().lookup_procedure(name), "Missing GIMP procedure: " + name)
    settings = proc.create_config()
    settings.set_property("run-mode", Gimp.RunMode.NONINTERACTIVE)
    for key, value in properties.items():
        settings.set_property(key.replace("_", "-"), value)
    result = proc.run(settings)
    require(result.index(0) == Gimp.PDBStatusType.SUCCESS, name + " failed: " + str(result.index(0)))


def layers(items):
    for item in items:
        yield item
        if item.is_group():
            yield from layers(item.get_children())


def exchange():
    source = config["xcf"]
    image = require(Gimp.file_load(Gimp.RunMode.NONINTERACTIVE, Gio.File.new_for_path(source)),
                    "Cannot open the external working document")
    try:
        require(0 < image.get_width() <= 4096 and 0 < image.get_height() <= 4096,
                "GIMP image exceeds the 4096 × 4096 texture limit")
        if config["operation"] == "prepare-xcf":
            if config["float"]:
                require(image.get_precision() in (Gimp.Precision.FLOAT_LINEAR,
                        Gimp.Precision.FLOAT_NON_LINEAR, Gimp.Precision.FLOAT_PERCEPTUAL),
                        "Reader did not preserve 32-bit float pixels")
            require(Gimp.file_save(Gimp.RunMode.NONINTERACTIVE, image,
                    Gio.File.new_for_path(config["output"]), None), "Cannot save XCF working document")
            return
        guides = [layer for layer in layers(image.get_layers()) if layer.get_parasite(MARKER) or layer.get_name() == "Clay UV guide - excluded on reload"]
        require(guides, "UV guide is missing or was merged. Keep the original guide name and a separate guide layer, then save the working document again.")
        for guide in guides:
            require(not guide.is_group(), "Keep the UV guide as a separate layer, outside paint groups")
            guide.set_lock_content(False)
            guide.set_lock_position(False)
            require(image.remove_layer(guide), "Cannot exclude UV guide")
        require(image.get_layers(), "No paint layers remain in the GIMP document")
        # Do not silently recover textures from a reduced-precision working PSD.
        if config["float"]:
            require(image.get_precision() in (Gimp.Precision.FLOAT_LINEAR,
                    Gimp.Precision.FLOAT_NON_LINEAR, Gimp.Precision.FLOAT_PERCEPTUAL),
                    "Working document lost float precision. Relink a 32-bit EXR, use XCF with GIMP, or use an editor that saves float PSD; current texture kept.")
        else:
            require(image.get_precision() not in (Gimp.Precision.U8_LINEAR,
                    Gimp.Precision.U8_NON_LINEAR, Gimp.Precision.U8_PERCEPTUAL),
                    "Working document lost precision. Save a 16-bit or higher image; current texture kept.")
        options = dict(image=image, file=Gio.File.new_for_path(config["output"]))
        if not config["float"]:
            options["include_color_profile"] = True
        procedure("file-exr-export" if config["float"] else "file-png-export", **options)
    finally:
        image.delete()


try:
    exchange()
    result = {"ok": True}
except Exception as error:
    result = {"ok": False, "error": str(error)}
Path(config["result"]).write_text(json.dumps(result), encoding="utf-8")
