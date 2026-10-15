//! The camp's Party screens (camp-08 spec): one card per party member with
//! its 3D model above a name box (state 0x14: FUN_140320b30 /
//! FUN_14031fbb0), the second pick and swap (0x16: FUN_14031f9a0). Sora
//! (card 0) is never picked; with fewer than three members the cards
//! cannot move (no hand, LightCur or marker) and the help says so. The
//! event-driven notice (0x15), locked members, world guests and writing the
//! party table back are not built.
use super::*;

const PRIO_PARTY_PANEL: u32 = 0x753d;
const PRIO_CARD: u32 = 0x753e;
const PRIO_MARKER: u32 = 0x753f;
/// Tz::MenuObj in Party (FUN_1402ffba0(M, 0x7545)).
const PRIO_PARTY_MODEL: u32 = 0x7545;
const PANEL: [usize; 3] = [312, 313, 314];
/// Cards: slots 0–2, the reserve slot 3.
const CARD: [usize; 2] = [315, 316];
const MARKER: usize = 317;
/// Name boxes: Sora / locked, switchable.
const NAME: [usize; 2] = [318, 319];

/// The screens' sprites and lists.
#[derive(Default)]
pub(super) struct Ui {
    panel: Option<Id>,
    pub(super) cards: MenuList,
    pub(super) swap: MenuList,
    names: Vec<Id>,
    marker: Option<Id>,
}

impl Camp {
    /// The name cards' model pastes (party-spec §4): member i's render on
    /// card i at the card's context, with the card's alpha.
    pub(super) fn party_models(&self) -> Option<ModelLayer> {
        if !matches!(self.state, State::Party | State::PartySwap) {
            return None;
        }
        let panel_alpha = self
            .party_ui
            .panel
            .filter(|&p| self.sprites.alive(p))
            .map(|p| self.sprites.get(p).ctx_color[3])?;
        let members: Vec<_> = self
            .party_ui
            .names
            .iter()
            .enumerate()
            .filter(|&(_, &n)| self.sprites.alive(n))
            .map(|(i, &n)| {
                let c = self.sprites.context(n);
                let a = self.sprites.get(n).ctx_color[3] * panel_alpha;
                (i, [c[0] - 256., c[1] - 208.], a)
            })
            .collect();
        (!members.is_empty()).then_some((PRIO_PARTY_MODEL, members))
    }

    /// FUN_140320b30: tab 0, the panel, a card and a name box per member,
    /// the models, list 1 (LEFT / RIGHT) and the help.
    pub(super) fn enter_party(&mut self, seq: &Sequence) {
        self.state = State::Party;
        self.substate = 0;
        self.create_tab(seq, 0);
        let panel = self
            .sprites
            .create(seq, PRIO_PARTY_PANEL, ART, PANEL[0], Some(PANEL[1]), None);
        self.closers.push((panel, PANEL[2]));
        self.party_ui.panel = Some(panel);
        let names: Vec<Vec<u8>> = self
            .data
            .panels
            .members
            .iter()
            .map(|m| m.name.clone())
            .collect();
        let n = names.len().min(4);
        let card_pitch = seq.groups[CARD[0]].text_position_x as f32;
        let name_pitch = seq.groups[NAME[0]].text_position_x as f32;
        let mut cards = Vec::new();
        self.party_ui.names.clear();
        for (i, name) in names.into_iter().take(n).enumerate() {
            let g = CARD[usize::from(i == 3)];
            let r = self.sprites.create(seq, PRIO_CARD, ART, g, None, None);
            self.child(r, panel);
            self.sprites.get_mut(r).offset = [card_pitch * i as f32, 0.];
            cards.push(r);
            let mut t = Text::new(name, TEXT_SIZE);
            t.align = 2;
            let nb = self.sprites.create(
                seq,
                PRIO_CARD,
                ART,
                NAME[usize::from(i != 0)],
                None,
                Some(t),
            );
            self.child(nb, panel);
            self.sprites.get_mut(nb).offset = [name_pitch * i as f32, 0.];
            self.party_ui.names.push(nb);
        }
        // Sora (card 0) is skipped; the cursor starts on the first other.
        let cursor = usize::from(n > 1);
        let mut list = MenuList::new(cards.clone(), cursor);
        list.keys = Some((bits::LEFT, bits::RIGHT));
        list.skip = 1;
        let movable = n >= 3;
        if !movable {
            list.mask &= !0x13;
        }
        self.party_ui.cards = list;
        let mut swap = MenuList::new(cards, 0);
        swap.keys = Some((bits::LEFT, bits::RIGHT));
        self.party_ui.swap = swap;
        self.item_ui.no_pointer = !movable;
        self.model_time = Some(0.);
        let h = self.data.party_help[usize::from(movable)].clone();
        self.show_help(seq, h);
    }

