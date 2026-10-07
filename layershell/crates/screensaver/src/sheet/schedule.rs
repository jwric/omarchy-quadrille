//! Which subject each output shows, and when.
//!
//! Every output draws its subjects from a shuffle bag of its own: each comes
//! round once a round, in a random order, never twice running and never while
//! another output shows it. The draws come from a seeded generator and are
//! made in the order the sheets end, so the schedule is a function of the
//! seed: any moment of it can be drawn again.
use super::timeline::{Moment, Showing, duration};

#[derive(Debug, Clone)]
pub struct Schedule {
    /// How many parts each subject documents, by subject: a sheet's length.
    parts: Vec<usize>,
    rng: fastrand::Rng,
    /// Each output's sheets so far, as (start, subject).
    sheets: Vec<Vec<(f32, usize)>>,
    /// Each output's subjects still to come this round.
    bags: Vec<Vec<usize>>,
    /// The subject the first output starts with, if one is asked for.
    first: Option<usize>,
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

    /// What `output` shows `elapsed` seconds in.
    pub fn at(&mut self, output: usize, elapsed: f32) -> Showing {
        while self.sheets.len() <= output {
            self.open();
        }

        self.extend(elapsed.max(0.0));

        let sheets = &self.sheets[output];
        let index = sheets
            .partition_point(|(start, _)| *start <= elapsed)
            .max(1)
            - 1;
        let (start, subject) = sheets[index];

        Showing {
            subject,
            serial: index as u64,
            moment: Moment::at((elapsed - start).max(0.0), self.parts[subject]),
        }
    }

    /// Starts another output's schedule at the beginning.
    fn open(&mut self) {
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

        self.sheets[output].push((0.0, subject));
    }

    /// Adds sheets, in the order the outputs' last sheets end, until every
    /// output has one running at `until`.
    fn extend(&mut self, until: f32) {
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
            self.sheets[output].push((end, subject));
        }
    }

    /// When `output`'s last sheet ends.
    fn end(&self, output: usize) -> f32 {
        self.sheets[output].last().map_or(0.0, |&(start, subject)| {
            start + duration(self.parts[subject])
        })
    }

    /// The subject `output` shows at `at`, if its schedule reaches it.
    fn showing(&self, output: usize, at: f32) -> Option<usize> {
        let sheets = &self.sheets[output];
        let index = sheets
            .partition_point(|(start, _)| *start <= at)
            .checked_sub(1)?;

        (at < self.end(output) || index + 1 < sheets.len()).then(|| sheets[index].1)
    }

    /// The next subject for `output`, starting at `at`: from its bag, not the
    /// one it has just shown and none another output is showing.
    fn draw(&mut self, output: usize, at: f32) -> usize {
        let count = self.parts.len();
        let last = self.sheets[output].last().map(|&(_, subject)| subject);
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

    const PARTS: [usize; 7] = [4, 5, 4, 6, 5, 4, 5];

    fn schedule(seed: u64) -> Schedule {
        Schedule::new(PARTS.to_vec(), seed, None)
    }

    /// The subjects `output` shows over `seconds`, a sheet at a time.
    fn sequence(schedule: &mut Schedule, output: usize, seconds: f32) -> Vec<usize> {
        let mut seen: Vec<(u64, usize)> = Vec::new();

        for step in 0..(seconds as usize) {
            let showing = schedule.at(output, step as f32);

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
            let (a, b) = (schedule.at(0, at), schedule.at(1, at));

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
        let _ = late.at(0, 3000.0);
        assert_eq!(sequence(&mut late, 0, 3600.0), a);
    }

    #[test]
    fn a_chosen_first_subject_comes_first() {
        let mut schedule = Schedule::new(PARTS.to_vec(), 3, Some(5));

        assert_eq!(schedule.at(0, 0.0).subject, 5);
        assert_ne!(schedule.at(1, 0.0).subject, 5);
    }
}
