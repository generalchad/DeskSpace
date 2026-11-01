//! A space's title icon (ROADMAP D16): which icon set it needs, loading that
//! set off the main thread, asking before a download, and turning the icon's
//! glyph into pixels.
//!
//! A set is read once from the local cache (`IconStore`) on the first start
//! that needs it. If the cache has no file, `next_step` decides what happens
//! next: with `IconDownloads::Ask` the user is asked once, `Allowed` downloads
//! the signed set from DeskSpace's releases, and `Declined` leaves the icon
//! off. Nothing here runs on the main thread but the decision.
//!
//! Automatic icons match against the default set's words and names; the match
//! is memoized by title until that set changes.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, channel};

use deskspace_config::{ConfigStore, IconDownloads, IconSetSpec, IconStore, default_set, icon_set};
use deskspace_core::icon_match::best_icon;
#[cfg(test)]
use deskspace_core::icon_set::Glyph;
use deskspace_core::icon_set::IconSet;
use deskspace_core::theme::Color;
use deskspace_core::{Space, SpaceIcon};
use deskspace_platform::{GlyphCache, Icon, Ui};

use super::ICONS_WAKE;

/// What one set's cache holds, or why it cannot be used.
#[derive(Debug)]
pub(super) enum SetState {
    /// A worker is reading or downloading it.
    Loading,
    Ready(Arc<IconSet>),
    /// The cache has no file for it.
    Missing,
    Failed(String),
}

/// What to do about a set, decided in one place (S rule 2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum IconStep {
    /// Already loading, ready, or waiting on an answer.
    Nothing,
    /// Read the cache.
    Load,
    /// Ask the user once this session.
    Ask,
    /// Download the signed set, which the user allowed.
    Fetch,
    /// No icon can be had this session.
    Unavailable,
}

/// The next step for a set in `state`, with the user's consent and whether the
/// question already showed. Only `Allowed` downloads; a failure waits for the
/// next start. A build with no signing key (`can_download` false) cannot verify
/// a set, so it neither downloads nor asks, and no answer is saved for it.
pub(super) fn next_step(
    state: Option<&SetState>,
    consent: IconDownloads,
    asked: bool,
    can_download: bool,
) -> IconStep {
    match state {
        None => IconStep::Load,
        Some(SetState::Loading | SetState::Ready(_)) => IconStep::Nothing,
        Some(SetState::Failed(_)) => IconStep::Unavailable,
        Some(SetState::Missing) if !can_download => IconStep::Unavailable,
        Some(SetState::Missing) => match consent {
            IconDownloads::Allowed => IconStep::Fetch,
            IconDownloads::Declined => IconStep::Unavailable,
            IconDownloads::Ask if asked => IconStep::Nothing,
            IconDownloads::Ask => IconStep::Ask,
        },
    }
}

/// What a load or download worker sends back: the set's prefix and what the
/// store made of it.
type WorkerResult = (String, Result<Option<IconSet>, String>);

/// The app's title icons: the sets in use, their loaded forms, and the glyph
/// cache the pictures come from.
pub(super) struct SpaceIcons {
    sets: HashMap<String, SetState>,
    glyphs: GlyphCache,
    /// Automatic matches by title, in the default set; `None` is a remembered
    /// miss. Cleared when the default set changes.
    automatic: HashMap<String, Option<String>>,
    /// The workers' results. The sender is kept so a later download can use
    /// the same wake.
    sender: Sender<WorkerResult>,
    results: Receiver<WorkerResult>,
    /// The set whose consent question is waiting for an answer.
    pending: Option<&'static IconSetSpec>,
    /// Whether the consent question showed this session.
    asked: bool,
}

impl Default for SpaceIcons {
    fn default() -> Self {
        let (sender, results) = channel();
        Self {
            sets: HashMap::new(),
            glyphs: GlyphCache::new(),
            automatic: HashMap::new(),
            sender,
            results,
            pending: None,
            asked: false,
        }
    }
}

