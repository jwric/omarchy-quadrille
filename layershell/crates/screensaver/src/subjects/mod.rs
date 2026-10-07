//! What the sheets show.
//!
//! A subject is one drawing: a thing, drawn in its own units at a moment of
//! its motion, and the card that documents it (title, notes, parts). It knows
//! nothing about pixels, sheets or plotting; the sheet does all of that the
//! same way for every subject.
//!
//! To add one, write a module with a type that implements [`Subject`] and add
//! it to [`all`]. `quadrille-screensaver render --subject NAME` draws it
//! headless at any moment, to look at while it is designed.
use crate::draft::{Draft, Extent, V2};

mod aerofoil;
mod engine;
mod gears;
mod geneva;
mod optics;
mod orbit;
mod schematic;
mod timer;

/// Every subject, in sheet order.
pub fn all() -> Vec<Box<dyn Subject>> {
    vec![
        Box::new(gears::Gears::new()),
        Box::new(engine::Engine::new()),
        Box::new(geneva::Geneva::new()),
        Box::new(timer::Timer::new()),
        Box::new(optics::Optics::new()),
        Box::new(aerofoil::Aerofoil::new()),
        Box::new(orbit::Orbit::new()),
    ]
}

/// A drawing of one thing.
pub trait Subject {
    /// A short name for the command line: `gears`, `engine`.
    fn name(&self) -> &'static str;

    /// What the title block, notes and parts list say.
    fn card(&self) -> &Card;

    /// The part of the model the view frames, in the card's unit.
    fn extent(&self) -> Extent;

    /// Draws the subject as it is `t` seconds into its motion.
    ///
    /// Marks that move are made inside [`Draft::moving`] and the drawing of a
    /// part inside [`Draft::part`], so the sheet can redraw only what moves and
    /// pick a part out when it documents it.
    fn draw(&self, draft: &mut Draft, t: f32);

    /// What the detail view of part `index` magnifies at `t`: the card's
    /// fixed circle, unless the subject follows a moving part with it.
    fn detail(&self, index: usize, _t: f32) -> Option<Detail> {
        self.card().parts.get(index)?.detail
    }

    /// What the subject's instruments read at `t`: shown beside the view.
    fn readings(&self, _t: f32) -> Vec<Reading> {
        Vec::new()
    }
}

/// The documentation of a subject.
#[derive(Debug, Clone)]
pub struct Card {
    pub title: String,
    /// The drawing number: `QD-` and the domain's letter.
    pub number: String,
    pub domain: Domain,
    pub unit: Unit,
    /// Whether the view is drawn to a scale or is a diagram.
    pub scaled: bool,
    /// The view's name under it: `FRONT VIEW`, `SCHEMATIC`.
    pub view: String,
    /// General notes, numbered on the sheet.
    pub notes: Vec<String>,
    pub parts: Vec<Part>,
}

/// One item of the parts list.
#[derive(Debug, Clone)]
pub struct Part {
    pub name: String,
    pub quantity: u32,
    /// What it is made of, or its type for a component.
    pub material: String,
    /// Its specification, as name and value.
    pub spec: Vec<(String, String)>,
    /// What the detail view magnifies to show it: a circle of the model.
    pub detail: Option<Detail>,
}

impl Part {
    pub fn new(name: &str, quantity: u32, material: &str) -> Self {
        Self {
            name: name.into(),
            quantity,
            material: material.into(),
            spec: Vec::new(),
            detail: None,
        }
    }

    pub fn spec(mut self, name: &str, value: impl Into<String>) -> Self {
        self.spec.push((name.into(), value.into()));
        self
    }

    pub fn detail(mut self, centre: V2, radius: f32) -> Self {
        self.detail = Some(Detail::fixed(centre, radius));
        self
    }
}

/// A circle of the model that a detail view magnifies.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Detail {
    pub centre: V2,
    pub radius: f32,
    /// Whether it moves with its part, and so is drawn every frame.
    pub follows: bool,
}

impl Detail {
    pub const fn fixed(centre: V2, radius: f32) -> Self {
        Self {
            centre,
            radius,
            follows: false,
        }
    }

