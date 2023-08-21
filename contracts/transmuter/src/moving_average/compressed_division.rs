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
    use cosmwasm_std::{Decimal, StdError, Timestamp, Uint64};

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

        pub fn average(
            mut divisions: impl Iterator<Item = Self>,
            division_size: Uint64,
            window_size: Uint64,
            block_time: Timestamp,
        ) -> Result<Decimal, ContractError> {
            match divisions.next() {
                Some(CompressedDivision {
                    started_at,
                    updated_at,
                    latest_value,
                    cumsum,
                }) => {
                    // weighted average
                    // cumsum_elasped_time = updated_at - started_at
                    // latest_value_elasped_time = block_time - updated_at
                    // total_elasped_time = block_time - started_at
                    // ((cumsum * cumsum_elasped_time) + (latest_value * latest_value_elasped_time)) / total_elasped_time

                    let cumsum_elasped_time =
                        Uint64::from(updated_at.nanos()).checked_sub(started_at.nanos().into())?;

                    let latest_value_elasped_time =
                        Uint64::from(block_time.nanos()).checked_sub(updated_at.nanos().into())?;

                    let total_elasped_time =
                        Uint64::from(block_time.nanos()).checked_sub(started_at.nanos().into())?;

                    cumsum
                        .checked_add(latest_value.checked_mul(Decimal::checked_from_ratio(
                            latest_value_elasped_time,
                            1u128,
                        )?)?)?
                        .checked_div(Decimal::checked_from_ratio(total_elasped_time, 1u128)?)
                        .map_err(Into::into)
                }
                None => Err(StdError::not_found("division").into()),
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use cosmwasm_std::StdError;

        use super::*;

        #[test]
        fn test_new_compressed_division() {
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
            let block_time = Timestamp::from_nanos(100);
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
        fn test_average_single_elem_iter() {
            let started_at = Timestamp::from_nanos(100);
            let updated_at = Timestamp::from_nanos(110);
            let value = Decimal::percent(20);
            let prev_value = Decimal::percent(10);
            let compressed_division =
                CompressedDivision::new(started_at, updated_at, value, prev_value).unwrap();

            let divisions = vec![compressed_division];
            let division_size = Uint64::from(100u64);
            let window_size = Uint64::from(1000u64);
            let block_time = Timestamp::from_nanos(110);
            let average = CompressedDivision::average(
                divisions.clone().into_iter(),
                division_size,
                window_size,
                block_time,
            )
            .unwrap();

            assert_eq!(average, prev_value);

            let block_time = Timestamp::from_nanos(115);
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

            let block_time = Timestamp::from_nanos(150);
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
                    + (value * Decimal::from_ratio(40u128, 1u128)))
                    / Decimal::from_ratio(50u128, 1u128)
            );
        }
    }
}
