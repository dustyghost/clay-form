use super::*;

// Controls outside Action have explicit stable destinations instead of invented commands.
pub(super) const SETTINGS: &[(&str, &str, Destination, &str)] = &[
    (
        "Roughness",
        "material surface gloss shine",
        Destination::Menu(BuiltinMenu::Materials),
        "Change the selected material's roughness. Linked painted surfaces update with the shared material.",
    ),
    (
        "Metallic",
        "material metal reflectivity",
        Destination::Menu(BuiltinMenu::Materials),
        "Change how metallic the shared material appears.",
    ),
    (
        "IOR",
        "refraction index material surface options",
        Destination::Menu(BuiltinMenu::Materials),
        "Adjust the material's index of refraction in Surface options.",
    ),
    (
        "Specular",
        "reflection material surface options",
        Destination::Menu(BuiltinMenu::Materials),
        "Adjust the shared material's specular reflection.",
    ),
    (
        "Emission strength",
        "glow light material surface options",
        Destination::Menu(BuiltinMenu::Materials),
        "Adjust the strength of the material's emitted colour.",
    ),
    (
        "Subsurface scattering",
        "skin wax translucent material",
        Destination::Menu(BuiltinMenu::Materials),
        "Adjust how light scatters beneath the material's surface.",
    ),
    (
        "Pressure curve",
        "brush shape pen stylus sensitivity",
        Destination::Panel(Tool::Brush),
        "Adjust how pen pressure changes the active brush.",
    ),
    (
        "Pressure to strength",
        "brush pressure intensity",
        Destination::Panel(Tool::Brush),
        "Control whether pen pressure changes stroke strength.",
    ),
    (
        "Pressure to radius",
        "brush pressure size diameter",
        Destination::Panel(Tool::Brush),
        "Control whether pen pressure changes brush size.",
    ),
    (
        "Alpha rotation",
        "brush shape stamp angle",
        Destination::Panel(Tool::Brush),
        "Rotate the alpha used by the current brush.",
    ),
    (
        "Trailing distance",
        "stroke stabilisation stabilization delay",
        Destination::Panel(Tool::Brush),
        "Adjust the trailing distance used by stroke stabilisation.",
    ),
    (
        "Field of view",
        "fov perspective lens camera",
        Destination::Menu(BuiltinMenu::View),
        "Adjust perspective camera field of view.",
    ),
    (
        "Pinch to zoom",
        "touch fingers gesture",
        Destination::Viewport,
        "Enable pinch zoom for touch navigation.",
    ),
    (
        "Two-finger twist to roll view",
        "touch rotation gesture horizon",
        Destination::Viewport,
        "Enable two-finger twist to roll the camera.",
    ),
    (
        "Trackpad scrolling",
        "touch wheel pan zoom",
        Destination::Viewport,
        "Choose how trackpad scrolling moves the camera.",
    ),
    (
        "Drag sensitivity",
        "touch trackpad speed",
        Destination::Viewport,
        "Adjust touch drag navigation speed.",
    ),
    (
        "Pinch sensitivity",
        "touch trackpad speed zoom",
        Destination::Viewport,
        "Adjust pinch zoom speed.",
    ),
    (
        "Twist sensitivity",
        "touch trackpad speed roll",
        Destination::Viewport,
        "Adjust the speed of camera roll gestures.",
    ),
    (
        "Mesh capacity",
        "maximum vertices triangles limits memory performance",
        Destination::Capacity,
        "Set per-mesh geometry limits. Saved automatically; changes require a restart.",
    ),
    (
        "Maximum vertices",
        "mesh capacity limit",
        Destination::Capacity,
        "Set the maximum number of vertices allowed in one mesh.",
    ),
    (
        "Maximum triangles",
        "mesh capacity limit quads",
        Destination::Capacity,
        "Set the maximum number of triangles allowed in one mesh. Quads count as two.",
    ),
    (
        "Viewport controls",
        "navigation roll pan orbit zoom buttons",
        Destination::Viewport,
        "Add, place, restyle or restore controls inside the viewport.",
    ),
    (
        "Touch navigation",
        "trackpad gesture scroll pinch drag",
        Destination::Viewport,
        "Choose what touch and trackpad gestures do.",
    ),
    (
        "Workspace appearance",
        "theme color colour light dark warm contrast",
        Destination::Workspace,
        "Adjust the workspace colours and visual appearance.",
    ),
    (
        "Panel placement",
        "dock float hide show hidden restore",
        Destination::Workspace,
        "Move panels between sidebars, floating windows and hidden placement.",
    ),
    (
        "Saved workspaces",
        "layout save load switch named",
        Destination::Workspace,
        "Save or load a named arrangement of panels, menus and viewport controls.",
    ),
    (
        "Custom menus and shelves",
        "favourite favorite brush toolbar buttons",
        Destination::Workspace,
        "Build personal menus and shelves from the shared action catalogue.",
    ),
    (
        "Recover workspace",
        "reset restore hidden offscreen lost panel default",
        Destination::Workspace,
        "Recover visible panels or restore the default layout without clearing shortcuts.",
    ),
    (
        "Keyboard shortcuts",
        "hotkeys keymap bindings assign conflict",
        Destination::Keys,
        "Find and change shortcuts. Bindings are shared between workspaces.",
    ),
    (
        "Temporary mask hold",
        "modifier masking ctrl shift alt",
        Destination::Keys,
        "Choose the key or button that temporarily borrows masking tools.",
    ),
    (
        "Temporary visibility hold",
        "modifier hide isolate",
        Destination::Keys,
        "Choose the key or button for temporary visibility selection.",
    ),
    (
        "Smooth modifier",
        "shift smooth brush hold",
        Destination::Keys,
        "Choose the modifier used to borrow a Smooth brush.",
    ),
    (
        "Reverse brush modifier",
        "alt invert subtract hold",
        Destination::Keys,
        "Choose the modifier that reverses the current brush effect.",
    ),
    (
        "Recovery snapshots",
        "autosave backup restore crash interval",
        Destination::Recovery,
        "Configure recovery snapshots and inspect saved recovery files.",
    ),
    (
        "Tablet pressure",
        "pen stylus sensitivity navigation",
        Destination::Panel(Tool::Tablet),
        "Inspect pen pressure and navigation gestures.",
    ),
    (
        "Brush falloff",
        "softness hardness radius shape pressure curve",
        Destination::Panel(Tool::Brush),
        "Adjust the shape and pressure response of the active brush.",
    ),
    (
        "Stroke spacing",
        "distance samples brush stroke settings",
        Destination::Panel(Tool::Brush),
        "Adjust how brush samples are spaced along a stroke.",
    ),
    (
        "Stroke stabilisation",
        "stabilization lazy mouse smoothing jitter",
        Destination::Panel(Tool::Brush),
        "Smooth pointer movement for more controlled brush strokes.",
    ),
    (
        "Alpha library",
        "stamp image texture pattern import",
        Destination::Panel(Tool::Alpha),
        "Browse built-in and project alpha images for brush stamps and stencils.",
    ),
    (
        "Primitive dimensions",
        "sphere cube cylinder cone torus major base size radius",
        Destination::Menu(BuiltinMenu::Primitive),
        "Set primitive dimensions before converting it to editable geometry.",
    ),
    (
        "Slice options",
        "cut trim keep both fill cap plane",
        Destination::Menu(BuiltinMenu::Slice),
        "Choose which side of a slice to keep and how to cap the cut.",
    ),
    (
        "Symmetry axes",
        "mirror x y z radial centre center",
        Destination::Menu(BuiltinMenu::Symmetry),
        "Choose the axes and centre used for symmetric sculpting.",
    ),
    (
        "Camera projection",
        "perspective orthographic fov lens focal",
        Destination::Menu(BuiltinMenu::View),
        "Choose perspective or orthographic viewing and camera settings.",
    ),
    (
        "Viewport lighting",
        "light studio shadows environment exposure",
        Destination::Menu(BuiltinMenu::View),
        "Adjust lighting used to inspect the sculpt.",
    ),
    (
        "Dynamic mesh resolution",
        "remesh voxel density quality",
        Destination::Panel(Tool::Mesh),
        "Adjust remeshing settings for the active model.",
    ),
    (
        "Adaptive sculpting",
        "detail triangles subdivision density",
        Destination::Panel(Tool::Mesh),
        "Configure local mesh refinement while sculpting.",
    ),
    (
        "Subdivision levels",
        "higher lower resolution smooth",
        Destination::Panel(Tool::Mesh),
        "Inspect stored levels and subdivision settings.",
    ),
    (
        "Texture library",
        "images maps color colour normal roughness",
        Destination::Uv(true),
        "Find, create, import and assign texture images.",
    ),
    (
        "UV islands",
        "unwrap packing layout seams coordinates",
        Destination::Uv(false),
        "Inspect and edit the active model's UV layout.",
    ),
    (
        "Texture resolution",
        "image width height size pixels precision",
        Destination::Uv(true),
        "Choose image size and precision when creating a texture.",
    ),
    (
        "External paint application",
        "gimp export reload image editor",
        Destination::Uv(true),
        "Send a texture to an external image editor and reload the edited result.",
    ),
    (
        "Paint target",
        "material texture channel scope model surface",
        Destination::Paint,
        "Inspect which surface and channel the next paint stroke affects.",
    ),
    (
        "Shared colour",
        "color material existing surfaces linked",
        Destination::Menu(BuiltinMenu::Materials),
        "Edit the shared material colour. Existing painted uses update together.",
    ),
    (
        "Material name",
        "rename shared project material",
        Destination::Menu(BuiltinMenu::Materials),
        "Rename the selected shared material without changing its identity.",
    ),
    (
        "Emission colour",
        "glow emitted light material",
        Destination::Menu(BuiltinMenu::Materials),
        "Choose the colour emitted by the shared material.",
    ),
    (
        "Subsurface weight",
        "skin wax translucency scatter",
        Destination::Menu(BuiltinMenu::Materials),
        "Set the amount of subsurface scattering in the shared material.",
    ),
    (
        "Subsurface scale",
        "scatter distance translucent material",
        Destination::Menu(BuiltinMenu::Materials),
        "Scale the scattering distance in the shared material.",
    ),
    (
        "Subsurface radius RGB",
        "red green blue scattering distance",
        Destination::Menu(BuiltinMenu::Materials),
        "Adjust the red, green and blue scattering distances.",
    ),
    (
        "Material layers",
        "mix add surface stacking layers",
        Destination::Menu(BuiltinMenu::Materials),
        "Inspect the layers of the shared material. Changes update its painted uses.",
    ),
    (
        "Layer weight",
        "opacity blend material layer",
        Destination::Menu(BuiltinMenu::Materials),
        "Set how strongly each additional material layer contributes. Add a layer first if none exist.",
    ),
    (
        "Layer blending",
        "mix add material surface layers",
        Destination::Menu(BuiltinMenu::Materials),
        "Choose Mix or Add for an additional material layer.",
    ),
    (
        "Assign material to model",
        "apply existing surface vertices full fill",
        Destination::Menu(BuiltinMenu::Materials),
        "Find the explicit assignment control for the named active model. Revealing it does not assign anything.",
    ),
    (
        "Save material to personal library",
        "reuse material future projects preset",
        Destination::Menu(BuiltinMenu::Materials),
        "Save the selected material to your personal library. Find the button without saving automatically.",
    ),
    (
        "Paint channels",
        "whole material colour color roughness metallic",
        Destination::Paint,
        "Choose which vertex appearance channels future Paint strokes affect. Image painting uses colour.",
    ),
    (
        "Paint inspection",
        "inspect lit material unlit colour uv checker",
        Destination::Paint,
        "Choose how to inspect the painted model without changing its saved colours.",
    ),
    (
        "Paint application",
        "airbrush spray regular stroke flow",
        Destination::Paint,
        "Choose regular strokes, continuous Airbrush or Spray dots for the Paint preset.",
    ),
    (
        "Flow per second",
        "airbrush continuous paint rate",
        Destination::Paint,
        "Adjust paint flow when using Airbrush or Spray with a Freehand path.",
    ),
    (
        "Dot density",
        "spray paint scatter amount",
        Destination::Paint,
        "Adjust the number of dots for the Spray dots application.",
    ),
    (
        "Dot size",
        "spray paint scatter radius",
        Destination::Paint,
        "Adjust individual dot size for the Spray dots application.",
    ),
    (
        "Brush roughness",
        "paint inputs future stroke surface",
        Destination::Paint,
        "Adjust roughness for future vertex Paint strokes that include this channel.",
    ),
    (
        "Brush metallic",
        "paint inputs future stroke metal",
        Destination::Paint,
        "Adjust metallic appearance for future vertex Paint strokes that include this channel.",
    ),
    (
        "HDR colour values",
        "float linear rgb high dynamic range paint",
        Destination::Paint,
        "Edit linear RGB values when painting a 32-bit float colour image.",
    ),
    (
        "Island angle",
        "modeling modelling flat connected faces angle",
        Destination::Panel(Tool::Mesh),
        "Set the angle for the Flat island selection target in Modeling. Choose that target to adjust it.",
    ),
    (
        "Eraser radius",
        "modeling modelling paint away component selection",
        Destination::Panel(Tool::Mesh),
        "Set the selection eraser size. Enable Modeling, choose Select, then Paint away selection.",
    ),
    (
        "Paint away selection",
        "modeling erase deselect brush components",
        Destination::Panel(Tool::Mesh),
        "Find the component selection eraser. Enable Modeling and choose Select to use it.",
    ),
    (
        "Individual faces",
        "extrude inset bevel independent faces modeling",
        Destination::Panel(Tool::Mesh),
        "Extrude, inset or bevel each selected face independently. Choose one of those Modeling tools first.",
    ),
    (
        "Snap range",
        "fuse matching loops caps modeling fraction",
        Destination::Panel(Tool::Mesh),
        "Set the contact snapping distance for the Modeling Fuse tool.",
    ),
    (
        "Modeling amount",
        "modelling distance width angle scale fraction radius",
        Destination::Panel(Tool::Mesh),
        "Inspect the amount for the current Modeling edit. Choose an edit tool to expose its amount control.",
    ),
    (
        "Point base radius",
        "point extrude construction width modeling",
        Destination::Panel(Tool::Mesh),
        "Set the starting radius for Point extrude. Choose Point extrude in Vertex mode first.",
    ),
    (
        "Circularize ring",
        "circularise round point split extrude",
        Destination::Panel(Tool::Mesh),
        "Make the ring circular when using Point split or Point extrude.",
    ),
    (
        "Double ring sides",
        "point split four eight construction modeling",
        Destination::Panel(Tool::Mesh),
        "Double a Point split ring from four to eight sides. Choose Point split first.",
    ),
    (
        "Crease ring",
        "point split extrude sharp boundary construction",
        Destination::Panel(Tool::Mesh),
        "Crease the generated ring in Point split or Point extrude.",
    ),
    (
        "Triangulate centre",
        "triangulate center point split fan modeling",
        Destination::Panel(Tool::Mesh),
        "Triangulate the centre of a Point split ring. Choose Point split first.",
    ),
    (
        "Support loop count",
        "edges border loops number",
        Destination::Panel(Tool::Mesh),
        "Choose how many support loops to add around the affected edges.",
    ),
    (
        "Inset width",
        "edges support loops border distance",
        Destination::Panel(Tool::Mesh),
        "Set support-loop inset as a fraction of nearby edge length.",
    ),
    (
        "Border relax amount",
        "edges boundary cleanup relaxation",
        Destination::Panel(Tool::Mesh),
        "Set how far border vertices move when Relax borders is applied.",
    ),
    (
        "Affected edges",
        "holes masked visible groups selected boundary",
        Destination::Panel(Tool::Mesh),
        "Choose which boundaries edge operations affect.",
    ),
    (
        "Smooth subdivision amount",
        "smoothness refine round uncreased surface",
        Destination::Panel(Tool::Mesh),
        "Set the smoothing amount used by Smooth subdivide. Adjusting it alone makes no mesh edit.",
    ),
    (
        "Mask brush preset",
        "temporary mask shape remembered brush",
        Destination::Menu(BuiltinMenu::Mask),
        "Choose the Mask preset used while holding for temporary masking.",
    ),
    (
        "Selection threshold",
        "mask percentage group delete extraction",
        Destination::Menu(BuiltinMenu::Mask),
        "Set the mask threshold used by grouping, deletion and extraction.",
    ),
    (
        "Mask intensity",
        "mask strength protection scale",
        Destination::Menu(BuiltinMenu::Mask),
        "Scale the active mask while preserving its soft falloff.",
    ),
    (
        "Cap cuts",
        "slice fill holes caps trim separate",
        Destination::Menu(BuiltinMenu::Slice),
        "Choose whether Slice fills cut boundaries. Available for Trim and Separate.",
    ),
    (
        "Flip kept side",
        "slice opposite half cut trim reverse",
        Destination::Menu(BuiltinMenu::Slice),
        "Choose the opposite side to keep when trimming with Slice.",
    ),
    (
        "Primitive width",
        "Width shape sphere cube cylinder cone torus dimensions resolution",
        Destination::Menu(BuiltinMenu::Primitive),
        "Adjust width on a matching parametric primitive. Create or select that primitive first; revealing this control does not replace the model.",
    ),
    (
        "Primitive height",
        "Height shape sphere cube cylinder cone torus dimensions resolution",
        Destination::Menu(BuiltinMenu::Primitive),
        "Adjust height on a matching parametric primitive. Create or select that primitive first; revealing this control does not replace the model.",
    ),
    (
        "Primitive depth",
        "Depth shape sphere cube cylinder cone torus dimensions resolution",
        Destination::Menu(BuiltinMenu::Primitive),
        "Adjust depth on a matching parametric primitive. Create or select that primitive first; revealing this control does not replace the model.",
    ),
    (
        "Primitive radius",
        "Radius Base radius Major radius shape sphere cube cylinder cone torus dimensions resolution",
        Destination::Menu(BuiltinMenu::Primitive),
        "Adjust radius on a matching parametric primitive. Create or select that primitive first; revealing this control does not replace the model.",
    ),
    (
        "Primitive top radius",
        "Top radius shape sphere cube cylinder cone torus dimensions resolution",
        Destination::Menu(BuiltinMenu::Primitive),
        "Adjust top radius on a matching parametric primitive. Create or select that primitive first; revealing this control does not replace the model.",
    ),
    (
        "Primitive tube radius",
        "Tube radius shape sphere cube cylinder cone torus dimensions resolution",
        Destination::Menu(BuiltinMenu::Primitive),
        "Adjust tube radius on a matching parametric primitive. Create or select that primitive first; revealing this control does not replace the model.",
    ),
    (
        "Primitive x spans",
        "X spans shape sphere cube cylinder cone torus dimensions resolution",
        Destination::Menu(BuiltinMenu::Primitive),
        "Adjust x spans on a matching parametric primitive. Create or select that primitive first; revealing this control does not replace the model.",
    ),
    (
        "Primitive y spans",
        "Y spans shape sphere cube cylinder cone torus dimensions resolution",
        Destination::Menu(BuiltinMenu::Primitive),
        "Adjust y spans on a matching parametric primitive. Create or select that primitive first; revealing this control does not replace the model.",
    ),
    (
        "Primitive z spans",
        "Z spans shape sphere cube cylinder cone torus dimensions resolution",
        Destination::Menu(BuiltinMenu::Primitive),
        "Adjust z spans on a matching parametric primitive. Create or select that primitive first; revealing this control does not replace the model.",
    ),
    (
        "Primitive segments",
        "Segments shape sphere cube cylinder cone torus dimensions resolution",
        Destination::Menu(BuiltinMenu::Primitive),
        "Adjust segments on a matching parametric primitive. Create or select that primitive first; revealing this control does not replace the model.",
    ),
    (
        "Primitive rings",
        "Rings shape sphere cube cylinder cone torus dimensions resolution",
        Destination::Menu(BuiltinMenu::Primitive),
        "Adjust rings on a matching parametric primitive. Create or select that primitive first; revealing this control does not replace the model.",
    ),
    (
        "Primitive subdivisions",
        "Subdivisions shape sphere cube cylinder cone torus dimensions resolution",
        Destination::Menu(BuiltinMenu::Primitive),
        "Adjust subdivisions on a matching parametric primitive. Create or select that primitive first; revealing this control does not replace the model.",
    ),
    (
        "Primitive vertices",
        "Vertices shape sphere cube cylinder cone torus dimensions resolution",
        Destination::Menu(BuiltinMenu::Primitive),
        "Adjust vertices on a matching parametric primitive. Create or select that primitive first; revealing this control does not replace the model.",
    ),
    (
        "Primitive sides",
        "Sides shape sphere cube cylinder cone torus dimensions resolution",
        Destination::Menu(BuiltinMenu::Primitive),
        "Adjust sides on a matching parametric primitive. Create or select that primitive first; revealing this control does not replace the model.",
    ),
    (
        "Primitive height spans",
        "Height spans shape sphere cube cylinder cone torus dimensions resolution",
        Destination::Menu(BuiltinMenu::Primitive),
        "Adjust height spans on a matching parametric primitive. Create or select that primitive first; revealing this control does not replace the model.",
    ),
    (
        "Primitive major segments",
        "Major segments shape sphere cube cylinder cone torus dimensions resolution",
        Destination::Menu(BuiltinMenu::Primitive),
        "Adjust major segments on a matching parametric primitive. Create or select that primitive first; revealing this control does not replace the model.",
    ),
    (
        "Primitive tube segments",
        "Tube segments shape sphere cube cylinder cone torus dimensions resolution",
        Destination::Menu(BuiltinMenu::Primitive),
        "Adjust tube segments on a matching parametric primitive. Create or select that primitive first; revealing this control does not replace the model.",
    ),
    (
        "Primitive fill and caps",
        "Fill circle Cap ends Triangle topology triangulated geometry",
        Destination::Menu(BuiltinMenu::Primitive),
        "Find fill, end cap and triangle topology options for the selected parametric shape.",
    ),
    (
        "Adaptive edge size",
        "Edge size px percent radius detail",
        Destination::Panel(Tool::Mesh),
        "Find adaptive edge size in Mesh settings. Choose the corresponding Adaptive sculpt, Dynamic mesh or Subdivision mode to adjust it.",
    ),
    (
        "Simplify ratio",
        "collapse ratio decimate reduction",
        Destination::Panel(Tool::Mesh),
        "Find simplify ratio in Mesh settings. Choose the corresponding Adaptive sculpt, Dynamic mesh or Subdivision mode to adjust it.",
    ),
    (
        "Detail relative to brush size",
        "relative radius adaptive",
        Destination::Panel(Tool::Mesh),
        "Find detail relative to brush size in Mesh settings. Choose the corresponding Adaptive sculpt, Dynamic mesh or Subdivision mode to adjust it.",
    ),
    (
        "Refine stretched edges",
        "refinement adaptive",
        Destination::Panel(Tool::Mesh),
        "Find refine stretched edges in Mesh settings. Choose the corresponding Adaptive sculpt, Dynamic mesh or Subdivision mode to adjust it.",
    ),
    (
        "Simplify while sculpting",
        "automatic decimate adaptive",
        Destination::Panel(Tool::Mesh),
        "Find simplify while sculpting in Mesh settings. Choose the corresponding Adaptive sculpt, Dynamic mesh or Subdivision mode to adjust it.",
    ),
    (
        "Adapt only unmasked areas",
        "mask protection adaptive topology",
        Destination::Panel(Tool::Mesh),
        "Find adapt only unmasked areas in Mesh settings. Choose the corresponding Adaptive sculpt, Dynamic mesh or Subdivision mode to adjust it.",
    ),
    (
        "Adaptive topology",
        "quads triangles local mesh",
        Destination::Panel(Tool::Mesh),
        "Find adaptive topology in Mesh settings. Choose the corresponding Adaptive sculpt, Dynamic mesh or Subdivision mode to adjust it.",
    ),
    (
        "Remesh resolution",
        "Resolution voxels longest bounding axis",
        Destination::Panel(Tool::Mesh),
        "Find remesh resolution in Mesh settings. Choose the corresponding Adaptive sculpt, Dynamic mesh or Subdivision mode to adjust it.",
    ),
    (
        "Remesh topology",
        "quads triangles dynamic mesh",
        Destination::Panel(Tool::Mesh),
        "Find remesh topology in Mesh settings. Choose the corresponding Adaptive sculpt, Dynamic mesh or Subdivision mode to adjust it.",
    ),
    (
        "Quad algorithm",
        "Fast quads Optimized quads optimised",
        Destination::Panel(Tool::Mesh),
        "Find quad algorithm in Mesh settings. Choose the corresponding Adaptive sculpt, Dynamic mesh or Subdivision mode to adjust it.",
    ),
    (
        "Active subdivision level",
        "Active level higher lower stored detail",
        Destination::Panel(Tool::Mesh),
        "Find active subdivision level in Mesh settings. Choose the corresponding Adaptive sculpt, Dynamic mesh or Subdivision mode to adjust it.",
    ),
    (
        "Stencil position",
        "Horizontal Vertical placement",
        Destination::Panel(Tool::Alpha),
        "Adjust stencil position after enabling an alpha stencil. This changes the guide, without editing the mesh.",
    ),
    (
        "Stencil size",
        "Width Height scale placement",
        Destination::Panel(Tool::Alpha),
        "Adjust stencil size after enabling an alpha stencil. This changes the guide, without editing the mesh.",
    ),
    (
        "Stencil rotation",
        "Rotation angle placement",
        Destination::Panel(Tool::Alpha),
        "Adjust stencil rotation after enabling an alpha stencil. This changes the guide, without editing the mesh.",
    ),
    (
        "Stencil overlay opacity",
        "Overlay opacity transparency placement",
        Destination::Panel(Tool::Alpha),
        "Adjust stencil overlay opacity after enabling an alpha stencil. This changes the guide, without editing the mesh.",
    ),
    (
        "Alpha crop",
        "Crop source fractions Left Top Right Bottom",
        Destination::Panel(Tool::Alpha),
        "Find alpha crop in the alpha editor. Select a project alpha and choose Edit alpha, or Duplicate alpha for a built-in image, to open an editable draft.",
    ),
    (
        "Alpha black level",
        "Black levels image heights",
        Destination::Panel(Tool::Alpha),
        "Find alpha black level in the alpha editor. Select a project alpha and choose Edit alpha, or Duplicate alpha for a built-in image, to open an editable draft.",
    ),
    (
        "Alpha white level",
        "White levels image heights",
        Destination::Panel(Tool::Alpha),
        "Find alpha white level in the alpha editor. Select a project alpha and choose Edit alpha, or Duplicate alpha for a built-in image, to open an editable draft.",
    ),
    (
        "Alpha gamma",
        "Gamma contrast image heights",
        Destination::Panel(Tool::Alpha),
        "Find alpha gamma in the alpha editor. Select a project alpha and choose Edit alpha, or Duplicate alpha for a built-in image, to open an editable draft.",
    ),
    (
        "Alpha edge fade",
        "Edge fade vignette alpha",
        Destination::Panel(Tool::Alpha),
        "Find alpha edge fade in the alpha editor. Select a project alpha and choose Edit alpha, or Duplicate alpha for a built-in image, to open an editable draft.",
    ),
    (
        "Invert alpha heights",
        "Invert heights negative alpha",
        Destination::Panel(Tool::Alpha),
        "Find invert alpha heights in the alpha editor. Select a project alpha and choose Edit alpha, or Duplicate alpha for a built-in image, to open an editable draft.",
    ),
    (
        "Alpha name and category",
        "Name Category rename classify image",
        Destination::Panel(Tool::Alpha),
        "Find alpha name and category in the alpha editor. Select a project alpha and choose Edit alpha, or Duplicate alpha for a built-in image, to open an editable draft.",
    ),
    (
        "Light azimuth",
        "horizontal angle direction orbit",
        Destination::Menu(BuiltinMenu::View),
        "Adjust light azimuth in Lighting. Per-light settings apply to the selected light.",
    ),
    (
        "Light elevation",
        "vertical angle direction",
        Destination::Menu(BuiltinMenu::View),
        "Adjust light elevation in Lighting. Per-light settings apply to the selected light.",
    ),
    (
        "Light distance",
        "point light distance",
        Destination::Menu(BuiltinMenu::View),
        "Adjust light distance in Lighting. Per-light settings apply to the selected light.",
    ),
    (
        "Light intensity",
        "brightness exposure lamp",
        Destination::Menu(BuiltinMenu::View),
        "Adjust light intensity in Lighting. Per-light settings apply to the selected light.",
    ),
    (
        "Light colour",
        "light color tint",
        Destination::Menu(BuiltinMenu::View),
        "Adjust light colour in Lighting. Per-light settings apply to the selected light.",
    ),
    (
        "Light follows camera",
        "camera relative follow",
        Destination::Menu(BuiltinMenu::View),
        "Adjust light follows camera in Lighting. Per-light settings apply to the selected light.",
    ),
    (
        "Ambient fill",
        "ambient brightness environment",
        Destination::Menu(BuiltinMenu::View),
        "Adjust ambient fill in Lighting. Per-light settings apply to the selected light.",
    ),
    (
        "Show light controls",
        "gizmo light viewport",
        Destination::Menu(BuiltinMenu::View),
        "Adjust show light controls in Lighting. Per-light settings apply to the selected light.",
    ),
    (
        "Light type",
        "Directional light Point light",
        Destination::Menu(BuiltinMenu::View),
        "Adjust light type in Lighting. Per-light settings apply to the selected light.",
    ),
    (
        "Light list",
        "Add light Remove light selected",
        Destination::Menu(BuiltinMenu::View),
        "Adjust light list in Lighting. Per-light settings apply to the selected light.",
    ),
    (
        "Target quads",
        "quad target count polygons",
        Destination::Panel(Tool::Mesh),
        "Find target quads in Guided retopology. Open its preview editor from Mesh management to adjust the setting.",
    ),
    (
        "Protect sharp bends",
        "sharp angle degrees features",
        Destination::Panel(Tool::Mesh),
        "Find protect sharp bends in Guided retopology. Open its preview editor from Mesh management to adjust the setting.",
    ),
    (
        "Input triangle limit",
        "retopology maximum input budget",
        Destination::Panel(Tool::Mesh),
        "Find input triangle limit in Guided retopology. Open its preview editor from Mesh management to adjust the setting.",
    ),
    (
        "Layout symmetry",
        "experimental Add centreline Keep sculpt shape Keep + and mirror",
        Destination::Panel(Tool::Mesh),
        "Find layout symmetry in Guided retopology. Open its preview editor from Mesh management to adjust the setting.",
    ),
    (
        "Retopology path strength",
        "Path strength Edge direction Mirror paths across X",
        Destination::Panel(Tool::Mesh),
        "Find retopology path strength in Guided retopology. Open its preview editor from Mesh management to adjust the setting.",
    ),
    (
        "Shape adaptation",
        "adaptive polygon density surface bends",
        Destination::Panel(Tool::Mesh),
        "Find shape adaptation in Guided retopology. Open its preview editor from Mesh management to adjust the setting.",
    ),
    (
        "Density strength",
        "retopology density painting detail",
        Destination::Panel(Tool::Mesh),
        "Find density strength in Guided retopology. Open its preview editor from Mesh management to adjust the setting.",
    ),
    (
        "Density brush radius",
        "Brush radius retopology paint more less detail",
        Destination::Panel(Tool::Mesh),
        "Find density brush radius in Guided retopology. Open its preview editor from Mesh management to adjust the setting.",
    ),
    (
        "Density paint strength",
        "Paint strength retopology",
        Destination::Panel(Tool::Mesh),
        "Find density paint strength in Guided retopology. Open its preview editor from Mesh management to adjust the setting.",
    ),
    (
        "Path editing options",
        "Smoothing Rounding Corner Automatic pressure Use active brush size Custom radius Run in reverse Invert brush effect Preview stroke Save path",
        Destination::Panel(Tool::Mesh),
        "Find Editable paths to inspect the active path draft and its brush preview options. Opening the path editor is an explicit action; search does not switch tools.",
    ),
    (
        "Array layer options",
        "Count Copies Columns Grid spacing Arc angle Axis Rotate copies around centre Linear Grid Radial",
        Destination::Panel(Tool::Submodels),
        "Find the linked array on its source model, then choose Edit array to inspect layer count, spacing and rotation. Search keeps the current tool and model.",
    ),
];