impl SpaceIcons {
    /// The sets the spaces need: each chosen set, and the default set when a
    /// space matches its title automatically. Sorted and deduplicated by
    /// prefix.
    pub(super) fn sets_in_use(spaces: &[Space]) -> Vec<&'static IconSetSpec> {
        let mut wanted = Vec::new();
        for space in spaces {
            match space.icon() {
                SpaceIcon::Chosen(icon) => {
                    if let Some(spec) = icon_set(&icon.set) {
                        wanted.push(spec);
                    }
                }
                SpaceIcon::Automatic => wanted.push(default_set()),
                SpaceIcon::None => {}
            }
        }
        wanted.sort_by_key(|spec| spec.prefix);
        wanted.dedup_by_key(|spec| spec.prefix);
        wanted
    }

    /// What is known about `prefix`, for `next_step`.
    pub(super) fn state(&self, prefix: &str) -> Option<&SetState> {
        self.sets.get(prefix)
    }

    pub(super) fn asked(&self) -> bool {
        self.asked
    }

    pub(super) fn set_pending(&mut self, spec: &'static IconSetSpec) {
        self.asked = true;
        self.pending = Some(spec);
    }

    /// Lets the question show again, after General is set back to "Ask first".
    pub(super) fn reset_asked(&mut self) {
        self.asked = false;
    }

    pub(super) fn take_pending(&mut self) -> Option<&'static IconSetSpec> {
        self.pending.take()
    }

    /// Reads the cache for `spec` on a worker, then wakes the app.
    pub(super) fn load(&mut self, store: &ConfigStore, ui: &Ui, spec: &'static IconSetSpec) {
        self.sets.insert(spec.prefix.to_owned(), SetState::Loading);
        let dir = store.icons_dir().to_path_buf();
        let sender = self.sender.clone();
        let waker = ui.waker();
        std::thread::spawn(move || {
            let result = IconStore::new(dir)
                .load(spec)
                .map_err(|error| error.to_string());
            let _ = sender.send((spec.prefix.to_owned(), result));
            waker.wake(ICONS_WAKE);
        });
    }

    /// Downloads `spec`'s signed set and saves it on a worker, then wakes the
    /// app. A failure is logged and becomes `Failed`, so it waits for the next
    /// start.
    pub(super) fn fetch(&mut self, store: &ConfigStore, ui: &Ui, spec: &'static IconSetSpec) {
        self.sets.insert(spec.prefix.to_owned(), SetState::Loading);
        let dir = store.icons_dir().to_path_buf();
        let sender = self.sender.clone();
        let waker = ui.waker();
        std::thread::spawn(move || {
            let store = IconStore::new(dir);
            let result = deskspace_update::fetch_signed(&spec.release_tag(), &spec.file_name())
                .map_err(|error| error.to_string())
                .and_then(|bytes| store.save(spec, &bytes).map_err(|error| error.to_string()))
                .map(Some);
            let _ = sender.send((spec.prefix.to_owned(), result));
            waker.wake(ICONS_WAKE);
        });
    }

    /// Takes in every finished load or download. Returns whether a set
    /// changed, and the sets that turned out missing, so the caller can ask or
    /// fetch them.
    pub(super) fn take_results(&mut self) -> (bool, Vec<&'static IconSetSpec>) {
        let mut changed = false;
        let mut missing = Vec::new();
        while let Ok((prefix, result)) = self.results.try_recv() {
            if prefix == default_set().prefix {
                self.automatic.clear();
            }
            let state = match result {
                Ok(Some(set)) => SetState::Ready(Arc::new(set)),
                Ok(None) => SetState::Missing,
                Err(error) => SetState::Failed(error),
            };
            if let SetState::Failed(error) = &state {
                tracing::warn!(%error, set = prefix, "could not load an icon set");
            }
            if matches!(state, SetState::Missing)
                && let Some(spec) = icon_set(&prefix)
            {
                missing.push(spec);
            }
            self.sets.insert(prefix, state);
            changed = true;
        }
        (changed, missing)
    }

    /// The glyph `space` shows, or `None` for no icon, an unknown one, or a
    /// set that is not loaded. The icon picker and the tests use it; the
    /// scenes go through `space_icon`, which caches the pixels.
    #[cfg(test)]
    pub(super) fn space_glyph(&mut self, space: &Space) -> Option<Glyph> {
        let (prefix, name) = self.icon_name(space)?;
        let SetState::Ready(set) = self.sets.get(&prefix)? else {
            return None;
        };
        set.glyph(&name)
    }

    /// `space`'s icon at `title_height`, in `color`, drawn through the glyph
    /// cache. The glyph is built only on a cache miss, so a repaint costs a
    /// lookup.
    pub(super) fn space_icon(
        &mut self,
        space: &Space,
        color: Color,
        title_height: i32,
    ) -> Option<Icon> {
        let (prefix, name) = self.icon_name(space)?;
        let SetState::Ready(set) = self.sets.get(&prefix)? else {
            return None;
        };
        let set = Arc::clone(set);
        let key = format!("{prefix}:{name}");
        let px = crate::scene::title_icon_rect(title_height).width;
        self.glyphs.icon(&key, px, color, || set.glyph(&name))
    }

    /// The set and icon name `space` resolves to: its own reference, or the
    /// best match for its title in the default set.
    fn icon_name(&mut self, space: &Space) -> Option<(String, String)> {
        match space.icon() {
            SpaceIcon::None => None,
            SpaceIcon::Chosen(icon) => Some((icon.set.clone(), icon.name.clone())),
            SpaceIcon::Automatic => {
                let spec = default_set();
                let title = space.title();
                if let Some(cached) = self.automatic.get(title).cloned() {
                    return cached.map(|name| (spec.prefix.to_owned(), name));
                }
                let SetState::Ready(set) = self.sets.get(spec.prefix)? else {
                    return None;
                };
                let name = best_icon(title, set, spec.words).map(str::to_owned);
                self.automatic.insert(title.to_owned(), name.clone());
                name.map(|name| (spec.prefix.to_owned(), name))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use deskspace_core::IconRef;
    use deskspace_core::icon_set::{IconData, IconSet};
    use std::collections::BTreeMap;

    /// A default set with one icon, "games", under the default prefix.
    fn games() -> IconSet {
        let mut icons = BTreeMap::new();
        icons.insert(
            "games-20-regular".to_owned(),
            IconData {
                body: "<path d=\"M0 0h8v8H0z\"/>".to_owned(),
                width: None,
                height: None,
            },
        );
        IconSet {
            prefix: default_set().prefix.to_owned(),
            icons,
            ..IconSet::default()
        }
    }

    fn with_games() -> SpaceIcons {
        let mut icons = SpaceIcons::default();
        icons.sets.insert(
            default_set().prefix.to_owned(),
            SetState::Ready(Arc::new(games())),
        );
        icons
    }

    fn space_titled(icon: SpaceIcon, title: &str) -> Space {
        let mut desktop = deskspace_core::Desktop::new();
        let id = crate::fixtures::create_space(
            &mut desktop,
            title,
            deskspace_core::geometry::Rect::new(0, 0, 240, 160),
        );
        crate::fixtures::apply(
            &mut desktop,
            deskspace_core::Command::SetSpaceIcon { id, icon },
        );
        desktop.space(id).cloned().expect("the space")
    }

    fn chosen(name: &str) -> SpaceIcon {
        SpaceIcon::Chosen(IconRef::parse(name).expect("a reference"))
    }

    #[test]
    fn a_chosen_icon_resolves_through_its_set() {
        let mut icons = with_games();
        let space = space_titled(chosen("fluent:games-20-regular"), "Anything");
        let glyph = icons.space_glyph(&space).expect("a glyph");
        assert_eq!(glyph.key, "fluent:games-20-regular");
        assert_eq!(glyph.width, 16);
    }

    #[test]
    fn an_automatic_icon_matches_the_title() {
        let mut icons = with_games();
        let games = space_titled(SpaceIcon::Automatic, "Games");
        assert_eq!(
            icons.space_glyph(&games).map(|glyph| glyph.key),
            Some("fluent:games-20-regular".to_owned())
        );
        let stuff = space_titled(SpaceIcon::Automatic, "Stuff");
        assert!(icons.space_glyph(&stuff).is_none());
    }

    #[test]
    fn no_icon_or_a_missing_set_gives_none() {
        let mut icons = with_games();
        let none = space_titled(SpaceIcon::None, "Games");
        assert!(icons.space_glyph(&none).is_none());

        let mut empty = SpaceIcons::default();
        let chosen = space_titled(chosen("fluent:games-20-regular"), "Games");
        assert!(empty.space_glyph(&chosen).is_none());
        empty
            .sets
            .insert(default_set().prefix.to_owned(), SetState::Missing);
        assert!(empty.space_glyph(&chosen).is_none());
    }

    #[test]
    fn sets_in_use_names_each_prefix_once() {
        let mut desktop = deskspace_core::Desktop::new();
        let automatic = crate::fixtures::create_space(
            &mut desktop,
            "Games",
            deskspace_core::geometry::Rect::new(0, 0, 200, 120),
        );
        crate::fixtures::apply(
            &mut desktop,
            deskspace_core::Command::SetSpaceIcon {
                id: automatic,
                icon: SpaceIcon::Automatic,
            },
        );
        let same = crate::fixtures::create_space(
            &mut desktop,
            "Photos",
            deskspace_core::geometry::Rect::new(0, 0, 200, 120),
        );
        crate::fixtures::apply(
            &mut desktop,
            deskspace_core::Command::SetSpaceIcon {
                id: same,
                icon: SpaceIcon::Automatic,
            },
        );
        let chosen_space = crate::fixtures::create_space(
            &mut desktop,
            "Mine",
            deskspace_core::geometry::Rect::new(0, 0, 200, 120),
        );
        crate::fixtures::apply(
            &mut desktop,
            deskspace_core::Command::SetSpaceIcon {
                id: chosen_space,
                icon: chosen("fluent:games-20-regular"),
            },
        );
        let specs = SpaceIcons::sets_in_use(desktop.spaces());
        assert_eq!(specs.len(), 1);
        assert_eq!(specs[0].prefix, default_set().prefix);
    }

    #[test]
    fn only_allowed_fetches() {
        use IconDownloads::{Allowed, Ask, Declined};
        use IconStep::{Ask as AskStep, Fetch, Load, Nothing, Unavailable};
        let missing = SetState::Missing;
        let loading = SetState::Loading;
        let ready = SetState::Ready(Arc::new(games()));
        let failed = SetState::Failed("no".to_owned());

        assert_eq!(next_step(None, Ask, false, true), Load);
        assert_eq!(next_step(None, Allowed, false, true), Load);
        assert_eq!(next_step(Some(&loading), Allowed, false, true), Nothing);
        assert_eq!(next_step(Some(&ready), Ask, false, true), Nothing);
        assert_eq!(next_step(Some(&failed), Allowed, false, true), Unavailable);

        assert_eq!(next_step(Some(&missing), Allowed, false, true), Fetch);
        assert_eq!(
            next_step(Some(&missing), Declined, false, true),
            Unavailable
        );
        assert_eq!(next_step(Some(&missing), Ask, false, true), AskStep);
        assert_eq!(next_step(Some(&missing), Ask, true, true), Nothing);
    }

    #[test]
    fn a_build_with_no_key_neither_asks_nor_downloads() {
        use IconDownloads::{Allowed, Ask, Declined};
        let missing = SetState::Missing;
        for consent in [Ask, Allowed, Declined] {
            assert_eq!(
                next_step(Some(&missing), consent, false, false),
                IconStep::Unavailable
            );
        }
        // A cached set was verified when it was saved, so it still loads.
        assert_eq!(next_step(None, Ask, false, false), IconStep::Load);
    }
}
