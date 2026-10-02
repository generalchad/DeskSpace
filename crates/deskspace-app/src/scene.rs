use deskspace_core::{Desktop, spaceId, Rect, Theme};
use deskspace_platform::{spaceVisual, IconCache, ItemVisual, Scene};

pub struct SceneRequest<'a> {
    pub desktop: &'a Desktop,
    pub theme: &'a Theme,
    pub width: i32,
    pub height: i32,
    pub selected: Option<spaceId>,
    pub status: Option<String>,
}

pub fn build_scene(request: &SceneRequest<'_>, icons: &mut IconCache) -> Scene {
    let mut scene = Scene::new(request.width, request.height, request.theme.color_key);
    scene.status = request.status.clone();

    let cell = request.desktop.layout.cell();

    for space in &request.desktop.spaces {
        let mut visual = spaceVisual {
            id: space.id.0,
            rect: space.rect,
            title: space.title.clone(),
            title_rect: space.title_bar(),
            content_rect: space.content_rect(),
            fill: request.theme.background_fill.with_alpha(space.opacity),
            border: request.theme.space_border,
            shadow: request.theme.space_shadow,
            title_fill: request.theme.title_fill,
            title_text: request.theme.title_text,
            item_text: request.theme.item_text,
            selected: request.selected == Some(space.id),
            rolled_up: space.rolled_up,
            locked: space.locked,
            items: Vec::with_capacity(space.items.len()),
        };

        if space.rolled_up {
            scene.spaces.push(visual);
            continue;
        }

        for item in &space.items {
            visual.items.push(ItemVisual {
                id: item.id.0,
                rect: Rect::from_point_size(item.position, cell),
                label: item.label.clone(),
                fill: request.theme.item_fill,
                border: request.theme.item_border,
                icon: icons.icon_for(&item.path),
            });
        }

        scene.spaces.push(visual);
    }

    scene
}
