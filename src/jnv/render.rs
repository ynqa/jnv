use promkit::{
    impl_as_any, impl_cast, json, keymap::KeymapManager, listbox, pane::Pane, snapshot::Snapshot,
    suggest::Suggest, text, text_editor,
};

pub struct Renderer {
    pub keymap: KeymapManager<Self>,
    pub query_editor_snapshot: Snapshot<text_editor::Renderer>,
    pub hint_message_snapshot: Snapshot<text::Renderer>,
    pub suggest: Suggest,
    pub suggest_snapshot: Snapshot<listbox::Renderer>,
    pub json_bundle_snapshot: Snapshot<json::bundle::Renderer>,
}

impl_as_any!(Renderer);
impl_cast!(Renderer);

impl promkit::Renderer for Renderer {
    fn create_panes(&self, width: u16) -> Vec<Pane> {
        let mut panes = Vec::new();
        panes.extend(self.query_editor_snapshot.create_panes(width));
        panes.extend(self.hint_message_snapshot.create_panes(width));
        panes.extend(self.suggest_snapshot.create_panes(width));
        panes.extend(self.json_bundle_snapshot.create_panes(width));
        panes
    }
}
