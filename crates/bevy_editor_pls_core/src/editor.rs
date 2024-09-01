use std::any::{Any, TypeId};
use std::collections::HashSet;
use bevy::window::WindowMode;
use bevy::{prelude::*, utils::HashMap};
use bevy_inspector_egui::bevy_egui::{egui, EguiContext};
use bevy_inspector_egui::egui::{Context, Ui};
use egui_dock::{NodeIndex, SurfaceIndex, TabBarStyle, TabIndex};
use egui_dock::egui::{PointerButton};
use indexmap::IndexMap;
use crate::editor_inputs::EditorPointerState;
use crate::editor_window::{EditorWindow, EditorWindowContext};

#[non_exhaustive]
#[derive(Event)]
pub enum EditorEvent {
    Toggle { now_active: bool },
    FocusSelected,
}

#[derive(Debug)]
enum ActiveEditorInteraction {
    Viewport,
    Editor,
}

#[derive(Resource)]
pub struct Editor {
    on_window: Entity,
    always_active: bool,

    active: bool,

    pointer_used: bool,
    active_editor_interaction: Option<ActiveEditorInteraction>,
    listening_for_text: bool,
    viewport: egui::Rect,

    windows: IndexMap<TypeId, EditorWindowData>,
    window_states: HashMap<TypeId, EditorWindowState>,

    pub pointer_state: EditorPointerState,
}

impl Editor {
    pub fn new(on_window: Entity, always_active: bool) -> Self {
        Editor {
            on_window,
            always_active,

            active: always_active,
            pointer_used: false,
            active_editor_interaction: None,
            listening_for_text: false,
            viewport: egui::Rect::from_min_size(egui::Pos2::ZERO, egui::Vec2::new(640., 480.)),

            windows: IndexMap::default(),
            window_states: HashMap::default(),
            pointer_state: EditorPointerState::default(),
        }
    }

    pub fn window(&self) -> Entity {
        self.on_window
    }
    pub fn always_active(&self) -> bool {
        self.always_active
    }
    pub fn active(&self) -> bool {
        self.active
    }

    /// Panics if `self.always_active` is true
    pub fn set_active(&mut self, active: bool) {
        if !active && self.always_active {
            warn!("cannot call set_active on always-active editor");
        }

        self.active = active;
    }

    pub fn viewport(&self) -> egui::Rect {
        self.viewport
    }
    pub fn is_in_viewport(&self, pos: egui::Pos2) -> bool {
        self.viewport.contains(pos)
    }

    pub fn pointer_used(&self) -> bool {
        self.pointer_used
            || matches!(
                self.active_editor_interaction,
                Some(ActiveEditorInteraction::Editor)
            )
    }

    pub fn listening_for_text(&self) -> bool {
        self.listening_for_text
    }

    pub fn viewport_interaction_active(&self) -> bool {
        !self.pointer_used
            || matches!(
                self.active_editor_interaction,
                Some(ActiveEditorInteraction::Viewport)
            )
    }

    pub(crate) fn extract_viewport_pointer_pos(&mut self, ui: &mut Ui){
        let Some(cursor_pos) = ui.input(|input| input.pointer.latest_pos()) else {
            self.pointer_state.viewport_pointer_pos = None;
            return;
        };

        let local_cursor_pos = (cursor_pos - ui.min_rect().min).to_pos2();
        self.pointer_state.viewport_pointer_pos = Some(local_cursor_pos);
    }
}

pub(crate) type UiFn =
    Box<dyn Fn(&mut World, EditorWindowContext, &mut egui::Ui) + Send + Sync + 'static>;
pub(crate) type EditorWindowState = Box<dyn Any + Send + Sync>;

struct EditorWindowData {
    name: &'static str,
    ui_fn: UiFn,
    menu_ui_fn: UiFn,
    menu_bar_ui_fn: UiFn,
    /// Ui function execution order among other [`EditorWindow`]s
    menu_bar_order: usize,
    viewport_toolbar_ui_fn: UiFn,
    viewport_ui_fn: UiFn,
    default_size: (f32, f32),
}

#[derive(Resource)]
pub struct EditorInternalState {
    state: egui_dock::DockState<TreeTab>,
    pub(crate) floating_windows: Vec<FloatingWindow>,

    next_floating_window_id: u32,

    /// Contains all closed floating windows during current redraw
    closed_floating_windows: HashSet<TypeId>
}

impl Default for EditorInternalState {
    fn default() -> Self {
        Self {
            state: egui_dock::DockState::new(vec![TreeTab::GameView]),
            floating_windows: Default::default(),
            next_floating_window_id: Default::default(),
            closed_floating_windows: Default::default(),
        }
    }
}

