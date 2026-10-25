//! Volume math (`docs/specs/02-playback.md` §2.1): linear gain ↔ cubic level ↔ percent ↔ dB.
//!
//! `g` is a PipeWire linear amplitude gain, `ℓ = ∛g` the level (1.0 == 100 % == 0 dB),
//! `p = round_half_up(100·ℓ)` the percent shown to the user.

/// Highest level vol writes (150 %).
pub const LEVEL_MAX: f32 = 1.5;
/// Highest percent vol writes.
pub const PERCENT_MAX: i32 = 150;

/// Gains at or below this (≤ −100 dB) have no finite dB value.
const DB_FLOOR_GAIN: f32 = 1e-5;

/// Level of a linear gain; negative gains count as 0.
pub fn gain_to_level(g: f32) -> f32 {
    g.max(0.0).cbrt()
}

/// Linear gain of a level clamped to `0..=LEVEL_MAX`.
pub fn level_to_gain(l: f32) -> f32 {
    l.clamp(0.0, LEVEL_MAX).powi(3)
}

/// Percent of a level, rounded half up and clamped at 0. There is no upper clamp: levels
/// above 150 % report their true percent (VOL-VM-003).
pub fn level_to_percent(l: f32) -> i32 {
    ((l * 100.0 + 0.5).floor() as i32).max(0)
}

/// Level of a percent clamped to `0..=PERCENT_MAX`.
pub fn percent_to_level(p: i32) -> f32 {
    p.clamp(0, PERCENT_MAX) as f32 / 100.0
}

/// Gain in dB; `None` for gains ≤ 1e-5 (≤ −100 dB, shown as `-inf`).
pub fn gain_to_db(g: f32) -> Option<f32> {
    (g > DB_FLOOR_GAIN).then(|| 20.0 * g.log10())
}

/// dB text of a gain: `"-inf dB"`, `" 0.0 dB"` within ±0.05 dB, else signed with one decimal.
pub fn fmt_db(g: f32) -> String {
    match gain_to_db(g) {
        None => "-inf dB".to_owned(),
        Some(db) if db.abs() < 0.05 => " 0.0 dB".to_owned(),
        Some(db) => format!("{db:+.1} dB"),
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;
    use crate::model::{Channel, NodeAudio};

    fn audio(volume: f32, channel_volumes: &[f32]) -> NodeAudio {
        NodeAudio {
            volume,
            channel_volumes: channel_volumes.to_vec(),
            mute: false,
            channel_map: vec![Channel(3), Channel(4)],
        }
    }

    fn percent_to_gain(p: i32) -> f32 {
        level_to_gain(percent_to_level(p))
    }

    #[test]
    fn anchors_100_percent_is_gain_one() {
        assert_eq!(percent_to_gain(100), 1.0);
        assert_eq!(level_to_percent(gain_to_level(1.0)), 100);
        assert_eq!(gain_to_db(1.0), Some(0.0));
    }

    #[test]
    fn anchors_150_percent_is_3_375() {
        assert_eq!(percent_to_gain(150), 3.375);
        assert_eq!(level_to_percent(gain_to_level(3.375)), 150);
        assert_eq!(level_to_gain(2.0), 3.375, "writes are capped at 150 %");
        assert_eq!(percent_to_level(151), LEVEL_MAX);
    }

    #[test]
    fn fmt_db_anchors() {
        assert_eq!(fmt_db(level_to_gain(1.0)), " 0.0 dB");
        assert_eq!(fmt_db(level_to_gain(1.5)), "+10.6 dB");
        assert_eq!(fmt_db(0.0), "-inf dB");
        assert_eq!(fmt_db(percent_to_gain(50)), "-18.1 dB");
        assert_eq!(fmt_db(percent_to_gain(87)), "-3.6 dB");
    }

    #[test]
    fn observed_pw_dump_value_shows_87_percent() {
        let observed = audio(1.0, &[0.658_492, 0.658_492]);
        assert_eq!(observed.master_percent(), Some(87));
        assert_eq!(observed.channel_percents(), [87, 87]);
    }

    #[test]
    fn effective_gain_multiplies_master_volume() {
        let a = audio(0.5, &[1.0, 0.25]);
        assert_eq!(a.effective_gains(), [0.5, 0.125]);
        assert_eq!(a.master_gain(), Some(0.5));
        assert_eq!(a.master_level(), Some(0.5f32.cbrt()));
        // ∛0.5 = 0.7937 → 79 %; ∛0.125 = 0.5 → 50 %.
        assert_eq!(a.master_percent(), Some(79));
        assert_eq!(a.channel_percents(), [79, 50]);
    }

    #[test]
    fn master_is_max_over_channels() {
        let a = audio(1.0, &[0.125, 1.0]);
        assert_eq!(a.master_percent(), Some(100));
        assert_eq!(a.channel_percents(), [50, 100]);
    }

    #[test]
    fn empty_channel_volumes_has_no_master() {
        let a = audio(1.0, &[]);
        assert_eq!(a.effective_gains(), Vec::<f32>::new());
        assert_eq!(a.master_gain(), None);
        assert_eq!(a.master_level(), None);
        assert_eq!(a.master_percent(), None);
        assert_eq!(a.channel_percents(), Vec::<i32>::new());
    }

    #[test]
    fn over_150_reports_true_percent() {
        // 2.1³ = 9.261 (below the observed `channelmix.max-volume` of 10.0).
        let a = audio(1.0, &[9.261, 1.0]);
        assert_eq!(a.master_percent(), Some(210));
        assert_eq!(a.channel_percents(), [210, 100]);
        assert_eq!(fmt_db(9.261), "+19.3 dB");
    }

    proptest! {
        #[test]
        fn roundtrip_percent_0_to_150_is_identity(p in 0i32..=PERCENT_MAX) {
            let gain: f32 = level_to_gain(percent_to_level(p));
            prop_assert_eq!(level_to_percent(gain_to_level(gain)), p);
        }

        #[test]
        fn db_is_monotonic_in_percent(p1 in 0i32..=PERCENT_MAX, p2 in 0i32..=PERCENT_MAX) {
            prop_assume!(p1 < p2);
            let (d1, d2) = (gain_to_db(percent_to_gain(p1)), gain_to_db(percent_to_gain(p2)));
            match (d1, d2) {
                (Some(d1), Some(d2)) => prop_assert!(d1 < d2, "{p1}% {d1} dB, {p2}% {d2} dB"),
                (Some(_), None) => prop_assert!(false, "{p1}% finite but {p2}% is -inf"),
                (None, _) => prop_assert!(p1 < 3, "{p1}% is -inf"),
            }
        }
    }
}
