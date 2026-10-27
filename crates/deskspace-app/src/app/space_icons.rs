//! A space's title icon (ROADMAP D16): which icon set it needs, loading that
//! set off the main thread, and turning the icon's glyph into pixels.
//!
//! A set is loaded once, from the local icon cache (`IconStore`), on the first
//! start that needs it. Automatic icons match against the default set's words
//! and names; the match is memoized by title until that set changes. Nothing
//! here touches the network: a missing set simply shows no icon (S8 adds the
//! download).

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, TryRecvError, channel};

use deskspace_config::{ConfigStore, IconStore, default_set, icon_set};
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
enum SetState {
    /// A worker is reading it.
    Loading,
    Ready(Arc<IconSet>),
    /// The cache has no file for it; S8 may download it.
    Missing,
    Failed(String),
}

/// What a load worker sends back: the set's prefix and what the store made of
/// it.
type LoadResult = (String, Result<Option<IconSet>, String>);

/// The app's title icons: the sets in use, their loaded forms, and the glyph
/// cache the pictures come from.
#[derive(Default)]
pub(super) struct SpaceIcons {
    sets: HashMap<String, SetState>,
    glyphs: GlyphCache,
    /// Automatic matches by title, in the default set; `None` is a remembered
    /// miss. Cleared when the default set changes.
    automatic: HashMap<String, Option<String>>,
    /// The workers' results, while any is running.
    loading: Option<Receiver<LoadResult>>,
}

impl SpaceIcons {
    /// Starts loading every set the spaces need, one worker per set. A set
    /// that is already known (loading, ready, missing, failed) is skipped, so
    /// the second start is a no-op.
    pub(super) fn start(&mut self, store: &ConfigStore, ui: &Ui, spaces: &[Space]) {
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

        let (sender, receiver) = channel();
        let dir = store.icons_dir().to_path_buf();
        let waker = ui.waker();
        let mut started = false;
        for spec in wanted {
            if self.sets.contains_key(spec.prefix) {
                continue;
            }
            self.sets.insert(spec.prefix.to_owned(), SetState::Loading);
            started = true;
            let sender = sender.clone();
            let dir = dir.clone();
            let waker = waker.clone();
            std::thread::spawn(move || {
                let result = IconStore::new(dir)
                    .load(spec)
                    .map_err(|error| error.to_string());
                let _ = sender.send((spec.prefix.to_owned(), result));
                waker.wake(ICONS_WAKE);
            });
        }
        if started {
            self.loading = Some(receiver);
        }
    }

    /// Takes in every finished load. Returns whether a set changed, so the
    /// caller can repaint the spaces.
    pub(super) fn on_loaded(&mut self) -> bool {
        let Some(receiver) = self.loading.take() else {
            return false;
        };
        let mut changed = false;
        let mut connected = true;
        loop {
            match receiver.try_recv() {
                Ok((prefix, result)) => {
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
                    self.sets.insert(prefix, state);
                    changed = true;
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    connected = false;
                    break;
                }
            }
        }
        if connected {
            self.loading = Some(receiver);
        }
        changed
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
}
