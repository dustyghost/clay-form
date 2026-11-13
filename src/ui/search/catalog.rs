use super::settings::{SETTINGS, setting_section};
use super::*;

impl UiState {
    pub(super) fn search_entries(&self, document: &Document) -> Vec<Entry> {
        let mut entries = self.action_catalog(document).into_iter().map(|action| {
            let mut destination = action_destination(&action);
            let hidden = matches!(destination, Destination::Panel(tool) if self.preferences.panels.iter().find(|p| p.tool == tool).is_none_or(|p| p.dock == Dock::Hidden));
            if hidden && let Some(menu) = self.preferences.menus.iter().find(|menu| menu.placement != Placement::Hidden && menu.items.iter().any(|item| item.action.as_ref() == Some(&action))) {
                destination = Destination::Custom(menu.id);
            }
            let navigation = if hidden && matches!(destination, Destination::Panel(_)) && self.preferences.viewport_controls.iter().any(|control| control.kind == ViewportControlKind::Action(action.clone())) {
                destination = Destination::Viewport;
                Some(ViewportControlKind::Action(action.clone()))
            } else { None };
            let name = self.action_label(&action, document);
            let kind = match &action {
                Action::SelectBrush(_) | Action::NewBrush(_) => "Brush",
                Action::ShowPanel(_) => "Panel",
                Action::OpenMenu(_) => "Custom menu",
                Action::ModelPreset(_) => "Preset",
                action if super::super::brush_controls::is_control(action) => "Setting",
                _ => "Command",
            };
            Entry {
                key: format!("action:{action:?}"),
                description: action.help().map(str::to_owned).unwrap_or_else(|| action_description(&action)),
                aliases: format!("{} {}", action_aliases(&action), self.preferences.shortcut(&action).map(|s| s.label()).unwrap_or_default()),
                location: self.search_location(&destination),
                section: action_section(&action).map(str::to_owned),
                destination,
                name,
                kind,
                action: Some(action),
                navigation,
            }
        }).collect::<Vec<_>>();
        for (name, aliases, destination, description) in SETTINGS {
            entries.push(Entry {
                key: format!("setting:{name}"),
                name: (*name).into(),
                kind: "Setting",
                description: (*description).into(),
                aliases: (*aliases).into(),
                location: self.search_location(destination),
                destination: destination.clone(),
                action: None,
                navigation: None,
                section: setting_section(name).map(str::to_owned),
            });
        }
        for menu in MENUS {
            let destination = Destination::Menu(*menu);
            entries.push(Entry {
                key: format!("menu:{menu:?}"),
                name: menu.label().into(),
                kind: "Menu",
                description: format!(
                    "Show the {} controls in their current workspace location.",
                    menu.label()
                ),
                aliases: String::new(),
                location: self.search_location(&destination),
                destination,
                action: None,
                navigation: None,
                section: None,
            });
        }
        for (kind, name, aliases, description) in [
            (
                ViewportControlKind::Pan,
                "Pan view",
                "move camera navigation",
                "Drag to move the camera sideways or vertically.",
            ),
            (
                ViewportControlKind::Zoom,
                "Zoom view",
                "magnify camera navigation",
                "Drag up to zoom in and down to zoom out.",
            ),
            (
                ViewportControlKind::Orbit,
                "Orbit view",
                "rotate camera navigation",
                "Drag to orbit the camera around the model.",
            ),
            (
                ViewportControlKind::Roll,
                "Roll view",
                "tilt camera horizon navigation",
                "Drag to roll the camera in screen space without changing the model.",
            ),
        ] {
            entries.push(Entry {
                key: format!("navigation:{kind:?}"),
                name: name.into(),
                kind: "Drag control",
                description: description.into(),
                aliases: aliases.into(),
                location: "Viewport navigation / Workspace > Viewport controls".into(),
                destination: Destination::Viewport,
                action: None,
                navigation: Some(kind),
                section: None,
            });
        }
        let mut named = |key: String,
                         name: String,
                         kind: &'static str,
                         destination: Destination,
                         description: String| {
            let section = if kind == "Model" {
                (0..document.model_count())
                    .filter_map(|i| document.model(i))
                    .find(|model| key == format!("model:{}", model.metadata.model_id))
                    .map(|model| format!("model-group:{}", model.metadata.group))
            } else if kind == "Model group" {
                Some(format!("model-group:{name}"))
            } else if kind == "Workspace" {
                Some("workspace_saved".into())
            } else if kind == "Path" {
                Some("saved_paths".into())
            } else if matches!(kind, "Reusable model" | "Personal object brush") {
                Some("reusable_models".into())
            } else {
                None
            };
            entries.push(Entry {
                key,
                name,
                kind,
                description,
                aliases: String::new(),
                location: self.search_location(&destination),
                destination,
                action: None,
                navigation: None,
                section,
            });
        };
        for index in 0..document.model_count() {
            if let Some(model) = document.model(index) {
                named(format!("model:{}", model.metadata.model_id), model.name.into(), "Model", Destination::Panel(Tool::Submodels), "Find this model in Submodels. Revealing it keeps the current sculpt target unchanged.".into());
            }
        }
        for name in document.groups() {
            named(
                format!("model-group:{name}"),
                name.clone(),
                "Model group",
                Destination::Panel(Tool::Submodels),
                "Find this group of models in Submodels.".into(),
            );
        }
        for group in &document.mesh().vertex_groups {
            named(
                format!("vertex-group:{}", group.name),
                group.name.clone(),
                "Vertex group",
                Destination::Panel(Tool::Submodels),
                "Find the named vertex group on the active model.".into(),
            );
        }
        for path in document.paths() {
            named(format!("path:{}", path.id), path.name.clone(), "Path", Destination::Panel(Tool::Mesh), "Find this saved path in Mesh management > Saved paths. Use Edit paths to change it.".into());
        }
        for (index, alpha) in document.alphas().iter().enumerate() {
            named(format!("alpha:{index}:{}", alpha.name), alpha.name.clone(), "Project alpha", Destination::Panel(Tool::Alpha), "Find this alpha in the project library. Select it there to change the active brush.".into());
        }
        for (index, alpha) in crate::sculpt::alpha::builtins().iter().enumerate() {
            named(
                format!("builtin-alpha:{index}"),
                alpha.name.clone(),
                "Built-in alpha",
                Destination::Panel(Tool::Alpha),
                "Find this built-in alpha in the Alpha library.".into(),
            );
        }
        for texture in document.texture_assets() {
            named(
                format!("texture:{}", texture.id),
                texture.name.clone(),
                "Texture",
                Destination::Uv(true),
                format!(
                    "Find this {} × {} texture in the texture library.",
                    texture.width, texture.height
                ),
            );
        }
        for asset in document.materials() {
            named(format!("material:{}", asset.id), asset.name.clone(), "Project material", Destination::Menu(BuiltinMenu::Materials), "Find this shared project material. Editing it can affect existing linked surfaces.".into());
        }
        for (index, name) in self.preferences.materials.names.iter().enumerate() {
            if !self
                .preferences
                .materials
                .hidden_builtins
                .contains(&(index as u64 + 1))
                && !document
                    .materials()
                    .iter()
                    .any(|a| a.id == index as u64 + 1)
            {
                named(
                    format!("material:{}", index + 1),
                    name.clone(),
                    "Material preset",
                    Destination::Menu(BuiltinMenu::Materials),
                    "Find this material preset for future paint strokes.".into(),
                );
            }
        }
        for asset in &self.preferences.materials.copies {
            if !document.materials().iter().any(|a| a.id == asset.id) {
                named(
                    format!("material:{}", asset.id),
                    asset.name.clone(),
                    "Material preset",
                    Destination::Menu(BuiltinMenu::Materials),
                    "Find this saved material preset.".into(),
                );
            }
        }
        for (index, reference) in document.references().iter().enumerate() {
            named(
                format!("reference:{index}:{}", reference.name),
                reference.name.clone(),
                "Reference image",
                Destination::Menu(BuiltinMenu::References),
                "Find this project reference image and its placement controls.".into(),
            );
        }
        for brush in &document.instances().brushes {
            named(format!("object-brush:{}", brush.id), brush.name.clone(), "Object brush", Destination::Panel(Tool::Library), "Find this object brush in the Brush library. Select its tile to place its objects.".into());
        }
        for asset in &document.instances().assets {
            named(format!("object-asset:{}", asset.id), asset.name.clone(), "Reusable model", Destination::Panel(Tool::Library), "Find this captured model in the reusable model library. Existing placed instances remain unchanged.".into());
        }
        for name in &self.instances.personal_brushes {
            named(
                format!("personal-brush:{name}"),
                name.clone(),
                "Personal object brush",
                Destination::Panel(Tool::Library),
                "Find this saved object brush. Add it to the project explicitly to use it.".into(),
            );
        }
        for (index, workspace) in self.preferences.workspaces.iter().enumerate() {
            named(format!("workspace:{index}:{}", workspace.name), workspace.name.clone(), "Workspace", Destination::Workspace, "Find this named workspace. Choose Load there to switch layouts; shortcuts remain shared.".into());
        }
        entries
    }
    fn search_location(&self, destination: &Destination) -> String {
        match destination {
            Destination::Panel(tool) => {
                let place = self
                    .preferences
                    .panels
                    .iter()
                    .find(|p| p.tool == *tool)
                    .map(|p| match p.dock {
                        Dock::Left => "Left sidebar",
                        Dock::Right => "Right sidebar",
                        Dock::Floating => "Floating panel",
                        Dock::Hidden => "Hidden panel (temporary reveal)",
                    })
                    .unwrap_or("Workspace");
                format!("{place} > {}", tool.label())
            }
            Destination::Menu(menu) => {
                let place = if let Some(section) = self
                    .preferences
                    .menu_sections
                    .iter()
                    .find(|s| s.menu == *menu)
                {
                    if section.dock == Dock::Left {
                        "Left sidebar"
                    } else {
                        "Right sidebar"
                    }
                } else if self
                    .preferences
                    .pinned_menus
                    .iter()
                    .any(|p| p.menu == *menu)
                {
                    "Floating menu"
                } else {
                    "Toolbar"
                };
                format!("{place} > {}", menu.label())
            }
            Destination::Custom(id) => self
                .preferences
                .menus
                .iter()
                .find(|m| m.id == *id)
                .map(|m| format!("{} > {}", m.placement.label(), m.name))
                .unwrap_or_else(|| "Custom menu no longer exists".into()),
            Destination::Project => "Project".into(),
            Destination::Paint => "Brush settings > Paint controls".into(),
            Destination::Workspace => "Workspace > Customise workspace".into(),
            Destination::Keys => "Workspace > Keyboard shortcuts".into(),
            Destination::Capacity => "Settings > Mesh capacity".into(),
            Destination::Viewport => "Workspace > Viewport controls".into(),
            Destination::Recovery => "Settings > Recovery".into(),
            Destination::Uv(true) => "Textures > Texture library".into(),
            Destination::Uv(false) => "Textures > UVs".into(),
        }
    }
}
fn action_destination(action: &Action) -> Destination {
    use Action::*;
    match action {
        Search | ToggleZen | CustomizeWorkspace => Destination::Workspace,
        KeySettings => Destination::Keys,
        RecoverySettings | SaveRecovery => Destination::Recovery,
        ShowPanel(tool) => Destination::Panel(*tool),
        OpenMenu(id) => Destination::Custom(*id),
        Save | SaveAs | Open | New | Export | ImportObj => Destination::Project,
        Undo | Redo => Destination::Menu(BuiltinMenu::Edit),
        ResetView | ResetZoom | ZoomIn | ZoomOut | Perspective => {
            Destination::Menu(BuiltinMenu::View)
        }
        Symmetry | Mirror(..) | Guides | CenterSymmetry => Destination::Menu(BuiltinMenu::Symmetry),
        Wireframe | WireVisible | WireFront | WireTransparent | WireTriangles => {
            Destination::Menu(BuiltinMenu::Wireframe)
        }
        FlatShading | PolygroupColors | Backfaces => Destination::Menu(BuiltinMenu::View),
        Slice => Destination::Menu(BuiltinMenu::Slice),
        Primitive | ConvertPrimitive => Destination::Menu(BuiltinMenu::Primitive),
        OpenTextures => Destination::Uv(true),
        OpenUv => Destination::Uv(false),
        OpenMaterials | DefaultPaintMaterial => Destination::Menu(BuiltinMenu::Materials),
        OpenColours | SwapPaintColours | ResetPaintColours => {
            Destination::Menu(BuiltinMenu::Colours)
        }
        AppearanceControls => Destination::Menu(BuiltinMenu::Colours),
        FillMaterial | FillAllMaterial => Destination::Paint,
        CaptureObjectBrush | ObjectBrushModels => Destination::Panel(Tool::Brush),
        ArrayControls => Destination::Panel(Tool::Submodels),
        AddReference | ArrangeReferences | ShowReferences | ReferenceSettings => {
            Destination::Menu(BuiltinMenu::References)
        }
        ClearMask
        | InvertMask
        | DeleteMasked
        | ExpandMask
        | ContractMask
        | BlurMask
        | SharpenMask
        | MaskLasso
        | MaskLine
        | FlipMaskLine
        | TemporaryMaskBrush
        | TemporaryMaskLasso
        | TemporaryMaskLine
        | TemporaryMaskAdd
        | TemporaryMaskErase
        | TemporaryMaskFlip
        | TemporaryMaskPreset(_) => Destination::Menu(BuiltinMenu::Mask),
        MaskVisibilityMenu
        | TemporaryVisibilityRectangle
        | TemporaryVisibilityLasso
        | SelectVisibilityLine
        | SelectVisibilityPath
        | VisibilityHide
        | VisibilityIsolate
        | FlipVisibilitySide
        | ApplyVisibilityPath
        | ClearVisibilityPath
        | DeleteVisibilityPoint
        | SelectRectangle
        | SelectLasso
        | ShowAllFaces
        | InvertVisibility
        | SplitHidden => Destination::Menu(BuiltinMenu::Visibility),
        Sculpt | Transform | LineTransform | TransformMenu | UniversalTransform | Move | Scale
        | Rotate | PlaceLine | PlacePivot | DeformationFrame | ResetPivot | RespectMask => {
            Destination::Panel(Tool::Transform)
        }
        DuplicateModel
        | DeleteSubmodel
        | MergeDown
        | ExtractMasked
        | GroupMasked
        | GroupAll
        | MaskGroup
        | AddMaskedToGroup
        | RemoveMaskedFromGroup
        | DeleteGroup
        | RenameGroup
        | CreateInstanceArray
        | CreatePathAssembly
        | ApplyThickness
        | TemporaryThickness => Destination::Panel(Tool::Submodels),
        SelectBrush(_) | PreviousBrush | NextBrush | DuplicateBrush => {
            Destination::Panel(Tool::Library)
        }
        NewBrush(_) => Destination::Menu(BuiltinMenu::Brushes),
        LoadAlpha | ToggleStencil | AttachStencil | RemoveStencil | AlphaSoft | AlphaHard
        | AlphaSquare => Destination::Panel(Tool::Alpha),
        SavePreset | LoadPreset | ExportLibrary | ImportLibrary => {
            Destination::Panel(Tool::Presets)
        }
        BrushSize | BrushStrength | BrushInnerRadius | SurfaceFootprint | StrokeMode | BrushAdd
        | BrushSubtract | Smaller | Larger | BrushIcon | CaptureBrushIcon | RestoreBrushIcon
        | ResetBrushSettings | StrokeFreehand | StrokeLine | StrokeDragRect | StrokeDragDot
        | StrokeStabilisation => Destination::Panel(Tool::Brush),
        ModelSavePreset | ModelPreset(_) | ModelDeletePreset(_) | ModelTopology(_)
        | ModelCreateGroup | ModelAddToGroup | ModelRemoveFromGroup | ModelRepeat
        | ModelProtectMask | ModelSmoothPreview | ModelTapRepeat | ModelApply | ModelBoundary
        | ModelClearSelection | ModelDissolveLoops | ModelNewMesh | ModelJoin | ModelFewerLoops
        | ModelMoreLoops | ModelSelectLoop | ModelInteraction(_) | ModelSelect | ModelExtrude
        | ModelLoopCut | ModelReplace | ModelAdd | ModelRemove | ModelSelectSingle
        | ModelSelectLoops | ModelSelectRing | ModelAxis(_) => Destination::Panel(Tool::Mesh),
        PenModeling => Destination::Panel(Tool::Mesh),
        ViewportContextMenu | ModelFaces | ModelEdges | ModelVertices | ModelTool(_)
        | ModelTarget(_) => Destination::Menu(BuiltinMenu::Modeling),
        _ => Destination::Panel(Tool::Mesh),
    }
}
fn action_section(action: &Action) -> Option<&'static str> {
    match action {
        Action::StrokeMode
        | Action::StrokeFreehand
        | Action::StrokeLine
        | Action::StrokeDragRect
        | Action::StrokeDragDot
        | Action::StrokeStabilisation => Some("brush_stroke_settings"),
        Action::CaptureBrushIcon
        | Action::RestoreBrushIcon
        | Action::ResetBrushSettings
        | Action::BrushIcon => Some("brush_preset_settings"),
        Action::SurfaceFootprint | Action::BrushInnerRadius => Some("brush_shape_settings"),
        Action::DuplicateBrush => Some("brush_library_management"),
        Action::ModelSavePreset | Action::ModelPreset(_) | Action::ModelDeletePreset(_) => {
            Some("model_presets")
        }
        Action::ModelTopology(_) => Some("model_topology"),
        Action::ModelCreateGroup | Action::ModelAddToGroup | Action::ModelRemoveFromGroup => {
            Some("model_groups")
        }
        Action::CaptureObjectBrush => Some("object_capture"),
        _ => None,
    }
}
fn action_aliases(action: &Action) -> &'static str {
    match action {
        Action::Slice => "cut trim bisect split slice",
        Action::Search => "find command palette locate help",
        Action::ToggleZen => "focus fullscreen distraction free tab",
        Action::Save | Action::SaveAs => "write project file save",
        Action::Open => "load project file",
        Action::New => "start sphere project",
        Action::Undo => "back revert mistake",
        Action::Redo => "repeat undo",
        Action::ResetView => "frame fit focus camera home",
        Action::PenModeling => "modelling modeling components polygons",
        Action::ModelVertices => "vertex vertices verts points",
        Action::ModelEdges => "edge edges lines",
        Action::ModelFaces => "faces polygons quads triangles",
        Action::KeySettings => "keyboard keybinding hotkeys shortcuts bindings",
        Action::CustomizeWorkspace => {
            "customise customize layout panels shelves menus appearance colours recover reset"
        }
        Action::GuidedRetopology => "retopo topology quads remesh",
        Action::BrushSize => "radius diameter brush size",
        Action::BrushStrength => "intensity power brush strength",
        Action::OpenColours => "color colors colour palette paint",
        Action::OpenMaterials => "materials surface paint shader",
        Action::OpenTextures | Action::OpenUv => "uv unwrap texture image maps painting",
        Action::SelectBrush(_) => "brush preset sculpt",
        Action::ShowPanel(_) => "panel hidden restore show",
        _ => "",
    }
}
fn action_description(action: &Action) -> String {
    match action {
        Action::Save => {
            "Save the current project, including its models and embedded assets.".into()
        }
        Action::SaveAs => "Save the current project to another file.".into(),
        Action::Open => {
            "Choose a Clay Form project to open. Unsaved edits keep their normal confirmation."
                .into()
        }
        Action::New => {
            "Start a new sculpting project. Unsaved edits keep their normal confirmation.".into()
        }
        Action::Undo => "Undo the most recent edit in the current project.".into(),
        Action::Redo => "Restore the most recently undone edit.".into(),
        Action::SelectBrush(_) => "Make this brush the active preset for the next stroke.".into(),
        Action::Slice => "Choose a slice option, then draw a cut across the active mesh.".into(),
        Action::ShowPanel(tool) => {
            format!("Show {} without changing the active tool.", tool.label())
        }
        Action::OpenMenu(_) => {
            "Open this custom menu with its current controls and settings.".into()
        }
        Action::ModelPreset(_) => "Select this saved modelling tool configuration.".into(),
        action if super::super::brush_controls::is_control(action) => {
            "Reveal this active-brush control. Adjusting it changes future strokes.".into()
        }
        _ => format!(
            "{} using the same selection, preview and undo rules as its standard control.",
            action.label()
        ),
    }
}
const MENUS: &[BuiltinMenu] = &[
    BuiltinMenu::Materials,
    BuiltinMenu::Colours,
    BuiltinMenu::Textures,
    BuiltinMenu::References,
    BuiltinMenu::Brushes,
    BuiltinMenu::Transform,
    BuiltinMenu::Slice,
    BuiltinMenu::Primitive,
    BuiltinMenu::Edit,
    BuiltinMenu::Settings,
    BuiltinMenu::Visibility,
    BuiltinMenu::Mesh,
    BuiltinMenu::Submodels,
    BuiltinMenu::Wireframe,
    BuiltinMenu::Symmetry,
    BuiltinMenu::Mask,
    BuiltinMenu::View,
    BuiltinMenu::Modeling,
];