#[derive(Copy, Clone)]
enum TreeTab {
    GameView,
    CustomWindow(TypeId),
}

impl EditorInternalState {
    pub fn push_to_focused_leaf<W: EditorWindow>(&mut self) {
        self.state
            .push_to_focused_leaf(TreeTab::CustomWindow(TypeId::of::<W>()));
        if let Some((surface_index, node_index)) = self.state.focused_leaf() {
            self.state
                .set_active_tab((surface_index, node_index, TabIndex(0)));
        };
    }

    pub fn split<W: EditorWindow>(
        &mut self,
        parent: NodeIndex,
        split: egui_dock::Split,
        fraction: f32,
    ) -> [NodeIndex; 2] {
        let node = egui_dock::Node::leaf(TreeTab::CustomWindow(TypeId::of::<W>()));
        self.state
            .split((SurfaceIndex::main(), parent), split, fraction, node)
    }

    pub fn split_right<W: EditorWindow>(
        &mut self,
        parent: NodeIndex,
        fraction: f32,
    ) -> [NodeIndex; 2] {
        self.split::<W>(parent, egui_dock::Split::Right, fraction)
    }
    pub fn split_left<W: EditorWindow>(
        &mut self,
        parent: NodeIndex,
        fraction: f32,
    ) -> [NodeIndex; 2] {
        self.split::<W>(parent, egui_dock::Split::Left, fraction)
    }
    pub fn split_above<W: EditorWindow>(
        &mut self,
        parent: NodeIndex,
        fraction: f32,
    ) -> [NodeIndex; 2] {
        self.split::<W>(parent, egui_dock::Split::Above, fraction)
    }
    pub fn split_below<W: EditorWindow>(
        &mut self,
        parent: NodeIndex,
        fraction: f32,
    ) -> [NodeIndex; 2] {
        self.split::<W>(parent, egui_dock::Split::Below, fraction)
    }

    pub fn split_many(
        &mut self,
        parent: NodeIndex,
        fraction: f32,
        split: egui_dock::Split,
        windows: &[TypeId],
    ) -> [NodeIndex; 2] {
        let tabs = windows.iter().copied().map(TreeTab::CustomWindow).collect();
        let node = egui_dock::Node::leaf_with(tabs);
        self.state
            .split((SurfaceIndex::main(), parent), split, fraction, node)
    }

    /// Determines if floating window of type ['W'] was added prior
    pub fn has_floating_window<W: 'static>(&self) -> bool {
        let floating_windows = &self.floating_windows;

        for window in floating_windows.iter() {
            if window.window == TypeId::of::<W>() {
                return true;
            }
        }

        false
    }

    /// Determines if floating window of type [`W`] was closed during last
    /// redraw / frame
    pub fn closed_floating_window<W: 'static>(&self) -> bool {
        let test_type = TypeId::of::<W>();
        self.closed_floating_windows.contains(&test_type)
    }
}

#[derive(Clone)]
pub(crate) struct FloatingWindow {
    pub(crate) window: TypeId,
    pub(crate) id: u32,
    pub(crate) initial_position: Option<egui::Pos2>,
    pub current_rect: egui::Rect
}

impl EditorInternalState {
    pub(crate) fn next_floating_window_id(&mut self) -> u32 {
        let id = self.next_floating_window_id;
        self.next_floating_window_id += 1;
        id
    }
}

fn ui_fn<W: EditorWindow>(world: &mut World, cx: EditorWindowContext, ui: &mut egui::Ui) {
    W::ui(world, cx, ui);
}
fn menu_ui_fn<W: EditorWindow>(world: &mut World, cx: EditorWindowContext, ui: &mut egui::Ui) {
    W::menu_ui(world, cx, ui);
}
fn menu_bar_ui_fn<W: EditorWindow>(world: &mut World, cx: EditorWindowContext, ui: &mut egui::Ui) {
    W::menu_bar_ui(world, cx, ui);
}
fn viewport_toolbar_ui_fn<W: EditorWindow>(
    world: &mut World,
    cx: EditorWindowContext,
    ui: &mut egui::Ui,
) {
    W::viewport_toolbar_ui(world, cx, ui);
}
fn viewport_ui_fn<W: EditorWindow>(world: &mut World, cx: EditorWindowContext, ui: &mut egui::Ui) {
    W::viewport_ui(world, cx, ui);
}

