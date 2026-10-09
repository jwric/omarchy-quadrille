//! Which subject each output shows, and when.
//!
//! Every output draws its subjects from a shuffle bag of its own: each comes
//! round once a round, in a random order, never twice running and never while
//! another output shows it. The draws come from a seeded generator and are
//! made in the order the sheets end, so the schedule is a function of the
//! seed: any moment of it can be drawn again. A sheet's length is its
//! subject's parts and how long its plot takes, which is asked for once, as
//! the sheet is put in the schedule, before it starts.
use super::timeline::{Moment, Showing, duration};

#[derive(Debug, Clone)]
pub struct Schedule {
    /// How many parts each subject documents, by subject.
    parts: Vec<usize>,
    rng: fastrand::Rng,
    /// Each output's sheets so far.
    sheets: Vec<Vec<Scheduled>>,
    /// Each output's subjects still to come this round.
    bags: Vec<Vec<usize>>,
    /// The subject the first output starts with, if one is asked for.
    first: Option<usize>,
}

/// A sheet in the schedule: when it starts, its subject and how many
/// seconds its plot takes.
#[derive(Debug, Clone, Copy)]
struct Scheduled {
    start: f32,
    subject: usize,
    plot: f32,
}

impl Scheduled {
    fn end(self, parts: &[usize]) -> f32 {
        self.start + duration(parts[self.subject], self.plot)
    }
}

impl Schedule {
    pub fn new(parts: Vec<usize>, seed: u64, first: Option<usize>) -> Self {
        Self {
            parts,
            rng: fastrand::Rng::with_seed(seed),
            sheets: Vec::new(),
            bags: Vec::new(),
            first,
        }
    }

    /// What `output` shows `elapsed` seconds in. `plot` says how long the
    /// plot of a sheet of a subject takes on an output, by their indices:
    /// it is asked once a sheet, when the sheet is put in the schedule.
    pub fn at(
        &mut self,
        output: usize,
        elapsed: f32,
        mut plot: impl FnMut(usize, usize) -> f32,
    ) -> Showing {
        while self.sheets.len() <= output {
            self.open(&mut plot);
        }

        self.extend(elapsed.max(0.0), &mut plot);

        let sheets = &self.sheets[output];
        let index = sheets
            .partition_point(|sheet| sheet.start <= elapsed)
            .max(1)
            - 1;
        let sheet = sheets[index];

        Showing {
            subject: sheet.subject,
            serial: index as u64,
            plot: sheet.plot,
            moment: Moment::at(
                (elapsed - sheet.start).max(0.0),
                self.parts[sheet.subject],
                sheet.plot,
            ),
        }
    }

    /// Starts another output's schedule at the beginning.
    fn open(&mut self, plot: &mut impl FnMut(usize, usize) -> f32) {
        let output = self.sheets.len();

        self.sheets.push(Vec::new());
        self.bags.push(Vec::new());

        let subject = match self.first.filter(|_| output == 0) {
            Some(first) => {
                self.bags[0] = (0..self.parts.len()).filter(|s| *s != first).collect();
                first
            }
            None => self.draw(output, 0.0),
        };

        self.sheets[output].push(Scheduled {
            start: 0.0,
            subject,
            plot: plot(output, subject),
        });
    }

    /// Adds sheets, in the order the outputs' last sheets end, until every
    /// output has one running at `until`.
    fn extend(&mut self, until: f32, plot: &mut impl FnMut(usize, usize) -> f32) {
        loop {
            let Some((output, end)) = (0..self.sheets.len())
                .map(|output| (output, self.end(output)))
                .min_by(|a, b| a.1.total_cmp(&b.1))
            else {
                return;
            };

            if end > until {
                return;
            }

            let subject = self.draw(output, end);

            self.sheets[output].push(Scheduled {
                start: end,
                subject,
                plot: plot(output, subject),
            });
        }
    }

    /// When `output`'s last sheet ends.
    fn end(&self, output: usize) -> f32 {
        self.sheets[output]
            .last()
            .map_or(0.0, |sheet| sheet.end(&self.parts))
    }

    /// The subject `output` shows at `at`, if its schedule reaches it.
    fn showing(&self, output: usize, at: f32) -> Option<usize> {
        let sheets = &self.sheets[output];
        let index = sheets
            .partition_point(|sheet| sheet.start <= at)
            .checked_sub(1)?;

        (at < self.end(output) || index + 1 < sheets.len()).then(|| sheets[index].subject)
    }

