//! `App` clipboard: cut, copy, and paste through the shell (R1).

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use deskspace_core::{Command, ItemId, SpaceId};
use deskspace_platform::{ClipboardFiles, Ui, item_facts};

use super::{App, Surface};

impl App {
    /// Puts the selected items on the shell clipboard, cut (to be moved on
    /// paste) or copied. Cut items are drawn ghosted until the paste.
    pub(super) fn copy_selection(&mut self, ui: &mut Ui, cut: bool) {
        let active = self.active_items();
        let paths: Vec<PathBuf> = active
            .iter()
            .filter_map(|item| self.config.desktop.item(*item))
            .map(|item| item.path.clone())
            .filter(|path| path.exists())
            .collect();
        if paths.is_empty() {
            return;
        }
        if !ui.set_clipboard_files(&paths, cut) {
            return;
        }
        // Replacing the cut set un-ghosts the previous items, which may live in
        // other spaces, so repaint every space that showed one.
        let mut touched: HashSet<SpaceId> = self
            .selection
            .cut
            .iter()
            .filter_map(|item| self.config.desktop.space_containing(*item))
            .collect();
        self.selection.cut = if cut {
            active.iter().copied().collect()
        } else {
            HashSet::new()
        };
        touched.extend(
            self.selection
                .cut
                .iter()
                .filter_map(|item| self.config.desktop.space_containing(*item)),
        );
        if let Some(Surface::Space(space)) = self.selection.surface {
            touched.insert(space);
        }
        for id in touched {
            self.refresh_space(ui, id);
        }
        if self.selection.surface == Some(Surface::Desktop) {
            self.refresh_desktop_layer(ui);
        }
    }

    /// Clears cut ghosting when the clipboard no longer holds the cut, for
    /// example after another app (or the desktop) copies, cuts, or pastes it.
    /// A paste elsewhere has moved the files, so any item whose file is gone is
    /// dropped. A failed clipboard read keeps the ghost rather than clearing it
    /// wrongly, since another process can hold the clipboard for a moment.
    pub(super) fn sync_cut_with_clipboard(&mut self, ui: &mut Ui) {
        if self.selection.cut.is_empty() {
            return;
        }
        let cut_paths: Vec<PathBuf> = self
            .selection
            .cut
            .iter()
            .filter_map(|item| self.config.desktop.item(*item))
            .map(|item| item.path.clone())
            .collect();
        let Some(clipboard) = ui.clipboard_files() else {
            return;
        };
        if cut_matches_clipboard(&clipboard, &cut_paths) {
            return;
        }
        let touched: HashSet<SpaceId> = self
            .selection
            .cut
            .iter()
            .filter_map(|item| self.config.desktop.space_containing(*item))
            .collect();
        self.selection.cut.clear();
        for id in touched {
            self.prune_missing(ui, id);
            self.refresh_space(ui, id);
        }
        // A cut loose item drawn ghosted on the layer needs its ghost cleared.
        if self.layer_active() {
            self.refresh_desktop_layer(ui);
        }
    }