impl UiState {
    pub(crate) fn search_saved_paths(
        &mut self,
        ui: &mut egui::Ui,
        document: &Document,
        commands: &mut Vec<Command>,
    ) {
        if document.paths().is_empty() {
            return;
        }
        egui::CollapsingHeader::new("Saved paths")
            .id_salt("search_saved_paths")
            .default_open(false)
            .open(requested_section(ui.ctx(), "saved_paths"))
            .show(ui, |ui| {
                ui.weak("Project paths. Browsing keeps your active model and tool unchanged.");
                egui::ScrollArea::vertical()
                    .id_salt("saved_path_names")
                    .max_height(160.)
                    .show(ui, |ui| {
                        for path in document.paths() {
                            let owner = (0..document.model_count())
                                .filter_map(|index| document.model(index))
                                .find(|model| model.metadata.model_id == path.target)
                                .map_or("Unavailable model", |model| model.name);
                            let response = ui.label(format!("{} · {}", path.name, owner));
                            highlight_response(&response, &format!("path:{}", path.id));
                        }
                    });
                let action = Action::EditPaths;
                let reason = self.action_unavailable(&action, document);
                let response = ui.add_enabled(
                    reason.is_none(),
                    egui::Button::new("Edit paths on active model"),
                );
                let response = if let Some(reason) = reason {
                    response.on_disabled_hover_text(reason)
                } else {
                    response
                };
                if response.clicked() {
                    commands.push(Command::Action(action));
                }
            });
    }
}