    /// The next subject for `output`, starting at `at`: from its bag, not the
    /// one it has just shown and none another output is showing.
    fn draw(&mut self, output: usize, at: f32) -> usize {
        let count = self.parts.len();
        let last = self.sheets[output].last().map(|sheet| sheet.subject);
        let busy: Vec<usize> = (0..self.sheets.len())
            .filter(|other| *other != output)
            .filter_map(|other| self.showing(other, at))
            .collect();
        let allowed = |subject: &usize| Some(*subject) != last && !busy.contains(subject);

        if !self.bags[output].iter().any(allowed) {
            self.bags[output] = (0..count).collect();
        }

        let candidates: Vec<usize> = self.bags[output].iter().copied().filter(allowed).collect();
        let subject = match candidates.len() {
            // Fewer subjects than outputs: anything but a repeat will do.
            0 => (0..count).find(|s| Some(*s) != last).unwrap_or(0),
            n => candidates[self.rng.usize(..n)],
        };

        self.bags[output].retain(|s| *s != subject);
        subject
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sheet::timeline::PLOT_START;

    const PARTS: [usize; 7] = [4, 5, 4, 6, 5, 4, 5];

    /// How long each subject's plot takes, on either output: a different
    /// length for each.
    fn plot(_: usize, subject: usize) -> f32 {
        6.0 + subject as f32
    }

    fn schedule(seed: u64) -> Schedule {
        Schedule::new(PARTS.to_vec(), seed, None)
    }

    /// The subjects `output` shows over `seconds`, a sheet at a time.
    fn sequence(schedule: &mut Schedule, output: usize, seconds: f32) -> Vec<usize> {
        let mut seen: Vec<(u64, usize)> = Vec::new();

        for step in 0..(seconds as usize) {
            let showing = schedule.at(output, step as f32, plot);

            if seen.last().map(|(serial, _)| *serial) != Some(showing.serial) {
                seen.push((showing.serial, showing.subject));
            }
        }

        seen.into_iter().map(|(_, subject)| subject).collect()
    }

    #[test]
    fn every_subject_comes_round_and_none_twice_running() {
        let mut schedule = schedule(7);
        let shown = sequence(&mut schedule, 0, 4.0 * 3600.0);

        assert!(shown.windows(2).all(|pair| pair[0] != pair[1]));

        // Each round of seven has all seven.
        for round in shown.chunks_exact(PARTS.len()).take(10) {
            let mut sorted = round.to_vec();
            sorted.sort();
            assert_eq!(sorted, (0..PARTS.len()).collect::<Vec<_>>(), "{shown:?}");
        }
    }

    #[test]
    fn two_outputs_never_show_the_same_subject_at_once() {
        let mut schedule = schedule(11);

        for step in 0..20_000 {
            let at = step as f32 * 0.5;
            let (a, b) = (schedule.at(0, at, plot), schedule.at(1, at, plot));

            assert_ne!(a.subject, b.subject, "both show {} at {at}", a.subject);
        }
    }

    #[test]
    fn the_order_is_random_but_the_schedule_is_the_seeds() {
        let a = sequence(&mut schedule(1), 0, 3600.0);
        let b = sequence(&mut schedule(2), 0, 3600.0);

        assert_ne!(a, b);
        assert_eq!(a, sequence(&mut schedule(1), 0, 3600.0));

        // Asking about a later moment first changes nothing.
        let mut late = schedule(1);
        let _ = late.at(0, 3000.0, plot);
        assert_eq!(sequence(&mut late, 0, 3600.0), a);
    }

    #[test]
    fn a_chosen_first_subject_comes_first() {
        let mut schedule = Schedule::new(PARTS.to_vec(), 3, Some(5));

        assert_eq!(schedule.at(0, 0.0, plot).subject, 5);
        assert_ne!(schedule.at(1, 0.0, plot).subject, 5);
    }

    /// Each sheet is plotted in its own length, asked for once as the sheet
    /// is put in the schedule, and the next sheet starts when it ends.
    #[test]
    fn a_sheet_lasts_as_long_as_its_plot_and_its_parts() {
        let mut schedule = schedule(5);
        let mut asked = Vec::new();
        let mut showings = Vec::new();

        for step in 0..2000 {
            showings.push(schedule.at(0, step as f32 * 0.25, |output, subject| {
                asked.push((output, subject));
                plot(output, subject)
            }));
        }

        let sheets = &schedule.sheets[0];

        assert_eq!(asked.len(), sheets.len());

        for pair in sheets.windows(2) {
            assert_eq!(pair[1].start, pair[0].end(&PARTS));
            assert_eq!(pair[0].plot, plot(0, pair[0].subject));
        }

        for showing in showings {
            assert_eq!(showing.plot, plot(0, showing.subject));

            // The subject is still while it is plotted, whatever its length.
            let local = showing.moment.local;

            assert_eq!(
                showing.moment.run == 0.0,
                local <= PLOT_START + showing.plot,
                "{local}"
            );
        }
    }
}