    /// Adds the clipboard's files to the current space, as a drop would: a cut
    /// moves them (items already in a space just change space, sharing the
    /// file), a copy duplicates them into the store. Physical copies and moves
    /// go through the shell's file operation so Explorer's progress UI and undo
    /// apply.
    pub(super) fn paste_clipboard(&mut self, ui: &mut Ui) {
        // Pasting lands in a space, never on the loose desktop layer.
        let Some(Surface::Space(target)) = self.selection.surface else {
            return;
        };
        let Some(clipboard) = ui.clipboard_files() else {
            return;
        };
        if clipboard.paths.is_empty() {
            return;
        }
        let at = self.space_center(target);
        let plan = plan_paste(&clipboard.paths, clipboard.cut, |path| {
            self.find_item_by_file(path)
        });
        let PastePlan {
            moved,
            external_moves,
            copies,
        } = plan;
        let mut touched = HashSet::from([target]);

        for item in &moved {
            match self.config.desktop.space_containing(*item) {
                Some(source) if source == target => continue,
                Some(source) => {
                    touched.insert(source);
                }
                None => {}
            }
            self.dispatch(Command::MoveItem { id: *item, target }, at);
        }

        let store = self.storage.root().to_path_buf();
        let mut created: Vec<PathBuf> = Vec::new();
        if !external_moves.is_empty()
            && let Some(paths) = ui.transfer_files(&external_moves, &store, true)
        {
            created.extend(paths);
        }
        if !copies.is_empty()
            && let Some(paths) = ui.transfer_files(&copies, &store, false)
        {
            created.extend(paths);
        }
        for path in &created {
            // Guard against a shell result that no longer exists; listing it
            // would leave a label with no icon.
            if !path.exists() {
                tracing::warn!(path = %path.display(), "pasted file is missing; ignored");
                continue;
            }
            self.dispatch(
                Command::AddItem {
                    space: target,
                    path: path.clone(),
                    facts: item_facts(path),
                },
                at,
            );
        }

        // Select what was pasted, as Explorer does.
        let mut pasted: Vec<ItemId> = moved
            .iter()
            .copied()
            .filter(|item| self.config.desktop.space_containing(*item) == Some(target))
            .collect();
        pasted.extend(
            created
                .iter()
                .filter_map(|path| self.find_item_by_file(path)),
        );
        if !pasted.is_empty() {
            self.selection.select_items(Surface::Space(target), pasted);
        }

        if clipboard.cut {
            // Un-ghost every cut item, including any not part of this paste,
            // and repaint the spaces that showed them.
            for item in std::mem::take(&mut self.selection.cut) {
                if let Some(space) = self.config.desktop.space_containing(item) {
                    touched.insert(space);
                }
            }
            ui.clear_clipboard();
        }
        for id in &touched {
            self.refresh_space(ui, *id);
        }
        self.refresh_tray(ui);
        self.save();
    }
}

/// Whether `clipboard` is the cut DeskSpace put there: the same paths in any
/// order, still marked as a cut.
fn cut_matches_clipboard(clipboard: &ClipboardFiles, cut: &[PathBuf]) -> bool {
    clipboard.cut
        && clipboard.paths.len() == cut.len()
        && cut.iter().all(|path| {
            clipboard
                .paths
                .iter()
                .any(|clip| crate::storage::same_path(clip, path))
        })
}

/// What a paste does with each clipboard path: change an existing item's
/// space, move an external file into the store, or copy a file into the store.
#[derive(Debug, Default, PartialEq, Eq)]
struct PastePlan {
    moved: Vec<ItemId>,
    external_moves: Vec<PathBuf>,
    copies: Vec<PathBuf>,
}

fn plan_paste(paths: &[PathBuf], cut: bool, known: impl Fn(&Path) -> Option<ItemId>) -> PastePlan {
    let mut plan = PastePlan::default();
    for path in paths {
        if !path.exists() {
            continue;
        }
        match (known(path), cut) {
            (Some(item), true) => plan.moved.push(item),
            (Some(_), false) => plan.copies.push(path.clone()),
            (None, true) => plan.external_moves.push(path.clone()),
            (None, false) => plan.copies.push(path.clone()),
        }
    }
    plan
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cut_matches_only_the_same_cut_files() {
        let cut = vec![PathBuf::from("C:/A/B.txt")];
        let matching = ClipboardFiles {
            paths: vec![PathBuf::from("c:/a/b.TXT")],
            cut: true,
        };
        assert!(cut_matches_clipboard(&matching, &cut));
        assert!(!cut_matches_clipboard(
            &ClipboardFiles {
                cut: false,
                ..matching.clone()
            },
            &cut
        ));
        assert!(!cut_matches_clipboard(
            &ClipboardFiles {
                paths: vec![PathBuf::from("C:/A/other.txt")],
                cut: true,
            },
            &cut
        ));
    }

    #[test]
    fn plan_paste_classifies_known_and_external_paths() {
        let name = |suffix: &str| {
            let path = std::env::temp_dir()
                .join(format!("deskspace-paste-{suffix}-{}", std::process::id()));
            std::fs::write(&path, "x").unwrap();
            path
        };
        let external = name("external.txt");
        let known = name("known.txt");
        let known_id = ItemId(7);
        let paths = vec![external.clone(), known.clone()];
        let lookup = |path: &Path| (known.as_path() == path).then_some(known_id);

        let copy = plan_paste(&paths, false, lookup);
        assert_eq!(copy.copies, paths);
        assert!(copy.moved.is_empty() && copy.external_moves.is_empty());

        let cut = plan_paste(&paths, true, lookup);
        assert_eq!(cut.external_moves, vec![external.clone()]);
        assert_eq!(cut.moved, vec![known_id]);
        assert!(cut.copies.is_empty());

        let _ = std::fs::remove_file(&external);
        let _ = std::fs::remove_file(&known);
    }
}
