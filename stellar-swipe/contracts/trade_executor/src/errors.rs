use soroban_sdk::contracterror;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum ContractError {
    NotInitialized = 1,
feature/position-limit-copy-trade
    PositionLimitReached = 2,

    InvalidAmount = 2,
    SlippageExceeded = 3,
main
}
