use cosmwasm_std::{
    CheckedFromRatioError, Coin, Decimal, DivideByZeroError, OverflowError, StdError, Timestamp,
    Uint128, Uint64,
};
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("Funds must be empty")]
    EmptyFundsExpected {},

    #[error("Funds must contain exactly one token")]
    SingleTokenExpected {},

    #[error("Funds must contain at least one token")]
    AtLeastSingleTokenExpected {},

    #[error("Denom has no supply, it might be an invalid denom: {denom}")]
    DenomHasNoSupply { denom: String },

    #[error("Unable to join pool with denom: {denom}: expected one of: {expected_denom:?}")]
    InvalidJoinPoolDenom {
        denom: String,
        expected_denom: Vec<String>,
    },

    #[error("Unable to transmute token with denom: {denom}: expected one of: {expected_denom:?} or alloyed asset")]
    InvalidTransmuteDenom {
        denom: String,
        expected_denom: Vec<String>,
    },

    #[error("Not a pool asset denom: {denom}")]
    InvalidPoolAssetDenom { denom: String },

    #[error("Pool asset denom count must be within {min} - {max} inclusive, but got: {actual}")]
    PoolAssetDenomCountOutOfRange {
        min: Uint64,
        max: Uint64,
        actual: Uint64,
    },

    #[error("Insufficient pool asset: required: {required}, available: {available}")]
    InsufficientPoolAsset { required: Coin, available: Coin },

    #[error("Insufficient shares: required: {required}, available: {available}")]
    InsufficientShares {
        required: Uint128,
        available: Uint128,
    },

    #[error("Invalid swap fee: expected: {expected}, actual: {actual}")]
    InvalidSwapFee { expected: Decimal, actual: Decimal },

    /// This error should never occur, but is here for completeness
    /// This will happens if and only if calculated token out and expected token out are not equal
    #[error("Invalid token out amount: expected: {expected}, actual: {actual}")]
    InvalidTokenOutAmount { expected: Uint128, actual: Uint128 },

    /// This error should never occur, but is here for completeness
    /// This will happens if and only if calculated token in and expected token in are not equal
    #[error("Invalid token in amount: expected: {expected}, actual: {actual}")]
    InvalidTokenInAmount { expected: Uint128, actual: Uint128 },

    #[error("Spot price query failed: reason {reason}")]
    SpotPriceQueryFailed { reason: String },

    #[error("Insufficient token out: required: {required}, available: {available}")]
    InsufficientTokenOut {
        required: Uint128,
        available: Uint128,
    },

    #[error("Excessive token in required: max acceptable token in: {limit}, required: {required}")]
    ExcessiveRequiredTokenIn { limit: Uint128, required: Uint128 },

    #[error("The pool is currently inactive")]
    InactivePool {},

    #[error("Unexpected denom: expected: {expected}, actual: {actual}")]
    UnexpectedDenom { expected: String, actual: String },

    #[error("Duplicated pool asset denom: {denom}")]
    DuplicatedPoolAssetDenom { denom: String },

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("Admin transferring state is inoperable for the requested operation")]
    InoperableAdminTransferringState {},

    #[error("Limiter count for {denom} exceed maximum per denom: {max}")]
    MaxLimiterCountPerDenomExceeded { denom: String, max: Uint64 },

    #[error("Denom: {denom} cannot have an empty limiter after it has been registered")]
    EmptyLimiterNotAllowed { denom: String },

    #[error("Limiter label must not be empty")]
    EmptyLimiterLabel {},

    #[error("Amount of coin to be operated on must be greater than zero")]
    ZeroValueOperation {},

    #[error("Window size must be greater than zero")]
    ZeroWindowSize {},

    #[error("Boundary must be greater than zero")]
    ZeroBoundaryOffset {},

    #[error("Upper limit must be greater than zero")]
    ZeroUpperLimit {},

    #[error("Upper limit must not exceed 100%")]
    ExceedHundredPercentUpperLimit {},

    #[error("Window must be evenly divisible by division size")]
    UnevenWindowDivision {},

    #[error("Division count must not exceed {max_division_count}")]
    DivisionCountExceeded { max_division_count: Uint64 },

    #[error("Moving average is undefined due to zero elapsed time since limiter started tracking")]
    UndefinedMovingAverage {},

    /// Time invariant error, this should never happen
    #[error("Time must be monotonically increasing")]
    NonMonotonicTime {},

    /// Time invariant error, this should never happen
    #[error("Division's update should occur before division ended: updated_at: {updated_at}, ended_at: {ended_at}")]
    UpdateAfterDivisionEnded {
        updated_at: Timestamp,
        ended_at: Timestamp,
    },

    #[error("Limiter does not exist for denom: {denom}, label: {label}")]
    LimiterDoesNotExist { denom: String, label: String },

    #[error("Limiter already exists for denom: {denom}, label: {label}")]
    LimiterAlreadyExists { denom: String, label: String },

    #[error(
        "Upper limit exceeded for `{denom}`, upper limit is {upper_limit}, but the resulted weight is {value}"
    )]
    UpperLimitExceeded {
        denom: String,
        upper_limit: Decimal,
        value: Decimal,
    },

    #[error("Modifying wrong limiter type: expected: {expected}, actual: {actual}")]
    WrongLimiterType { expected: String, actual: String },

    #[error("{0}")]
    OverflowError(#[from] OverflowError),

    #[error("{0}")]
    DivideByZeroError(#[from] DivideByZeroError),

    #[error("{0}")]
    CheckedFromRatioError(#[from] CheckedFromRatioError),
}