pub(super) fn setting_section(name: &str) -> Option<&'static str> {
    match name {
        "Primitive dimensions" => Some("primitive_dimensions"),
        "Primitive width" => Some("primitive_dimensions"),
        "Primitive height" => Some("primitive_dimensions"),
        "Primitive depth" => Some("primitive_dimensions"),
        "Primitive radius" => Some("primitive_dimensions"),
        "Primitive top radius" => Some("primitive_dimensions"),
        "Primitive tube radius" => Some("primitive_dimensions"),
        "Primitive x spans" => Some("primitive_dimensions"),
        "Primitive y spans" => Some("primitive_dimensions"),
        "Primitive z spans" => Some("primitive_dimensions"),
        "Primitive segments" => Some("primitive_dimensions"),
        "Primitive rings" => Some("primitive_dimensions"),
        "Primitive subdivisions" => Some("primitive_dimensions"),
        "Primitive vertices" => Some("primitive_dimensions"),
        "Primitive sides" => Some("primitive_dimensions"),
        "Primitive height spans" => Some("primitive_dimensions"),
        "Primitive major segments" => Some("primitive_dimensions"),
        "Primitive tube segments" => Some("primitive_dimensions"),
        "Primitive fill and caps" => Some("primitive_dimensions"),
        "Adaptive edge size" => Some("mesh_settings"),
        "Simplify ratio" => Some("mesh_settings"),
        "Detail relative to brush size" => Some("mesh_settings"),
        "Refine stretched edges" => Some("mesh_settings"),
        "Simplify while sculpting" => Some("mesh_settings"),
        "Adapt only unmasked areas" => Some("mesh_settings"),
        "Adaptive topology" => Some("mesh_settings"),
        "Remesh resolution" => Some("mesh_settings"),
        "Remesh topology" => Some("mesh_settings"),
        "Quad algorithm" => Some("mesh_settings"),
        "Active subdivision level" => Some("mesh_settings"),
        "Stencil position" => Some("stencil_placement"),
        "Stencil size" => Some("stencil_placement"),
        "Stencil rotation" => Some("stencil_placement"),
        "Stencil overlay opacity" => Some("stencil_placement"),
        "Alpha crop" => Some("alpha_adjustments"),
        "Alpha black level" => Some("alpha_adjustments"),
        "Alpha white level" => Some("alpha_adjustments"),
        "Alpha gamma" => Some("alpha_adjustments"),
        "Alpha edge fade" => Some("alpha_adjustments"),
        "Invert alpha heights" => Some("alpha_adjustments"),
        "Alpha name and category" => Some("alpha_adjustments"),
        "Light azimuth" => Some("viewport_lighting"),
        "Light elevation" => Some("viewport_lighting"),
        "Light distance" => Some("viewport_lighting"),
        "Light intensity" => Some("viewport_lighting"),
        "Light colour" => Some("viewport_lighting"),
        "Light follows camera" => Some("viewport_lighting"),
        "Ambient fill" => Some("viewport_lighting"),
        "Show light controls" => Some("viewport_lighting"),
        "Light type" => Some("viewport_lighting"),
        "Light list" => Some("viewport_lighting"),
        "Target quads" => Some("retopology_options"),
        "Protect sharp bends" => Some("retopology_options"),
        "Input triangle limit" => Some("retopology_options"),
        "Layout symmetry" => Some("retopology_options"),
        "Retopology path strength" => Some("retopology_options"),
        "Shape adaptation" => Some("retopology_options"),
        "Density strength" => Some("retopology_options"),
        "Density brush radius" => Some("retopology_options"),
        "Density paint strength" => Some("retopology_options"),
        "Path editing options" => Some("mesh_settings"),
        "Array layer options" => Some("array_layers"),
        "Shared colour" => Some("material_shared"),
        "Material name" => Some("material_shared"),
        "Emission colour" => Some("material_surface"),
        "Subsurface weight" => Some("material_subsurface"),
        "Subsurface scale" => Some("material_subsurface"),
        "Subsurface radius RGB" => Some("material_subsurface"),
        "Material layers" => Some("material_layers"),
        "Layer weight" => Some("material_layers"),
        "Layer blending" => Some("material_layers"),
        "Save material to personal library" => Some("material_shared"),
        "HDR colour values" => Some("paint_hdr"),
        "Island angle" => Some("modeling_options"),
        "Eraser radius" => Some("modeling_options"),
        "Paint away selection" => Some("modeling_options"),
        "Individual faces" => Some("modeling_options"),
        "Snap range" => Some("modeling_options"),
        "Modeling amount" => Some("modeling_options"),
        "Point base radius" => Some("modeling_options"),
        "Circularize ring" => Some("modeling_options"),
        "Double ring sides" => Some("modeling_options"),
        "Crease ring" => Some("modeling_options"),
        "Triangulate centre" => Some("modeling_options"),
        "Support loop count" => Some("edge_support"),
        "Inset width" => Some("edge_support"),
        "Border relax amount" => Some("edge_cleanup"),
        "Affected edges" => Some("edge_controls"),
        "Smooth subdivision amount" => Some("mesh_settings"),
        "Mask brush preset" => Some("mask_hold"),
        "Roughness" | "Metallic" => Some("material_shared"),
        "Workspace appearance" => Some("workspace_appearance"),
        "Panel placement" | "Recover workspace" => Some("workspace_panels"),
        "Saved workspaces" => Some("workspace_saved"),
        "Touch navigation"
        | "Pinch to zoom"
        | "Two-finger twist to roll view"
        | "Trackpad scrolling"
        | "Drag sensitivity"
        | "Pinch sensitivity"
        | "Twist sensitivity" => Some("touch_navigation"),
        "Viewport lighting" => Some("viewport_lighting"),
        "Dynamic mesh resolution" | "Adaptive sculpting" | "Subdivision levels" => {
            Some("mesh_settings")
        }
        "Texture resolution" => Some("texture_resolution"),
        "External paint application" => Some("external_paint"),
        "Stroke spacing" | "Stroke stabilisation" | "Trailing distance" => {
            Some("brush_stroke_settings")
        }
        "Brush falloff"
        | "Pressure curve"
        | "Pressure to strength"
        | "Pressure to radius"
        | "Alpha rotation" => Some("brush_shape_settings"),
        "IOR" | "Specular" | "Emission strength" => Some("material_surface"),
        "Subsurface scattering" => Some("material_subsurface"),
        _ => None,
    }
}

