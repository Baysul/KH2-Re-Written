//! The frame the category pages share: the sub-title pill with its orb
//! (FUN_140323c60 / FUN_140323f10 / FUN_140324230), the backdrop
//! (FUN_1402f2820) and the hand-over from Notes / Collection.
use super::*;

/// The pill's groups (seq5): in, idle, out; the orb is a LightCur.
const PILL: [usize; 3] = [135, 136, 137];
/// The backdrop's groups by origin (0x1405bab00): from Notes, from
/// Collection.
const BACKDROP: [usize; 2] = [32, 30];
const PRIO_BACKDROP: u32 = 0x755a;

/// The pill and the page's origin.
#[derive(Default)]
pub(super) struct Category {
    pub pill: Option<Id>,
    pub orb: LightCur,
    pub label: Vec<u8>,
    pub from_collection: bool,
}

/// A page a category row opens (the others are not built).
pub(super) fn is_category(state: u8) -> bool {
    !matches!(state, 0 | 1 | NOTES_PAGE | COLLECTION_PAGE)
}

impl Journal {
    /// FUN_140327330 / FUN_14032b210's decide: the label goes to the pill,
    /// the page slides out forward and the tabs and help go.
    pub(super) fn enter_category(&mut self, seq: &Sequence, state: u8, label: u16, row: usize) {
        self.cat.label = self.msg(label);
        self.cat.from_collection = self.state == COLLECTION_PAGE;
        self.slide(seq, 0, 1);
        // FUN_1402f3a70: the L / R tabs.
        self.close_if(seq, self.tabs[0], LEFT_TAB[2]);
        self.close_if(seq, self.tabs[1], RIGHT_TAB[2]);
        self.hide_help(seq);
        self.memory[self.state as usize] = row;
        self.pending = Some(state);
        self.substate = 2;
    }

    /// FUN_140323c60(0, -1, 0) and FUN_1402f2820: the pill if it is not
    /// alive, and the backdrop under the page.
    pub(super) fn category_frame(&mut self, seq: &Sequence) {
        self.frame_texts(seq);
        // The title stays; the L / R tabs are gone.
        if self.cat.pill.is_none_or(|p| !self.sprites.alive(p)) {
            let mut t = Self::text(self.cat.label.clone(), 15.);
            t.group_position = false;
            let p = self
                .sprites
                .create(seq, BASE, ART, PILL[0], Some(PILL[1]), Some(t));
            self.cat.pill = Some(p);
            let l = self
                .sprites
                .create(seq, BASE + 1, ART, LIGHT[0], Some(LIGHT[1]), None);
            self.cat.orb = LightCur::new(l);
        }
        let group = BACKDROP[self.cat.from_collection as usize];
        self.page_sprite(seq, PRIO_BACKDROP, group, self.container, None);
    }

    /// FUN_140323f10(0): the pill and its orb go out.
    pub(super) fn pill_out(&mut self, seq: &Sequence) {
        self.close_if(seq, self.cat.pill, PILL[2]);
        if let Some(l) = self.cat.orb.id
            && self.sprites.alive(l)
            && self.sprites.get(l).player.group != LIGHT[2]
        {
            self.sprites.close(seq, l, LIGHT[2]);
        }
    }

    /// FUN_140324230(0): instant.
    pub(super) fn kill_pill(&mut self) {
        if let Some(p) = self.cat.pill.take() {
            self.sprites.kill(p);
        }
        self.cat.orb.kill(&mut self.sprites);
    }

    /// The pill's orb sits at the pill's context + (LightPositionX,
    /// TextPositionY).
    pub(super) fn place_pill_orb(&mut self, seq: &Sequence) {
        let Some(p) = self.cat.pill else { return };
        if !self.sprites.alive(p) || self.cat.orb.id.is_none() {
            return;
        }
        let c = self.sprites.context(p);
        let g = &seq.groups[self.sprites.get(p).player.group];
        let t = [
            c[0] + g.light_position_x as f32,
            c[1] + g.text_position_y as f32,
        ];
        self.cat.orb.place(&mut self.sprites, t);
    }
}
