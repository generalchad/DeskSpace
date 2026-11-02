//! A cache of the installed apps for the "From installed apps…" picker
//! (ROADMAP F-53): every app in the shell's Applications folder with its icon,
//! sorted by name.
//!
//! Listing the folder and rendering a few hundred icons takes a noticeable
//! moment, so the app fills the cache on a background thread at startup
//! (`Ui::prefetch_apps`), and the picker refreshes it again in the background
//! each time it closes, so an app installed meanwhile shows up the next time.
//! The cached data is plain owned data (`ShellItem`'s ID list bytes and the
//! `Icon`'s pixels), so it crosses threads freely; COM objects never do.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx, CoUninitialize};
use windows::Win32::UI::Shell::{
    BHID_EnumItems, FOLDERID_AppsFolder, IEnumShellItems, IShellItem, KF_FLAG_DEFAULT,
    SHGetKnownFolderItem,
};

use crate::input::ShellItem;
use crate::render::Icon;
use crate::win32::icons::shell_item_icon;
use crate::win32::shortcuts::shell_item;

/// The size icons are cached at; the picker resamples them to its row size.
/// 48 px covers its 24 px rows up to 200 % scaling.
pub(crate) const CATALOG_ICON_SIZE: u32 = 48;

/// One installed app.
#[derive(Debug, Clone)]
pub(crate) struct CatalogApp {
    pub item: ShellItem,
    /// Its icon at `CATALOG_ICON_SIZE`, or `None` when it was listed without
    /// icons (the synchronous fallback) or the shell could not render one.
    pub icon: Option<Icon>,
}

// Process state: the installed apps, once a background listing finishes. The
// listing runs off the UI thread and the picker reads it from here, so it
// cannot be passed; the worker replaces it and nothing clears it.
static CATALOG: Mutex<Option<Arc<Vec<CatalogApp>>>> = Mutex::new(None);
// Process state: whether a background listing is already running, so a second
// request does nothing. `refresh_in_background` sets it and its worker clears
// it, including when the thread cannot be spawned.
static REFRESHING: AtomicBool = AtomicBool::new(false);

/// The cached apps, if a listing has finished.
pub(crate) fn cached() -> Option<Arc<Vec<CatalogApp>>> {
    CATALOG.lock().ok()?.clone()
}

/// Lists the apps now, on the calling (UI) thread, without icons, for a
/// picker opened before the background listing finished. Needs COM, which
/// the runtime has initialized.
pub(crate) fn list_now() -> Arc<Vec<CatalogApp>> {
    Arc::new(list(false))
}

/// Starts a background listing that replaces the cache when it is done. Does
/// nothing while one is already running.
pub(crate) fn refresh_in_background() {
    if REFRESHING.swap(true, Ordering::SeqCst) {
        return;
    }
    let spawned = std::thread::Builder::new()
        .name("deskspace-app-catalog".into())
        .spawn(|| {
            // SAFETY: this thread initializes its own single-threaded COM
            // apartment, which the shell's folder and image APIs expect, and
            // uninitializes it once every COM object it made is dropped.
            let initialized = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }.is_ok();
            let apps = list(true);
            if initialized {
                // SAFETY: balances the successful CoInitializeEx above.
                unsafe { CoUninitialize() };
            }
            tracing::debug!(count = apps.len(), "cached the installed apps");
            if let Ok(mut catalog) = CATALOG.lock() {
                *catalog = Some(Arc::new(apps));
            }
            REFRESHING.store(false, Ordering::SeqCst);
        });
    if let Err(error) = spawned {
        tracing::warn!(%error, "could not start listing the installed apps");
        REFRESHING.store(false, Ordering::SeqCst);
    }
}

/// Every app in the Applications folder, sorted by name, with icons when
/// `with_icons` is set.
fn list(with_icons: bool) -> Vec<CatalogApp> {
    let mut apps: Vec<CatalogApp> = enumerate()
        .into_iter()
        .map(|(item, shell)| CatalogApp {
            icon: with_icons
                .then(|| shell_item_icon(&shell, CATALOG_ICON_SIZE))
                .flatten(),
            item,
        })
        .collect();
    apps.sort_by_cached_key(|app| app.item.name.to_lowercase());
    apps
}

/// Every app in the Applications folder with its live shell item. COM must be
/// initialized on the calling thread.
/// https://learn.microsoft.com/en-us/windows/win32/shell/knownfolderid (FOLDERID_AppsFolder)
fn enumerate() -> Vec<(ShellItem, IShellItem)> {
    let mut apps = Vec::new();
    // SAFETY: COM is initialized on this thread; every interface is released
    // when it goes out of scope.
    unsafe {
        let Ok(folder) =
            SHGetKnownFolderItem::<IShellItem>(&FOLDERID_AppsFolder, KF_FLAG_DEFAULT, None)
        else {
            tracing::warn!("cannot open the Applications folder");
            return apps;
        };
        let Ok(items) = folder.BindToHandler::<_, IEnumShellItems>(None, &BHID_EnumItems) else {
            tracing::warn!("cannot enumerate the Applications folder");
            return apps;
        };
        loop {
            let mut batch = [None];
            let mut fetched = 0u32;
            if items.Next(&mut batch, Some(&mut fetched)).is_err() || fetched == 0 {
                break;
            }
            if let Some(shell) = batch[0].take()
                && let Some(item) = shell_item(&shell)
                && !item.name.trim().is_empty()
            {
                apps.push((item, shell));
            }
        }
    }
    apps
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_applications_folder_lists_apps_with_id_lists() {
        // SAFETY: initializes COM for this test thread only.
        let _ = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
        let apps = list(false);
        assert!(!apps.is_empty(), "every Windows install has some apps");
        assert!(apps.iter().all(|app| !app.item.id_list.is_empty()));
        // Store apps have no filesystem path; they must still be listed.
        assert!(
            apps.iter().any(|app| app.item.path.is_none()),
            "Store apps are listed"
        );
        let names: Vec<String> = apps
            .iter()
            .map(|app| app.item.name.to_lowercase())
            .collect();
        assert!(names.is_sorted(), "sorted by name");
    }

    #[test]
    fn a_background_refresh_fills_the_cache() {
        refresh_in_background();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
        while REFRESHING.load(Ordering::SeqCst) && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        let apps = cached().expect("the listing finished");
        assert!(
            apps.iter().any(|app| app.icon.is_some()),
            "icons are cached"
        );
    }
}