impl Editor {
    pub fn add_window<W: EditorWindow>(&mut self) {
        let type_id = TypeId::of::<W>();
        let ui_fn = Box::new(ui_fn::<W>);
        let menu_ui_fn = Box::new(menu_ui_fn::<W>);
        let menu_bar_ui_fn = Box::new(menu_bar_ui_fn::<W>);
        let viewport_toolbar_ui_fn = Box::new(viewport_toolbar_ui_fn::<W>);
        let viewport_ui_fn = Box::new(viewport_ui_fn::<W>);
        let data = EditorWindowData {
            ui_fn,
            menu_ui_fn,
            menu_bar_ui_fn,
            menu_bar_order: W::menu_bar_order(),
            viewport_toolbar_ui_fn,
            viewport_ui_fn,
            name: W::NAME,
            default_size: W::DEFAULT_SIZE,
        };
        if self.windows.insert(type_id, data).is_some() {
            panic!(
                "window of type {} already inserted",
                std::any::type_name::<W>()
            );
        }
        self.window_states
            .insert(type_id, Box::<<W as EditorWindow>::State>::default());
    }

    pub fn window_state_mut<W: EditorWindow>(&mut self) -> Option<&mut W::State> {
        self.window_states
            .get_mut(&TypeId::of::<W>())
            .and_then(|s| s.downcast_mut::<W::State>())
    }
    pub fn window_state<W: EditorWindow>(&self) -> Option<&W::State> {
        self.window_states
            .get(&TypeId::of::<W>())
            .and_then(|s| s.downcast_ref::<W::State>())
    }
}

impl Editor {
    pub(crate) fn system(world: &mut World) {
        world.resource_scope(|world, mut editor: Mut<Editor>| {
            let Ok(mut egui_context) = world
                .query::<&mut EguiContext>()
                .get_mut(world, editor.on_window)
            else {
                return;
            };
            let egui_context = egui_context.get_mut().clone();

            world.resource_scope(
                |world, mut editor_internal_state: Mut<EditorInternalState>| {
                    world.resource_scope(|world, mut editor_events: Mut<Events<EditorEvent>>| {
                        editor.editor_ui(
                            world,
                            &egui_context,
                            &mut editor_internal_state,
                            &mut editor_events,
                        );
                    });
                },
            );
        });
    }

    fn editor_ui(
        &mut self,
        world: &mut World,
        ctx: &egui::Context,
        internal_state: &mut EditorInternalState,
        editor_events: &mut Events<EditorEvent>,
    ) {
        self.editor_menu_bar(world, ctx, internal_state, editor_events);

        if !self.active {
            self.editor_floating_windows(world, ctx, internal_state);
            self.pointer_used = ctx.wants_pointer_input();
            return;
        }

        let mut tree = std::mem::replace(
            &mut internal_state.state,
            egui_dock::DockState::new(Vec::new()),
        );

        egui_dock::DockArea::new(&mut tree)
            .style(egui_dock::Style {
                tab_bar: TabBarStyle {
                    bg_fill: ctx.style().visuals.window_fill(),
                    ..default()
                },
                ..egui_dock::Style::from_egui(ctx.style().as_ref())
            })
            .show(
                ctx,
                &mut TabViewer {
                    editor: self,
                    internal_state,
                    world,
                },
            );
        internal_state.state = tree;

        let pointer_pos = ctx.input(|input| input.pointer.interact_pos());
        self.pointer_used = pointer_pos.map_or(false, |pos| !self.is_in_viewport(pos));
        self.editor_floating_windows(world, ctx, internal_state);

        self.setup_input_state(ctx, &internal_state);

        self.listening_for_text = ctx.wants_keyboard_input();

        let is_pressed = ctx.input(|input| input.pointer.press_start_time().is_some());
        match (&self.active_editor_interaction, is_pressed) {
            (_, false) => self.active_editor_interaction = None,
            (None, true) => {
                self.active_editor_interaction = Some(match self.pointer_used {
                    true => ActiveEditorInteraction::Editor,
                    false => ActiveEditorInteraction::Viewport,
                });
            }
            (Some(_), true) => {}
        }
    }

    fn setup_input_state(&mut self, ctx: &Context, internal_state: &EditorInternalState) {
        let is_pointer_pressed = ctx.input(|input| input.pointer.button_pressed(PointerButton::Primary));
        let is_pointer_held = ctx.input(|input| input.pointer.button_down(PointerButton::Primary));

        let pointer_state = &mut self.pointer_state;
        pointer_state.press_active = is_pointer_held;

        self.setup_input_viewport_state(ctx, is_pointer_pressed, is_pointer_held, internal_state);
    }

