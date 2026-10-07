//! Drawing scales: how much larger a drawing is than its subject.
//!
//! A sheet is plotted at a preferred scale (ISO 5455's 1, 2 and 5 times a
//! power of ten, with DIN 823's 2.5), and on a calibrated display the scale is
//! true: a 2:1 drawing of a 20 mm part measures 40 mm on the glass.

/// The size of a drawing over the size of what it shows.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct Ratio(pub f64);

impl Ratio {
    /// The largest preferred scale no larger than `limit`: an enlargement
    /// of 1, 2, 2.5 or 5 times a power of ten, or a reduction by one.
    pub fn preferred_at_most(limit: f64) -> Option<Self> {
        const STEPS: [f64; 5] = [1.0, 2.0, 2.5, 5.0, 10.0];

        if !(limit.is_finite() && limit > 0.0) {
            return None;
        }

        let decade = |n: f64| 10f64.powf(n.log10().floor());
        let slack = 1.0 + 1e-9;

        if limit >= 1.0 {
            let power = decade(limit);

            STEPS
                .iter()
                .rev()
                .map(|step| step * power)
                .find(|ratio| *ratio <= limit * slack)
                .map(Self)
        } else {
            // The smallest reduction 1:N that is no larger than the limit.
            let wanted = 1.0 / limit;
            let power = decade(wanted);

            STEPS
                .iter()
                .map(|step| step * power)
                .find(|n| *n * slack >= wanted)
                .map(|n| Self(1.0 / n))
        }
    }

    /// How the title block writes it: `2:1`, `1:1`, `1:5 000`, and past a
    /// hundred thousand in powers of ten, `1:5×10⁸`, so it fits its cell.
    pub fn label(self) -> String {
        let side = |n: f64| {
            if n >= 1e5 {
                let exponent = n.log10().floor();
                let mantissa = n / 10f64.powf(exponent);
                let power = format!("10{}", superscript(exponent as u32));

                if (mantissa - 1.0).abs() < 1e-6 {
                    power
                } else {
                    format!("{}×{power}", decimal(mantissa))
                }
            } else {
                decimal(n)
            }
        };

        if self.0 >= 1.0 {
            format!("{}:1", side(self.0))
        } else {
            format!("1:{}", side(1.0 / self.0))
        }
    }
}

/// A scale's number: whole and grouped, or 2.5.
fn decimal(n: f64) -> String {
    if (n - n.round()).abs() > 1e-6 {
        format!("{n:.1}")
    } else {
        grouped(n.round() as u64)
    }
}

fn superscript(n: u32) -> String {
    n.to_string()
        .chars()
        .map(|digit| {
            ['⁰', '¹', '²', '³', '⁴', '⁵', '⁶', '⁷', '⁸', '⁹'][digit as usize - '0' as usize]
        })
        .collect()
}

/// `n` with its digits in groups of three, parted by spaces.
pub fn grouped(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::new();

    for (i, digit) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(' ');
        }
        out.push(digit);
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_scale_is_the_largest_preferred_one_that_fits() {
        assert_eq!(Ratio::preferred_at_most(3.7), Some(Ratio(2.5)));
        assert_eq!(Ratio::preferred_at_most(2.3), Some(Ratio(2.0)));
        assert_eq!(Ratio::preferred_at_most(1.0), Some(Ratio(1.0)));
        assert_eq!(
            Ratio::preferred_at_most(0.31).map(Ratio::label),
            Some("1:5".into())
        );
        assert_eq!(Ratio::preferred_at_most(0.0), None);
    }

    #[test]
    fn scales_are_written_as_drawings_write_them() {
        assert_eq!(Ratio(5.0).label(), "5:1");
        assert_eq!(Ratio(2.5).label(), "2.5:1");
        assert_eq!(Ratio(0.4).label(), "1:2.5");
        assert_eq!(Ratio(1.0).label(), "1:1");
        assert_eq!(Ratio(0.0002).label(), "1:5 000");
        assert_eq!(Ratio(2e-9).label(), "1:5×10⁸");
        assert_eq!(Ratio(4e-9).label(), "1:2.5×10⁸");
        assert_eq!(Ratio(1e-9).label(), "1:10⁹");
    }
}
