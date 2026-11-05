//! The categories pane's groups: which pages each scope lists, under which
//! titles, in order. The one source for both the pane's rows and
//! `listed_pages`, so the two cannot disagree.

use super::ids::page;
use super::{LOOK_PAGE_COUNT, ScopeKind, tables};

/// One titled group of the categories pane.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct NavGroup {
    pub(super) title: &'static str,
    /// Page indices (`page::*`), in the order the pane lists them.
    pub(super) pages: Vec<usize>,
}

impl NavGroup {
    fn new(title: &'static str, pages: impl IntoIterator<Item = usize>) -> Self {
        Self {
            title,
            pages: pages.into_iter().collect(),
        }
    }
}

/// The groups `scope` lists. A space lists its own pages first, then its
/// look; a lens's own pages start with its Lens page, its folder and title,
/// which make it that lens.
pub(super) fn nav_groups(scope: ScopeKind) -> Vec<NavGroup> {
    let look = NavGroup::new("Look", 0..LOOK_PAGE_COUNT);
    match scope {
        ScopeKind::Global => vec![
            look,
            NavGroup::new("Fonts", [page::SPACE_FONTS, page::WINDOW_FONTS]),
            NavGroup::new("New items", [page::NEW_SPACES, page::LENSES]),
            NavGroup::new("DeskSpace", [page::GENERAL, page::HOTKEYS, page::ABOUT]),
            NavGroup::new(
                "Animations",
                page::ANIMATIONS..page::ANIMATIONS + tables::ANIMATION_PAGES.len(),
            ),
        ],
        ScopeKind::Space => vec![
            NavGroup::new("This space", [page::VIEW, page::SHORTCUTS]),
            look,
        ],
        ScopeKind::Lens => vec![
            NavGroup::new("This space", [page::LENS, page::VIEW, page::SHORTCUTS]),
            look,
        ],
    }
}

/// The pages the categories pane lists for `scope`, in the pane's order.
pub fn listed_pages(scope: ScopeKind) -> Vec<usize> {
    nav_groups(scope)
        .into_iter()
        .flat_map(|group| group.pages)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn global_lists_every_global_page_once() {
        let mut listed = listed_pages(ScopeKind::Global);
        let count = listed.len();
        listed.sort_unstable();
        listed.dedup();
        assert_eq!(listed.len(), count, "a page listed twice");
        for page in [page::VIEW, page::SHORTCUTS, page::LENS] {
            assert!(!listed.contains(&page), "page {page} is a space's");
        }
        // Every built page but the three that belong to a space.
        assert_eq!(count, page::WINDOW_FONTS + 1 - 3);
    }

    #[test]
    fn every_group_has_a_title_and_a_page() {
        for scope in [ScopeKind::Global, ScopeKind::Space, ScopeKind::Lens] {
            for group in nav_groups(scope) {
                assert!(!group.title.is_empty());
                assert!(!group.pages.is_empty(), "{}", group.title);
            }
        }
    }
}