    fn setup_input_viewport_state(&mut self,
                                  ctx: &Context,
                                  is_pointer_pressed: bool,
                                  is_pointer_held: bool,
                                  internal_state: &EditorInternalState
    ) {
        let pointer_pos = ctx.input(|input| input.pointer.interact_pos());
        let pointer_in_viewport = pointer_pos.map_or(false, |pos| self.is_in_viewport(pos));

        let mut pointer_inside_floating_window = false;

        // Resolve clicks on top of floating windows as non-viewport ones
        if let Some(position) = pointer_pos {
            let windows = &internal_state.floating_windows;

            for window in windows {
                let rect = window.current_rect;

                if rect.contains(position) {
                    pointer_inside_floating_window = true;
                    break;
                }
            }
        }

        // Discard previously read position.
        // Otherwise, will register outside viewport
        if !pointer_in_viewport || pointer_inside_floating_window {
            self.pointer_state.viewport_pointer_pos = None;
        }

        // Check if pointer is in viewport, in order to determine if
        // viewport should be altered during rendering
        if is_pointer_pressed {
            self.pointer_state.press_start_in_viewport = pointer_in_viewport && !pointer_inside_floating_window;
            return;
        }

        // Button has been released -> no need to perform actions in this case
        if !is_pointer_held {
            self.pointer_state.press_start_in_viewport = false;
        }
    }

    fn editor_menu_bar(
        &mut self,
        world: &mut World,
        ctx: &egui::Context,
        internal_state: &mut EditorInternalState,
        editor_events: &mut Events<EditorEvent>,
    ) {
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            let bar_response = egui::menu::bar(ui, |ui| {
                if !self.always_active && play_pause_button(self.active, ui).clicked() {
                    self.active = !self.active;
                    editor_events.send(EditorEvent::Toggle {
                        now_active: self.active,
                    });
                }

                ui.menu_button("Open window", |ui| {
                    for (&_, window) in self.windows.iter() {
                        let cx = EditorWindowContext {
                            window_states: &mut self.window_states,
                            internal_state,
                        };
                        (window.menu_ui_fn)(world, cx, ui);
                    }
                });

                self.draw_window_menu_items(world, internal_state, ui);
            })
            .response;
            // .interact(egui::Sense::click());

            if bar_response.double_clicked() {
                let mut window = world
                    .query::<&mut Window>()
                    .get_mut(world, self.on_window)
                    .unwrap();

                match window.mode {
                    WindowMode::Windowed => window.mode = WindowMode::BorderlessFullscreen,
                    _ => window.mode = WindowMode::Windowed,
                }
            }
        });
    }

    /// Performs drawing of the menu bar Ui per [`EditorWindow`] based on
    /// specified order of items
    fn draw_window_menu_items(&mut self,
                              world: &mut World,
                              internal_state: &mut EditorInternalState,
                              ui: &mut Ui,
    ) {
        let windows = &self.windows;

        // Collect and sort indices based on the window menu bar order
        let mut sorted_indices = windows
            .iter()
            .enumerate()
            .map(|(i, (_, window_data))| (i, window_data.menu_bar_order))
            .collect::<Vec<_>>();

        sorted_indices.sort_by_key(|&(_, order)| order);

        for (window_index, _) in sorted_indices {
            let cx = EditorWindowContext {
                window_states: &mut self.window_states,
                internal_state,
            };

            let window = &windows[window_index];
            (window.menu_bar_ui_fn)(world, cx, ui);
        }
    }

    fn editor_window_inner(
        &mut self,
        world: &mut World,
        internal_state: &mut EditorInternalState,
        selected: TypeId,
        ui: &mut egui::Ui,
    ) {
        let cx = EditorWindowContext {
            window_states: &mut self.window_states,
            internal_state,
        };
        let ui_fn = &self.windows.get_mut(&selected).unwrap().ui_fn;
        ui_fn(world, cx, ui);
    }

    fn editor_window_context_menu(
        &mut self,
        ui: &mut egui::Ui,
        internal_state: &mut EditorInternalState,
        tab: TreeTab,
    ) {
        if ui.button("Pop out").clicked() {
            if let TreeTab::CustomWindow(window) = tab {
                let id = internal_state.next_floating_window_id();
                internal_state.floating_windows.push(FloatingWindow {
                    window,
                    id,
                    initial_position: None,
                    current_rect: egui::Rect::ZERO  // Read later on
                });
            }

            ui.close_menu();
        }
    }

    fn editor_floating_windows(
        &mut self,
        world: &mut World,
        ctx: &Context,
        state: &mut EditorInternalState,
    ) {
        let mut close_floating_windows = Vec::new();
        let mut floating_windows = state.floating_windows.clone();

        state.closed_floating_windows.clear();

        for (i, fl_window) in floating_windows.iter_mut().enumerate() {
            let id = egui::Id::new(fl_window.id);
            let title = self.windows[&fl_window.window].name;

            let mut open = true;
            let default_size = self.windows[&fl_window.window].default_size;
            let mut window = egui::Window::new(title)
                .id(id)
                .open(&mut open)
                .resizable(true)
                .default_size(default_size);
            if let Some(initial_position) = fl_window.initial_position {
                window = window.default_pos(initial_position - egui::Vec2::new(10.0, 10.0))
            }

            let opt_response = window.show(ctx, |ui| {
                self.editor_window_inner(world, state, fl_window.window, ui);
                let desired_size = (ui.available_size() - (5.0, 5.0).into()).max((0.0, 0.0).into());
                ui.allocate_space(desired_size);
            });

            if let Some(response) = opt_response {
                fl_window.current_rect = response.response.rect;
            }

            if !open {
                close_floating_windows.push(i);
                state.closed_floating_windows.insert(fl_window.window);
            }
        }

        let original_windows = &mut state.floating_windows;

        // Update read values from the floating window copy
        for (i, window) in original_windows.into_iter().enumerate() {
            window.current_rect = floating_windows[i].current_rect;
        }

        for &to_remove in close_floating_windows.iter().rev() {
            original_windows.swap_remove(to_remove);
        }
    }

    fn editor_viewport_toolbar_ui(
        &mut self,
        world: &mut World,
        ui: &mut egui::Ui,
        internal_state: &mut EditorInternalState,
    ) {
        for (_, window) in self.windows.iter() {
            let cx = EditorWindowContext {
                window_states: &mut self.window_states,
                internal_state,
            };

            (window.viewport_toolbar_ui_fn)(world, cx, ui);
        }
    }

    fn editor_viewport_ui(
        &mut self,
        world: &mut World,
        ui: &mut egui::Ui,
        internal_state: &mut EditorInternalState,
    ) {
        for (_, window) in self.windows.iter() {
            let cx = EditorWindowContext {
                window_states: &mut self.window_states,
                internal_state,
            };

            (window.viewport_ui_fn)(world, cx, ui);
        }
    }
}