/// Tag a non-action widget without changing its normal response or interaction.
pub(crate) fn setting_response(response: egui::Response, name: &str) -> egui::Response {
    highlight_response(&response, &format!("setting:{name}"));
    response
}

pub(crate) fn requested_any_section(ctx: &egui::Context, sections: &[&str]) -> Option<bool> {
    sections
        .iter()
        .find_map(|section| requested_section(ctx, section))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn material_search_opens_the_outer_editor_and_nested_surface_options() {
        let ctx = egui::Context::default();
        ctx.style_mut(|style| style.animation_time = 0.);
        let mut state = UiState::default();
        let document = Document::default();
        let selected = state.selected;
        let entry = state
            .search_entries(&document)
            .into_iter()
            .find(|entry| entry.name == "IOR")
            .unwrap();
        state.reveal_search_entry(&ctx, &entry);
        let mut output = None;
        for _ in 0..2 {
            output = Some(ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(900., 1600.),
                    )),
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        state.appearance_controls(ui, &document, &mut Vec::new(), false);
                    });
                },
            ));
        }
        assert_eq!(
            state.selected, selected,
            "revealing material options must not choose a Paint brush"
        );
        assert!(output.unwrap().shapes.iter().any(|shape| matches!(&shape.shape, egui::Shape::Text(text) if text.galley.text() == "IOR")), "nested IOR slider must actually be rendered");
    }

    #[test]
    fn specific_controls_resolve_to_their_real_editors() {
        let state = UiState::default();
        let entries = state.search_entries(&Document::default());
        for (name, destination, section) in [
            ("Paint target", Destination::Paint, None),
            (
                "Individual faces",
                Destination::Panel(Tool::Mesh),
                Some("modeling_options"),
            ),
            (
                "Inset width",
                Destination::Panel(Tool::Mesh),
                Some("edge_support"),
            ),
            (
                "Subsurface scale",
                Destination::Menu(BuiltinMenu::Materials),
                Some("material_subsurface"),
            ),
        ] {
            let entry = entries.iter().find(|entry| entry.name == name).unwrap();
            assert_eq!(entry.destination, destination);
            assert_eq!(entry.section.as_deref(), section);
        }
        let mut names = std::collections::HashSet::new();
        assert!(
            SETTINGS.iter().all(|(name, _, _, _)| names.insert(*name)),
            "setting keys must remain unique"
        );
    }
}
