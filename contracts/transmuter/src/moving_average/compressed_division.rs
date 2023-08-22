use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Decimal, Timestamp, Uint64};

use crate::ContractError;

/// CompressedDivision is a compressed representation of a compressed sliding window
/// for calculating approximated simple moving average.
#[cw_serde]
pub struct CompressedDivision {
    pub start_time: Timestamp,
    pub cumsum: Decimal,
    pub n: Uint64,
}

impl CompressedDivision {
    pub fn start(start_time: Timestamp, value: Decimal) -> Self {
        Self {
            start_time,
            cumsum: value,
            n: Uint64::one(),
        }
    }

    pub fn accum(&self, value: Decimal) -> Result<Self, ContractError> {
        Ok(CompressedDivision {
            start_time: self.start_time,
            cumsum: self
                .cumsum
                .checked_add(value)
                .map_err(ContractError::calculation_error)?,
            n: self
                .n
                .checked_add(Uint64::one())
                .map_err(ContractError::calculation_error)?,
        })
    }

    pub fn elasped_time(&self, block_time: Timestamp) -> Result<Uint64, ContractError> {
        Uint64::from(block_time.nanos())
            .checked_sub(self.start_time.nanos().into())
            .map_err(ContractError::calculation_error)
    }

    pub fn average(&self) -> Result<Decimal, ContractError> {
        let n = Decimal::from_atomics(self.n, 0).map_err(ContractError::calculation_error)?;

        self.cumsum
            .checked_div(n)
            .map_err(ContractError::calculation_error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compressed_division() {
        // Create a new CompressedDivision
        let start_time = Timestamp::from_nanos(0);
        let value0 = Decimal::percent(10);
        let compressed_division = CompressedDivision::start(start_time, value0);

        // Accumulate values
        let value1 = Decimal::percent(30);
        let value2 = Decimal::percent(40);
        let value3 = Decimal::percent(50);
        let updated_division = compressed_division
            .accum(value1)
            .unwrap()
            .accum(value2)
            .unwrap()
            .accum(value3)
            .unwrap();

        // Verify the accumulated values
        assert_eq!(updated_division.start_time, start_time);
        assert_eq!(updated_division.cumsum, value0 + value1 + value2 + value3);
        assert_eq!(updated_division.n, Uint64::from(4u64));
    }
}

mod v2 {
    use cosmwasm_schema::cw_serde;
    use cosmwasm_std::{ensure, Decimal, StdError, Timestamp, Uint64};

    use crate::ContractError;

    /// CompressedDivision is a compressed representation of a compressed sliding window
    /// for calculating approximated moving average.
    #[cw_serde]
    pub struct CompressedDivision {
        /// Time where the division is mark as started
        started_at: Timestamp,

        /// Time where it is last updated
        updated_at: Timestamp,

        /// The latest value that gets updated
        latest_value: Decimal,

        /// cumulative sum of each updated value * elasped time since last update
        cumsum: Decimal,
    }

    impl CompressedDivision {
        pub fn new(
            started_at: Timestamp,
            updated_at: Timestamp,
            value: Decimal,
            prev_value: Decimal,
        ) -> Result<Self, ContractError> {
            ensure!(
                updated_at >= started_at,
                ContractError::change_limit_error(
                    "`updated_at` must be greater than or equal to `started_at`"
                )
            );

            let elapsed_time =
                Uint64::from(updated_at.nanos()).checked_sub(started_at.nanos().into())?;
            Ok(Self {
                started_at,
                updated_at,
                latest_value: value,
                cumsum: prev_value
                    .checked_mul(Decimal::checked_from_ratio(elapsed_time, 1u128)?)?,
            })
        }

        pub fn update(&self, updated_at: Timestamp, value: Decimal) -> Result<Self, ContractError> {
            let elapsed_time =
                Uint64::from(updated_at.nanos()).checked_sub(self.updated_at.nanos().into())?;
            Ok(Self {
                started_at: self.started_at,
                updated_at,
                latest_value: value,
                cumsum: self.cumsum.checked_add(
                    self.latest_value
                        .checked_mul(Decimal::checked_from_ratio(elapsed_time, 1u128)?)?,
                )?,
            })
        }

        // weighted average
        // cumsum_elasped_time = updated_at - started_at
        // latest_value_elasped_time = block_time - updated_at
        // total_elasped_time = block_time - started_at
        // ((cumsum * cumsum_elasped_time) + (latest_value * latest_value_elasped_time)) / total_elasped_time

        // [Assumption] divisions are sorted by started_at and last division's updated_at is less than block_time
        pub fn average(
            mut divisions: impl Iterator<Item = Self>,
            division_size: Uint64,
            window_size: Uint64,
            block_time: Timestamp,
        ) -> Result<Decimal, ContractError> {
            let window_started_at = Uint64::from(block_time.nanos()).checked_sub(window_size)?;

            // Process first division
            let (first_div_stared_at, mut cumsum) = match divisions.next() {
                Some(division) => {
                    let division_started_at = Uint64::from(division.started_at.nanos());
                    let remaining_division_size = division_started_at
                        .checked_add(division_size)?
                        .checked_sub(window_started_at)?
                        .min(division_size);

                    let latest_value_elapsed_time =
                        division.latest_value_elapsed_time(division_size, block_time)?;

                    if remaining_division_size > latest_value_elapsed_time {
                        let current_cumsum_weight = Uint64::from(division.updated_at.nanos())
                            .checked_sub(division.started_at.nanos().into())?;

                        // recalculate cumsum if window start after first division
                        let cumsum = if window_started_at > division_started_at {
                            let new_cumsum_weight =
                                remaining_division_size.checked_sub(latest_value_elapsed_time)?;

                            let division_average_before_latest_update =
                                division.cumsum.checked_div(Decimal::checked_from_ratio(
                                    current_cumsum_weight,
                                    1u128,
                                )?)?;

                            division_average_before_latest_update.checked_mul(
                                Decimal::checked_from_ratio(new_cumsum_weight, 1u128)?,
                            )?
                        } else {
                            division.cumsum
                        };

                        (
                            division.started_at,
                            cumsum.checked_add(
                                division.weighted_latest_value(division_size, block_time)?,
                            )?,
                        )
                    } else {
                        (
                            division.started_at,
                            division
                                .latest_value
                                .checked_mul(Decimal::checked_from_ratio(
                                    remaining_division_size,
                                    1u128,
                                )?)?,
                        )
                    }
                }
                None => return Err(StdError::not_found("division").into()),
            };

            // Accumulate divisions until the last division's updated_at is less than block_time
            for division in divisions {
                cumsum = cumsum
                    .checked_add(division.cumsum_at_block_time(division_size, block_time)?)?;
            }

            let started_at = window_started_at.max(first_div_stared_at.nanos().into());
            let total_elapsed_time = Uint64::from(block_time.nanos()).checked_sub(started_at)?;

            cumsum
                .checked_div(Decimal::checked_from_ratio(total_elapsed_time, 1u128)?)
                .map_err(Into::into)
        }

        fn cumsum_at_block_time(
            &self,
            division_size: Uint64,
            block_time: Timestamp,
        ) -> Result<Decimal, ContractError> {
            self.cumsum
                .checked_add(self.weighted_latest_value(division_size, block_time)?)
                .map_err(Into::into)
        }

        fn latest_value_elapsed_time(
            &self,
            division_size: Uint64,
            block_time: Timestamp,
        ) -> Result<Uint64, ContractError> {
            let ended_at = Uint64::from(self.started_at.nanos()).checked_add(division_size)?;
            let block_time = Uint64::from(block_time.nanos());
            if block_time > ended_at {
                ended_at.checked_sub(self.updated_at.nanos().into())
            } else {
                block_time.checked_sub(self.updated_at.nanos().into())
            }
            .map_err(Into::into)
        }

        fn weighted_latest_value(
            &self,
            division_size: Uint64,
            block_time: Timestamp,
        ) -> Result<Decimal, ContractError> {
            let elapsed_time = self.latest_value_elapsed_time(division_size, block_time)?;
            self.latest_value
                .checked_mul(Decimal::checked_from_ratio(elapsed_time, 1u128)?)
                .map_err(Into::into)
        }
    }

    #[cfg(test)]
    mod tests {
        use cosmwasm_std::StdError;

        use super::*;

        #[test]
        fn test_new_compressed_division() {
            // started_at < updated_at
            let started_at = Timestamp::from_nanos(90);
            let updated_at = Timestamp::from_nanos(100);
            let value = Decimal::percent(10);
            let prev_value = Decimal::percent(10);
            let compressed_division =
                CompressedDivision::new(started_at, updated_at, value, prev_value).unwrap();

            assert_eq!(
                compressed_division,
                CompressedDivision {
                    started_at,
                    updated_at,
                    latest_value: value,
                    cumsum: Decimal::percent(10) * Decimal::from_ratio(10u128, 1u128)
                }
            );

            // started_at == updated_at
            let started_at = Timestamp::from_nanos(90);
            let updated_at = Timestamp::from_nanos(90);

            let compressed_division =
                CompressedDivision::new(started_at, updated_at, value, prev_value).unwrap();

            assert_eq!(
                compressed_division,
                CompressedDivision {
                    started_at,
                    updated_at,
                    latest_value: value,
                    cumsum: Decimal::zero()
                }
            );

            // started_at > updated_at
            let started_at = Timestamp::from_nanos(90);
            let updated_at = Timestamp::from_nanos(89);

            let err =
                CompressedDivision::new(started_at, updated_at, value, prev_value).unwrap_err();

            assert_eq!(
                err,
                ContractError::change_limit_error(
                    "`updated_at` must be greater than or equal to `started_at`"
                )
            );
        }

        #[test]
        fn test_update_compressed_division() {
            let started_at = Timestamp::from_nanos(90);
            let updated_at = Timestamp::from_nanos(100);
            let value = Decimal::percent(20);
            let prev_value = Decimal::percent(10);
            let compressed_division =
                CompressedDivision::new(started_at, updated_at, value, prev_value).unwrap();

            let updated_at = Timestamp::from_nanos(120);
            let value = Decimal::percent(20);
            let updated_compressed_division =
                compressed_division.update(updated_at, value).unwrap();

            assert_eq!(
                updated_compressed_division,
                CompressedDivision {
                    started_at,
                    updated_at,
                    latest_value: value,
                    cumsum: (Decimal::percent(10) * Decimal::from_ratio(10u128, 1u128))
                        + (Decimal::percent(20) * Decimal::from_ratio(20u128, 1u128))
                }
            );
        }

        #[test]
        fn test_average_empty_iter() {
            let divisions = vec![];
            let division_size = Uint64::from(100u64);
            let window_size = Uint64::from(1000u64);
            let block_time = Timestamp::from_nanos(1100);
            let average = CompressedDivision::average(
                divisions.into_iter(),
                division_size,
                window_size,
                block_time,
            );

            assert_eq!(
                average.unwrap_err(),
                ContractError::Std(StdError::not_found("division"))
            );
        }

        #[test]
        fn test_average_single_div() {
            let started_at = Timestamp::from_nanos(1100);
            let updated_at = Timestamp::from_nanos(1110);
            let value = Decimal::percent(20);
            let prev_value = Decimal::percent(10);
            let compressed_division =
                CompressedDivision::new(started_at, updated_at, value, prev_value).unwrap();

            let divisions = vec![compressed_division];
            let division_size = Uint64::from(100u64);
            let window_size = Uint64::from(1000u64);
            let block_time = Timestamp::from_nanos(1110);
            let average = CompressedDivision::average(
                divisions.clone().into_iter(),
                division_size,
                window_size,
                block_time,
            )
            .unwrap();

            // used to be x 10 / 10
            // but now it is x 100 / 10
            assert_eq!(average, prev_value);

            let block_time = Timestamp::from_nanos(1115);
            let average = CompressedDivision::average(
                divisions.clone().into_iter(),
                division_size,
                window_size,
                block_time,
            )
            .unwrap();

            assert_eq!(
                average,
                ((prev_value * Decimal::from_ratio(10u128, 1u128))
                    + (value * Decimal::from_ratio(5u128, 1u128)))
                    / Decimal::from_ratio(15u128, 1u128)
            );

            // half way to the division size
            let block_time = Timestamp::from_nanos(1150);
            let average = CompressedDivision::average(
                divisions.clone().into_iter(),
                division_size,
                window_size,
                block_time,
            )
            .unwrap();

            assert_eq!(
                average,
                ((prev_value * Decimal::from_ratio(10u128, 1u128))
                    + (value * Decimal::from_ratio(40u128, 1u128)))
                    / Decimal::from_ratio(50u128, 1u128)
            );

            // at the division edge
            let block_time = Timestamp::from_nanos(1200);
            let average = CompressedDivision::average(
                divisions.clone().into_iter(),
                division_size,
                window_size,
                block_time,
            )
            .unwrap();

            assert_eq!(
                average,
                ((prev_value * Decimal::from_ratio(10u128, 1u128))
                    + (value * Decimal::from_ratio(90u128, 1u128)))
                    / Decimal::from_ratio(100u128, 1u128)
            );

            // at the division edge but there is some update before
            let update_time = Timestamp::from_nanos(1150);
            let updated_value = Decimal::percent(30);

            let updated_division = divisions
                .into_iter()
                .next()
                .unwrap()
                .update(update_time, updated_value)
                .unwrap();

            let divisions = vec![updated_division];

            let block_time = Timestamp::from_nanos(1200);
            let average = CompressedDivision::average(
                divisions.into_iter(),
                division_size,
                window_size,
                block_time,
            )
            .unwrap();

            assert_eq!(
                average,
                ((prev_value * Decimal::from_ratio(10u128, 1u128))
                    + (value * Decimal::from_ratio(40u128, 1u128))
                    + (updated_value * Decimal::from_ratio(50u128, 1u128)))
                    / Decimal::from_ratio(100u128, 1u128)
            );
        }

        #[test]
        fn test_average_double_divs() {
            let division_size = Uint64::from(100u64);
            let window_size = Uint64::from(1000u64);

            let divisions = vec![
                {
                    let started_at = Timestamp::from_nanos(1100);
                    let updated_at = Timestamp::from_nanos(1110);
                    let value = Decimal::percent(20);
                    let prev_value = Decimal::percent(10);
                    CompressedDivision::new(started_at, updated_at, value, prev_value).unwrap()
                },
                {
                    let started_at = Timestamp::from_nanos(1200);
                    let updated_at = Timestamp::from_nanos(1260);
                    let value = Decimal::percent(30);
                    let prev_value = Decimal::percent(20);
                    CompressedDivision::new(started_at, updated_at, value, prev_value).unwrap()
                },
            ];

            let block_time = Timestamp::from_nanos(1270);
            let average = CompressedDivision::average(
                divisions.into_iter(),
                division_size,
                window_size,
                block_time,
            )
            .unwrap();

            assert_eq!(
                average,
                ((Decimal::percent(10) * Decimal::from_ratio(10u128, 1u128))
                    + (Decimal::percent(20) * Decimal::from_ratio(90u128, 1u128))
                    + (Decimal::percent(20) * Decimal::from_ratio(60u128, 1u128))
                    + (Decimal::percent(30) * Decimal::from_ratio(10u128, 1u128)))
                    / Decimal::from_ratio(170u128, 1u128)
            );
        }

        #[test]
        fn test_average_tripple_divs() {
            let division_size = Uint64::from(100u64);
            let window_size = Uint64::from(1000u64);

            let divisions = vec![
                {
                    let started_at = Timestamp::from_nanos(1100);
                    let updated_at = Timestamp::from_nanos(1110);
                    let value = Decimal::percent(20);
                    let prev_value = Decimal::percent(10);
                    CompressedDivision::new(started_at, updated_at, value, prev_value).unwrap()
                },
                {
                    let started_at = Timestamp::from_nanos(1200);
                    let updated_at = Timestamp::from_nanos(1260);
                    let value = Decimal::percent(30);
                    let prev_value = Decimal::percent(20);
                    CompressedDivision::new(started_at, updated_at, value, prev_value).unwrap()
                },
                {
                    let started_at = Timestamp::from_nanos(1300);
                    let updated_at = Timestamp::from_nanos(1340);
                    let value = Decimal::percent(40);
                    let prev_value = Decimal::percent(30);
                    CompressedDivision::new(started_at, updated_at, value, prev_value).unwrap()
                },
            ];

            let block_time = Timestamp::from_nanos(1370);

            let average = CompressedDivision::average(
                divisions.into_iter(),
                division_size,
                window_size,
                block_time,
            )
            .unwrap();

            assert_eq!(
                average,
                ((Decimal::percent(10) * Decimal::from_ratio(10u128, 1u128))
                    + (Decimal::percent(20) * Decimal::from_ratio(90u128, 1u128))
                    + (Decimal::percent(20) * Decimal::from_ratio(60u128, 1u128))
                    + (Decimal::percent(30) * Decimal::from_ratio(40u128, 1u128))
                    + (Decimal::percent(30) * Decimal::from_ratio(40u128, 1u128))
                    + (Decimal::percent(40) * Decimal::from_ratio(30u128, 1u128)))
                    / Decimal::from_ratio(270u128, 1u128)
            );
        }

        #[test]
        fn test_average_when_div_is_in_overlapping_window() {
            let division_size = Uint64::from(200u64);
            let window_size = Uint64::from(600u64);

            let divisions = vec![
                {
                    let started_at = Timestamp::from_nanos(1100);
                    let updated_at = Timestamp::from_nanos(1110);
                    let value = Decimal::percent(20);
                    let prev_value = Decimal::percent(10);
                    CompressedDivision::new(started_at, updated_at, value, prev_value).unwrap()
                },
                {
                    let started_at = Timestamp::from_nanos(1300);
                    let updated_at = Timestamp::from_nanos(1360);
                    let value = Decimal::percent(30);
                    let prev_value = Decimal::percent(20);
                    CompressedDivision::new(started_at, updated_at, value, prev_value).unwrap()
                },
                {
                    let started_at = Timestamp::from_nanos(1500);
                    let updated_at = Timestamp::from_nanos(1640);
                    let value = Decimal::percent(40);
                    let prev_value = Decimal::percent(30);
                    CompressedDivision::new(started_at, updated_at, value, prev_value).unwrap()
                },
            ];

            let block_time = Timestamp::from_nanos(1700);

            let average = CompressedDivision::average(
                divisions.clone().into_iter(),
                division_size,
                window_size,
                block_time,
            )
            .unwrap();

            assert_eq!(
                average,
                ((Decimal::percent(10) * Decimal::from_ratio(10u128, 1u128))
                    + (Decimal::percent(20) * Decimal::from_ratio(190u128, 1u128))
                    + (Decimal::percent(20) * Decimal::from_ratio(60u128, 1u128))
                    + (Decimal::percent(30) * Decimal::from_ratio(140u128, 1u128))
                    + (Decimal::percent(30) * Decimal::from_ratio(140u128, 1u128))
                    + (Decimal::percent(40) * Decimal::from_ratio(60u128, 1u128)))
                    / Decimal::from_ratio(600u128, 1u128)
            );

            let base_divisions = divisions;

            let divisions = vec![
                base_divisions.clone(),
                vec![{
                    let started_at = Timestamp::from_nanos(1700);
                    let updated_at = Timestamp::from_nanos(1700);
                    let value = Decimal::percent(50);
                    let prev_value = Decimal::percent(40);
                    CompressedDivision::new(started_at, updated_at, value, prev_value).unwrap()
                }],
            ]
            .concat();

            let block_time = Timestamp::from_nanos(1705);

            let average = CompressedDivision::average(
                divisions.into_iter(),
                division_size,
                window_size,
                block_time,
            )
            .unwrap();

            assert_eq!(
                average,
                ((Decimal::percent(10) * Decimal::from_ratio(5u128, 1u128))
                    + (Decimal::percent(20) * Decimal::from_ratio(190u128, 1u128))
                    + (Decimal::percent(20) * Decimal::from_ratio(60u128, 1u128))
                    + (Decimal::percent(30) * Decimal::from_ratio(140u128, 1u128))
                    + (Decimal::percent(30) * Decimal::from_ratio(140u128, 1u128))
                    + (Decimal::percent(40) * Decimal::from_ratio(60u128, 1u128))
                    + (Decimal::percent(50) * Decimal::from_ratio(5u128, 1u128)))
                    / Decimal::from_ratio(600u128, 1u128)
            );

            let divisions = vec![
                base_divisions.clone(),
                vec![{
                    let started_at = Timestamp::from_nanos(1700);
                    let updated_at = Timestamp::from_nanos(1701);
                    let value = Decimal::percent(50);
                    let prev_value = Decimal::percent(40);
                    CompressedDivision::new(started_at, updated_at, value, prev_value).unwrap()
                }],
            ]
            .concat();

            let block_time = Timestamp::from_nanos(1705);

            let average = CompressedDivision::average(
                divisions.into_iter(),
                division_size,
                window_size,
                block_time,
            )
            .unwrap();

            assert_eq!(
                average,
                ((Decimal::percent(10) * Decimal::from_ratio(5u128, 1u128))
                    + (Decimal::percent(20) * Decimal::from_ratio(190u128, 1u128))
                    + (Decimal::percent(20) * Decimal::from_ratio(60u128, 1u128))
                    + (Decimal::percent(30) * Decimal::from_ratio(140u128, 1u128))
                    + (Decimal::percent(30) * Decimal::from_ratio(140u128, 1u128))
                    + (Decimal::percent(40) * Decimal::from_ratio(60u128, 1u128))
                    + (Decimal::percent(40) * Decimal::from_ratio(1u128, 1u128))
                    + (Decimal::percent(50) * Decimal::from_ratio(4u128, 1u128)))
                    / Decimal::from_ratio(600u128, 1u128)
            );

            let divisions = vec![
                base_divisions,
                vec![{
                    let started_at = Timestamp::from_nanos(1700);
                    let updated_at = Timestamp::from_nanos(1740);
                    let value = Decimal::percent(50);
                    let prev_value = Decimal::percent(40);
                    CompressedDivision::new(started_at, updated_at, value, prev_value).unwrap()
                }],
            ]
            .concat();

            let block_time = Timestamp::from_nanos(1740);

            let average = CompressedDivision::average(
                divisions.clone().into_iter(),
                division_size,
                window_size,
                block_time,
            )
            .unwrap();

            assert_eq!(
                average,
                ((Decimal::percent(20) * Decimal::from_ratio(160u128, 1u128)) // 32
                    + (Decimal::percent(20) * Decimal::from_ratio(60u128, 1u128)) // 32 + 12 = 44
                    + (Decimal::percent(30) * Decimal::from_ratio(140u128, 1u128)) // 44 + 42 = 86
                    + (Decimal::percent(30) * Decimal::from_ratio(140u128, 1u128)) // 86 + 42 = 128
                    + (Decimal::percent(40) * Decimal::from_ratio(60u128, 1u128)) // 128 + 24 = 152
                    + (Decimal::percent(40) * Decimal::from_ratio(40u128, 1u128))) // 152 + 16 = 168
                    / Decimal::from_ratio(600u128, 1u128)
            );

            let block_time = Timestamp::from_nanos(1899);

            let average = CompressedDivision::average(
                divisions.into_iter(),
                division_size,
                window_size,
                block_time,
            )
            .unwrap();

            assert_eq!(
                average,
                ((Decimal::percent(20) * Decimal::from_ratio(1u128, 1u128))
                    + (Decimal::percent(20) * Decimal::from_ratio(60u128, 1u128))
                    + (Decimal::percent(30) * Decimal::from_ratio(140u128, 1u128))
                    + (Decimal::percent(30) * Decimal::from_ratio(140u128, 1u128))
                    + (Decimal::percent(40) * Decimal::from_ratio(60u128, 1u128))
                    + (Decimal::percent(40) * Decimal::from_ratio(40u128, 1u128))
                    + (Decimal::percent(50) * Decimal::from_ratio(159u128, 1u128)))
                    / Decimal::from_ratio(600u128, 1u128)
            );
        }
    }
}
