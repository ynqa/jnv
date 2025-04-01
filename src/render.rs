use std::sync::LazyLock;

use promkit_core::{crossterm::cursor, pane::Pane, terminal::Terminal};

// TODO: One Guide is sufficient.
#[derive(Debug, PartialEq)]
pub enum PaneIndex {
    Editor = 0,
    Guide = 1,
    Search = 2,
    Processor = 3,
}

pub static EMPTY_PANE: LazyLock<Pane> = LazyLock::new(|| Pane::new(vec![], 0));
const PANE_SIZE: usize = PaneIndex::Processor as usize + 1;

pub struct Renderer {
    no_hint: bool,
    terminal: Terminal,
    panes: [Pane; PANE_SIZE],
}

impl Renderer {
    pub fn try_init_draw(init_panes: [Pane; PANE_SIZE], no_hint: bool) -> anyhow::Result<Self> {
        let mut ret = Self {
            no_hint,
            terminal: Terminal {
                position: cursor::position()?,
            },
            panes: init_panes,
        };
        ret.terminal.draw(&ret.panes)?;
        Ok(ret)
    }

    pub fn update_and_draw<I: IntoIterator<Item = (PaneIndex, Pane)>>(
        &mut self,
        iter: I,
    ) -> anyhow::Result<()> {
        for (index, pane) in iter {
            if self.no_hint && index == PaneIndex::Guide {
                continue;
            }
            self.panes[index as usize] = pane;
        }
        self.terminal.draw(&self.panes)?;
        Ok(())
    }
}