    pub const fn following(centre: V2, radius: f32) -> Self {
        Self {
            centre,
            radius,
            follows: true,
        }
    }
}

/// The engineering discipline a drawing belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Domain {
    Mechanical,
    Electrical,
    Optical,
    Aeronautical,
    Astronautical,
}

impl Domain {
    pub fn label(self) -> &'static str {
        match self {
            Self::Mechanical => "MECHANICAL",
            Self::Electrical => "ELECTRICAL",
            Self::Optical => "OPTICAL",
            Self::Aeronautical => "AERONAUTICAL",
            Self::Astronautical => "ASTRONAUTICAL",
        }
    }

    /// What the parts list calls the column after the quantity.
    pub fn material_heading(self) -> &'static str {
        match self {
            Self::Electrical => "VALUE",
            _ => "MATERIAL",
        }
    }
}

/// The unit a model is drawn in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unit {
    Millimetre,
    Kilometre,
}

impl Unit {
    /// Millimetres in one unit.
    pub fn millimetres(self) -> f64 {
        match self {
            Self::Millimetre => 1.0,
            Self::Kilometre => 1e6,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Millimetre => "mm",
            Self::Kilometre => "km",
        }
    }
}

/// One instrument reading: what it is and what it says.
#[derive(Debug, Clone, PartialEq)]
pub struct Reading {
    pub name: &'static str,
    pub value: String,
}

impl Reading {
    pub fn new(name: &'static str, value: impl Into<String>) -> Self {
        Self {
            name,
            value: value.into(),
        }
    }
}

/// The subject called `name`.
pub fn find(name: &str) -> Option<(usize, Box<dyn Subject>)> {
    all()
        .into_iter()
        .enumerate()
        .find(|(_, subject)| subject.name() == name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::draft::raster::{Projection, rasterize, rect};
    use crate::draft::{Ink, Mark};

    #[test]
    fn every_subject_is_named_once_and_documents_its_parts() {
        let subjects = all();
        let mut names: Vec<_> = subjects.iter().map(|s| s.name()).collect();
        names.sort();
        names.dedup();

        assert_eq!(names.len(), subjects.len());

        for subject in &subjects {
            let card = subject.card();

            assert!(!card.parts.is_empty(), "{} has no parts", subject.name());
            assert!(
                card.title.chars().count() <= 24,
                "{}'s title is too long",
                subject.name()
            );

            // The parts list on the laptop letters 14 of a name and 10 of a
            // material; a name it would cut is a name to shorten.
            for part in &card.parts {
                assert!(
                    part.name.chars().count() <= 14,
                    "{}: {} is too long",
                    subject.name(),
                    part.name
                );
                assert!(
                    part.material.chars().count() <= 10,
                    "{}: {} is too long",
                    subject.name(),
                    part.material
                );
            }

            let mut draft = Draft::new();
            subject.draw(&mut draft, 0.0);

            // Every part is drawn, and every balloon names a part there is.
            for index in 0..card.parts.len() {
                assert!(
                    draft.marks().iter().any(|mark| mark.part == Some(index)),
                    "{} never draws part {}",
                    subject.name(),
                    index + 1
                );
            }
            for mark in draft.marks() {
                if let Ink::Balloon { item, .. } = mark.ink {
                    assert!(item >= 1 && item <= card.parts.len());
                }
            }
        }
    }

    #[test]
    fn every_subject_draws_at_any_moment() {
        for subject in all() {
            // The extent fitted to a laptop-sized view.
            let extent = subject.extent();
            let scale = (560.0 / extent.width()).min(400.0 / extent.height());
            let projection = Projection::centred(extent, rect(0, 0, 560, 400), scale);

            for t in [0.0, 0.37, 5.0, 61.3, 1234.5] {
                let mut draft = Draft::new();
                subject.draw(&mut draft, t);

                let marks: &[Mark] = draft.marks();
                let mut out = Vec::new();

                for mark in marks {
                    rasterize(mark, &projection, &mut out);
                }

                assert!(!out.is_empty());

                for reading in subject.readings(t) {
                    assert!(
                        !reading.value.contains("NaN"),
                        "{}: {reading:?}",
                        subject.name()
                    );
                }
            }
        }
    }
}
