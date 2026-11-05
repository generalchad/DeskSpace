//! How a settings page's fields group into cards (ROADMAP D26): a heading
//! sits above its card, and a note right after a field describes that field.
//! Pure, so the page shape can be tested without a window.

use crate::ui::{Field, FieldKind};

/// Where one field sits in its page's shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Placement {
    pub(crate) role: Role,
    /// This field is the first row of a new card, so the card's top padding
    /// starts above it.
    pub(crate) opens_card: bool,
    /// This field is the card's last, so the card's bottom padding and the
    /// gap below it follow.
    pub(crate) closes_card: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Role {
    /// Drawn above the next card, not on one.
    Heading,
    /// The first control row of the card, after its padding.
    Row,
    /// A note explaining the row above it.
    Description,
}

/// Card geometry at 96 DPI.
/// Padding inside a card's left and right edges.
pub(crate) const CARD_PADDING_X: i32 = 16;
/// Padding inside a card's top and bottom edges.
pub(crate) const CARD_PADDING_Y: i32 = 8;
pub(crate) const CARD_RADIUS: i32 = 4;
/// The gap below a card, before the next heading or card.
pub(crate) const CARD_GAP: i32 = 12;
/// The space above a hairline that divides two rows of a card.
pub(crate) const ROW_SEPARATOR_GAP: i32 = 8;
/// The gap between a row and its description.
pub(crate) const DESCRIPTION_GAP: i32 = 2;

/// One placement per field of a page, in order.
///
/// A heading closes the card above it and is drawn above the next one. A note
/// right after a field that is neither a heading nor a note describes that
/// field; every other field is a row, and opens a card if none is open. Fields
/// before a page's first heading form an untitled card. A card closes on its
/// last field.
pub(crate) fn card_plan(fields: &[Field]) -> Vec<Placement> {
    let mut plan = Vec::with_capacity(fields.len());
    let mut card_open = false;
    let mut last_of_card: Option<usize> = None;
    for field in fields {
        let note = matches!(field.kind, FieldKind::Note);
        if matches!(field.kind, FieldKind::Heading) {
            close(&mut plan, &mut last_of_card);
            card_open = false;
            plan.push(Placement {
                role: Role::Heading,
                opens_card: false,
                closes_card: false,
            });
        } else if note && matches!(plan.last().map(|each| each.role), Some(Role::Row)) {
            debug_assert!(card_open);
            plan.push(Placement {
                role: Role::Description,
                opens_card: false,
                closes_card: false,
            });
            last_of_card = Some(plan.len() - 1);
        } else {
            plan.push(Placement {
                role: Role::Row,
                opens_card: !card_open,
                closes_card: false,
            });
            card_open = true;
            last_of_card = Some(plan.len() - 1);
        }
    }
    close(&mut plan, &mut last_of_card);
    plan
}

/// Marks the open card's last field as closing it.
fn close(plan: &mut [Placement], last_of_card: &mut Option<usize>) {
    if let Some(index) = last_of_card.take() {
        plan[index].closes_card = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::FieldKind;

    fn field(kind: FieldKind) -> Field {
        Field {
            id: 0,
            label: String::new(),
            kind,
            enabled: true,
        }
    }

    fn heading() -> Field {
        field(FieldKind::Heading)
    }

    fn note() -> Field {
        field(FieldKind::Note)
    }

    fn toggle() -> Field {
        field(FieldKind::Toggle { on: false })
    }

    fn roles(fields: &[Field]) -> Vec<Role> {
        card_plan(fields).iter().map(|each| each.role).collect()
    }

    #[test]
    fn headings_and_rows_and_notes_plan_a_page() {
        let page = [heading(), toggle(), note(), toggle(), heading(), toggle()];
        assert_eq!(
            roles(&page),
            [
                Role::Heading,
                Role::Row,
                Role::Description,
                Role::Row,
                Role::Heading,
                Role::Row,
            ]
        );
        let plan = card_plan(&page);
        assert!(plan[1].opens_card, "the heading's first row opens its card");
        assert!(plan[3].closes_card, "the last row before a heading closes");
        assert!(plan[5].opens_card && plan[5].closes_card);
        assert!(!plan[0].opens_card && !plan[4].opens_card);
    }

    #[test]
    fn a_note_after_a_heading_is_a_row() {
        assert_eq!(roles(&[heading(), note()]), [Role::Heading, Role::Row]);
    }

    #[test]
    fn two_notes_in_a_row_are_both_rows() {
        let page = [heading(), toggle(), note(), note()];
        assert_eq!(
            roles(&page),
            [Role::Heading, Role::Row, Role::Description, Role::Row]
        );
        let plan = card_plan(&page);
        assert!(plan[3].closes_card, "the last of the two closes the card");
    }

    #[test]
    fn a_page_without_a_heading_is_one_untitled_card() {
        let page = [toggle(), toggle()];
        let plan = card_plan(&page);
        assert_eq!(roles(&page), [Role::Row, Role::Row]);
        assert!(plan[0].opens_card);
        assert!(!plan[0].closes_card);
        assert!(plan[1].closes_card);
    }

    #[test]
    fn a_heading_closes_the_card_before_it() {
        let page = [toggle(), heading(), toggle()];
        let plan = card_plan(&page);
        assert!(plan[0].opens_card && plan[0].closes_card);
        assert!(plan[1].role == Role::Heading && !plan[1].closes_card);
        assert!(plan[2].opens_card && plan[2].closes_card);
    }

    #[test]
    fn a_trailing_description_closes_the_card() {
        let page = [heading(), toggle(), note()];
        let plan = card_plan(&page);
        assert!(plan[2].closes_card);
    }

    #[test]
    fn an_empty_page_plans_nothing() {
        assert!(card_plan(&[]).is_empty());
    }
}
