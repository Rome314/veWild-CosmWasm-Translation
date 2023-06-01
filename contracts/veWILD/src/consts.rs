use cosmwasm_std::Uint64;

pub const SECONDS_PER_DAY: Uint64 = 24 * 60 * 60;
pub const MIN_LOCK_PERIOD: Uint64 = 7 * SECONDS_PER_DAY;
pub const MAX_LOCK_PERIOD: Uint64 = 1460 * SECONDS_PER_DAY;
pub const WITHDRAW_DELAY: Uint64 = 1 * SECONDS_PER_DAY;

pub const TOKEN_NAME: String = String::from("veWILD");
pub const TOKEN_SYMBOL: String = String::from("veWILD");
pub const TOKEN_DECIMALS: u8 = 18;