    /// FUN_140321420: the pulsing marker on the active hand's card.
    fn party_marker(&mut self, seq: &Sequence) {
        let card = self
            .glove
            .filter(|&g| self.sprites.alive(g))
            .and_then(|g| self.sprites.get(g).parent);
        let Some(card) = card else {
            if let Some(m) = self.party_ui.marker.take() {
                self.sprites.kill(m);
            }
            return;
        };
        let offset = self.sprites.get(card).offset;
        let m = match self.party_ui.marker {
            Some(m) if self.sprites.alive(m) => m,
            _ => {
                let m = self
                    .sprites
                    .create(seq, PRIO_MARKER, ART, MARKER, None, None);
                self.child(m, self.party_ui.panel.unwrap());
                self.party_ui.marker = Some(m);
                m
            }
        };
        self.sprites.get_mut(m).offset = offset;
    }

    /// FUN_14031fbb0.
    pub(super) fn party(&mut self, seq: &Sequence, pad: &Pad, sounds: &mut Vec<u32>) -> bool {
        let (pick, moved) = self.party_ui.cards.input(pad);
        if self.substate == 2 {
            let dead = !self.alive_opt(self.party_ui.panel) && !self.tab_alive(0);
            if dead {
                self.item_ui.no_pointer = false;
                self.pending = None;
                self.return_top(seq);
            } else if pad.trig != 0 {
                self.kill_opt(self.party_ui.panel);
                self.kill_tab(0);
            }
            return false;
        }
        if (pick != Pick::None || moved) && self.group_of(self.party_ui.panel) != Some(PANEL[1]) {
            if let Some(p) = self.party_ui.panel {
                self.sprites.set(seq, p, PANEL[1], None);
            }
            if let Some(tab) = &self.tabs[0]
                && self.sprites.alive(tab.id)
            {
                let id = tab.id;
                self.sprites.set(seq, id, TAB[1], None);
            }
        }
        if self.substate == 0 && self.group_of(self.party_ui.panel) == Some(PANEL[1]) {
            self.substate = 1;
        }
        if let Some(s) = pick_sound(pick, moved, true) {
            sounds.push(s);
        }
        match pick {
            Pick::Start => self.exit = true,
            Pick::Cancel => {
                self.hide_help(seq);
                self.close_tab(seq, 0);
                self.close_opt(seq, self.party_ui.panel, PANEL[2]);
                self.pending = Some(State::Top);
                self.substate = 2;
            }
            Pick::Row(a) => {
                self.state = State::PartySwap;
                self.substate = 0;
                let n = self.party_ui.swap.total;
                self.party_ui.swap.skip = self.party_ui.cards.skip | 1 << a;
                let b = if a + 1 >= n || self.party_ui.swap.skip >> (a + 1) & 1 != 0 {
                    a.saturating_sub(1)
                } else {
                    a + 1
                };
                self.party_ui.swap.cursor = b;
                let h = self.data.party_help[2].clone();
                self.show_help(seq, h);
            }
            _ => {}
        }
        self.party_marker(seq);
        moved
    }

    /// FUN_14031f9a0: the second pick; deciding swaps the two members.
    pub(super) fn party_swap(&mut self, seq: &Sequence, pad: &Pad, sounds: &mut Vec<u32>) -> bool {
        let (pick, moved) = self.party_ui.swap.input(pad);
        if self.substate == 2 {
            self.state = State::Party;
            self.substate = 1;
            let movable = self.party_ui.cards.total >= 3;
            let h = self.data.party_help[usize::from(movable)].clone();
            self.show_help(seq, h);
            return false;
        }
        self.substate = 1;
        if let Some(s) = pick_sound(pick, moved, true) {
            sounds.push(s);
        }
        match pick {
            Pick::Start => self.exit = true,
            Pick::Cancel => self.substate = 2,
            Pick::Row(b) => {
                let a = self.party_ui.cards.cursor;
                self.data.panels.members.swap(a, b);
                let (na, nb) = (self.party_ui.names[a], self.party_ui.names[b]);
                let ta = self.sprites.get(na).text.as_ref().map(|t| t.bytes.clone());
                let tb = self.sprites.get(nb).text.as_ref().map(|t| t.bytes.clone());
                if let (Some(ta), Some(tb)) = (ta, tb) {
                    self.set_text(na, tb);
                    self.set_text(nb, ta);
                }
                self.substate = 2;
            }
            _ => {}
        }
        self.party_marker(seq);
        moved
    }
}
