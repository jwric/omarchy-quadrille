//! When each thing happens on a sheet.
//!
//! A sheet's life is a function of the time since it began, so any moment
//! of it can be drawn again exactly (the headless renderer relies on that):
//!
//! 1. the title block types itself in and the drawing is plotted, a stroke
//!    at a time, construction first and lettering last;
//! 2. the subject starts to move, and after a beat each part in turn is
//!    picked out, magnified in a detail view and specified;
//! 3. a wipe clears the sheet for the next subject.

/// The title block's lettering types in over this long.
pub const TYPING: f32 = 1.2;
/// The plotter starts after the title block has begun...
pub const PLOT_START: f32 = 0.6;
/// ...and plots the whole drawing in this long.
pub const PLOT: f32 = 9.0;
/// The subject runs on its own a while before the first part is picked out.
pub const SETTLE: f32 = 2.5;
/// Each part is documented for this long.
pub const DETAIL: f32 = 7.0;
/// The subject runs on its own again before the sheet is cleared.
pub const CODA: f32 = 3.0;
/// The wipe that clears the sheet.
pub const WIPE: f32 = 1.4;

/// Within a part's documentation: the detail circle is drawn on the view...
pub const MARK: f32 = 0.5;
/// ...then the detail view plots in this long, and its specification types.
pub const DETAIL_PLOT: f32 = 1.6;

/// The length of a sheet documenting `parts` parts.
pub fn duration(parts: usize) -> f32 {
    PLOT_START + PLOT + SETTLE + DETAIL * parts as f32 + CODA + WIPE
}

/// Where a sheet is in its life.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Phase {
    /// The drawing is being plotted: the share of it drawn so far.
    Plot(f32),
    /// The subject runs; a part may be in focus.
    Run(Option<Focus>),
    /// The sheet is being cleared: the share of it wiped so far.
    Wipe(f32),
}

/// A part being documented.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Focus {
    pub part: usize,
    /// Seconds since it was picked out.
    pub time: f32,
}

impl Focus {
    /// The share of the detail view plotted.
    pub fn plotted(self) -> f32 {
        ((self.time - MARK) / DETAIL_PLOT).clamp(0.0, 1.0)
    }

    /// Whether everything about it has been drawn and lettered.
    pub fn settled(self) -> bool {
        self.time >= MARK + DETAIL_PLOT
    }
}

/// A moment of a sheet.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Moment {
    pub phase: Phase,
    /// Seconds since the sheet began.
    pub local: f32,
    /// Seconds the subject has been moving: none while it is plotted.
    pub run: f32,
}

impl Moment {
    pub fn at(local: f32, parts: usize) -> Self {
        let plotted = PLOT_START + PLOT;
        let run = (local - plotted).max(0.0);
        let wipe_from = duration(parts) - WIPE;

        let phase = if local < plotted {
            Phase::Plot(((local - PLOT_START) / PLOT).clamp(0.0, 1.0))
        } else if local >= wipe_from {
            Phase::Wipe(((local - wipe_from) / WIPE).clamp(0.0, 1.0))
        } else {
            let into = run - SETTLE;
            let index = (into / DETAIL).floor();

            Phase::Run((into >= 0.0 && (index as usize) < parts).then_some(Focus {
                part: index as usize,
                time: into - index * DETAIL,
            }))
        };

        Self { phase, local, run }
    }

    /// The share of the title block's lettering typed.
    pub fn typed(self) -> f32 {
        (self.local / TYPING).clamp(0.0, 1.0)
    }

    pub fn focus(self) -> Option<Focus> {
        match self.phase {
            Phase::Run(focus) => focus,
            _ => None,
        }
    }
}

/// The sheet on show and the moment it is at.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Showing {
    pub subject: usize,
    /// How many sheets the output showed before this one.
    pub serial: u64,
    pub moment: Moment,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_sheet_plots_then_runs_then_documents_each_part_then_clears() {
        let at = |local| Moment::at(local, 2).phase;

        assert_eq!(at(0.0), Phase::Plot(0.0));
        assert!(
            matches!(at(PLOT_START + PLOT / 2.0), Phase::Plot(share) if (share - 0.5).abs() < 1e-6)
        );
        assert_eq!(at(PLOT_START + PLOT + 0.1), Phase::Run(None));

        let first = PLOT_START + PLOT + SETTLE;
        assert!(matches!(
            at(first + 1.0),
            Phase::Run(Some(Focus { part: 0, .. }))
        ));
        assert!(matches!(
            at(first + DETAIL + 1.0),
            Phase::Run(Some(Focus { part: 1, .. }))
        ));
        assert_eq!(at(first + 2.0 * DETAIL + 0.5), Phase::Run(None));
        assert!(matches!(at(duration(2) - 0.1), Phase::Wipe(_)));
    }

    #[test]
    fn the_subject_does_not_move_until_it_is_plotted() {
        assert_eq!(Moment::at(3.0, 4).run, 0.0);
        assert!((Moment::at(PLOT_START + PLOT + 2.0, 4).run - 2.0).abs() < 1e-5);
    }
}