struct TabViewer<'a> {
    editor: &'a mut Editor,
    internal_state: &'a mut EditorInternalState,
    world: &'a mut World,
}
impl egui_dock::TabViewer for TabViewer<'_> {
    type Tab = TreeTab;

    fn ui(&mut self, ui: &mut egui::Ui, tab: &mut Self::Tab) {
        match *tab {
            TreeTab::GameView => {
                let viewport = ui.clip_rect();

                ui.horizontal(|ui| {
                    ui.style_mut().spacing.button_padding = egui::vec2(2.0, 0.0);
                    let height = ui.spacing().interact_size.y;
                    ui.set_min_size(egui::vec2(ui.available_width(), height));

                    self.editor
                        .editor_viewport_toolbar_ui(self.world, ui, self.internal_state);
                });

                self.editor.viewport = viewport;
                self.editor.extract_viewport_pointer_pos(ui);

                self.editor
                    .editor_viewport_ui(self.world, ui, self.internal_state);
            }
            TreeTab::CustomWindow(window_id) => {
                self.editor
                    .editor_window_inner(self.world, self.internal_state, window_id, ui);
            }
        }
    }

    fn context_menu(
        &mut self,
        ui: &mut egui::Ui,
        tab: &mut Self::Tab,
        _surface: SurfaceIndex,
        _node: NodeIndex,
    ) {
        self.editor
            .editor_window_context_menu(ui, self.internal_state, *tab);
    }

    fn title(&mut self, tab: &mut Self::Tab) -> egui::WidgetText {
        match *tab {
            TreeTab::GameView => "Viewport".into(),
            TreeTab::CustomWindow(window_id) => {
                self.editor.windows.get(&window_id).unwrap().name.into()
            }
        }
    }

    fn clear_background(&self, tab: &Self::Tab) -> bool {
        !matches!(tab, TreeTab::GameView)
    }
}

fn play_pause_button(active: bool, ui: &mut egui::Ui) -> egui::Response {
    let icon = match active {
        true => "▶",
        false => "⏸",
    };
    ui.add(egui::Button::new(icon).frame(false))
}