impl UiState {
    pub(crate) fn search_object_library(
        &mut self,
        ui: &mut egui::Ui,
        document: &Document,
        commands: &mut Vec<Command>,
    ) {
        egui::CollapsingHeader::new("Reusable models and personal brushes")
            .id_salt("reusable_models")
            .default_open(false)
            .open(requested_section(ui.ctx(), "reusable_models"))
            .show(ui, |ui| {
                ui.weak("Browse saved items without changing the active brush.");
                if ui.button("Refresh personal library").clicked() {
                    commands.push(Command::Instances(
                        crate::command::InstanceCommand::ListPersonalBrushes,
                    ));
                }
                egui::ScrollArea::vertical()
                    .id_salt("reusable_model_names")
                    .max_height(160.)
                    .show(ui, |ui| {
                        for asset in &document.instances().assets {
                            let response = ui.label(&asset.name);
                            highlight_response(&response, &format!("object-asset:{}", asset.id));
                        }
                        for name in &self.instances.personal_brushes {
                            ui.horizontal_wrapped(|ui| {
                                let response = ui.label(name);
                                highlight_response(&response, &format!("personal-brush:{name}"));
                                if !document.instances().brushes.iter().any(|b| b.name == *name)
                                    && ui.button("Add to project").clicked()
                                {
                                    commands.push(Command::Instances(
                                        crate::command::InstanceCommand::LoadPersonalBrush(
                                            name.clone(),
                                        ),
                                    ));
                                }
                            });
                        }
                    });
                if document.instances().assets.is_empty()
                    && self.instances.personal_brushes.is_empty()
                {
                    ui.weak("No saved reusable models or personal brushes yet.");
                }
            });
    }
}
