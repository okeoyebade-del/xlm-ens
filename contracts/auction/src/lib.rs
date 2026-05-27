mod test;

use soroban_sdk::{contract, contracterror, contractimpl, contracttype, Address, Env, String, Vec};
use xlm_ns_common::soroban::validate_fqdn_soroban;

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct Bid {
    pub bidder: Address,
    pub amount: u64,
    pub placed_at: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct Settlement {
    pub winner: Option<Address>,
    pub clearing_price: u64,
    pub winning_bid: u64,
    pub settled_at: u64,
    pub sold: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct Auction {
    pub name: String,
    pub reserve_price: u64,
    pub starts_at: u64,
    pub ends_at: u64,
    pub bids: Vec<Bid>,
}

#[derive(Clone)]
#[contracttype]
enum DataKey {
    Auction(String),
    Settlement(String),
    /// Append-only index of created auction names, enabling discovery queries
    /// (#157) without callers needing to know storage keys.
    AuctionNames,
}

/// Bounded result window for auction discovery queries (#157).
const MAX_AUCTION_RESULTS: u32 = 100;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum AuctionError {
    Validation = 1,
    AlreadyExists = 2,
    NotFound = 3,
    AuctionClosed = 4,
    AuctionNotStarted = 5,
    AuctionNotEnded = 6,
    AlreadySettled = 7,
    InvalidBid = 8,
}

#[contract]
pub struct AuctionContract;

#[contractimpl]
impl AuctionContract {
    pub fn create_auction(
        env: Env,
        name: String,
        reserve_price: u64,
        starts_at: u64,
        ends_at: u64,
    ) -> Result<(), AuctionError> {
        validate_fqdn_soroban(&name).map_err(|_| AuctionError::Validation)?;
        let key = DataKey::Auction(name.clone());
        if env.storage().persistent().has(&key) {
            return Err(AuctionError::AlreadyExists);
        }

        let auction = Auction {
            name: name.clone(),
            reserve_price,
            starts_at,
            ends_at,
            bids: Vec::new(&env),
        };
        env.storage().persistent().set(&key, &auction);

        // Record the name in the discovery index (#157).
        let mut names: Vec<String> = env
            .storage()
            .persistent()
            .get(&DataKey::AuctionNames)
            .unwrap_or_else(|| Vec::new(&env));
        names.push_back(name.clone());
        env.storage().persistent().set(&DataKey::AuctionNames, &names);
        Ok(())
    }

    /// Names of all auctions ever created, in creation order. Bounded to at most
    /// [`MAX_AUCTION_RESULTS`] entries (oldest first) so the call can't return an
    /// unbounded result set (#157).
    pub fn auction_names(env: Env) -> Vec<String> {
        let names: Vec<String> = env
            .storage()
            .persistent()
            .get(&DataKey::AuctionNames)
            .unwrap_or_else(|| Vec::new(&env));
        let mut out = Vec::new(&env);
        for name in names.iter().take(MAX_AUCTION_RESULTS as usize) {
            out.push_back(name);
        }
        out
    }

    /// Auctions that are currently open at `now_unix` — started, not yet ended,
    /// and not settled — in creation order, bounded to [`MAX_AUCTION_RESULTS`]
    /// (#157).
    pub fn active_auctions(env: Env, now_unix: u64) -> Vec<Auction> {
        let mut out = Vec::new(&env);
        for name in Self::auction_names(env.clone()).iter() {
            if env
                .storage()
                .persistent()
                .has(&DataKey::Settlement(name.clone()))
            {
                continue;
            }
            if let Some(auction) = Self::auction(env.clone(), name.clone()) {
                if now_unix >= auction.starts_at && now_unix <= auction.ends_at {
                    out.push_back(auction);
                }
            }
        }
        out
    }

    /// Auctions that have been settled, in creation order, bounded to
    /// [`MAX_AUCTION_RESULTS`] (#157).
    pub fn settled_auctions(env: Env) -> Vec<Auction> {
        let mut out = Vec::new(&env);
        for name in Self::auction_names(env.clone()).iter() {
            if env
                .storage()
                .persistent()
                .has(&DataKey::Settlement(name.clone()))
            {
                if let Some(auction) = Self::auction(env.clone(), name.clone()) {
                    out.push_back(auction);
                }
            }
        }
        out
    }

    pub fn place_bid(
        env: Env,
        name: String,
        bidder: Address,
        amount: u64,
        now_unix: u64,
    ) -> Result<(), AuctionError> {
        if amount == 0 {
            return Err(AuctionError::InvalidBid);
        }
        let mut auction = get_auction(&env, &name)?;
        if env
            .storage()
            .persistent()
            .has(&DataKey::Settlement(name.clone()))
        {
            return Err(AuctionError::AlreadySettled);
        }
        if now_unix < auction.starts_at {
            return Err(AuctionError::AuctionNotStarted);
        }
        if now_unix > auction.ends_at {
            return Err(AuctionError::AuctionClosed);
        }

        auction.bids.push_back(Bid {
            bidder,
            amount,
            placed_at: now_unix,
        });
        put_auction(&env, &name, &auction);
        Ok(())
    }

    pub fn settle(
        env: Env,
        name: String,
        now_unix: u64,
    ) -> Result<Option<Settlement>, AuctionError> {
        let auction = get_auction(&env, &name)?;
        if env
            .storage()
            .persistent()
            .has(&DataKey::Settlement(name.clone()))
        {
            return Err(AuctionError::AlreadySettled);
        }
        if now_unix < auction.ends_at {
            return Err(AuctionError::AuctionNotEnded);
        }

        let settlement = settle_vickrey(&auction, now_unix);
        if let Some(ref finalized) = settlement {
            env.storage()
                .persistent()
                .set(&DataKey::Settlement(name.clone()), finalized);
        }
        Ok(settlement)
    }

    pub fn auction(env: Env, name: String) -> Option<Auction> {
        env.storage().persistent().get(&DataKey::Auction(name))
    }
}

fn get_auction(env: &Env, name: &String) -> Result<Auction, AuctionError> {
    env.storage()
        .persistent()
        .get(&DataKey::Auction(name.clone()))
        .ok_or(AuctionError::NotFound)
}

fn put_auction(env: &Env, name: &String, auction: &Auction) {
    env.storage()
        .persistent()
        .set(&DataKey::Auction(name.clone()), auction);
}

fn settle_vickrey(auction: &Auction, settled_at: u64) -> Option<Settlement> {
    if auction.bids.is_empty() {
        return None;
    }

    let mut highest: Option<Bid> = None;
    let mut second_highest = 0u64;

    for bid in auction.bids.iter() {
        if highest
            .as_ref()
            .map(|current| bid.amount > current.amount)
            .unwrap_or(true)
        {
            second_highest = highest.as_ref().map(|current| current.amount).unwrap_or(0);
            highest = Some(bid);
        } else if bid.amount > second_highest {
            second_highest = bid.amount;
        }
    }

    let winning_bid = highest.as_ref()?.amount;
    if winning_bid < auction.reserve_price {
        return Some(Settlement {
            winner: None,
            clearing_price: 0,
            winning_bid,
            settled_at,
            sold: false,
        });
    }

    Some(Settlement {
        winner: highest.map(|bid| bid.bidder),
        clearing_price: if second_highest > auction.reserve_price {
            second_highest
        } else {
            auction.reserve_price
        },
        winning_bid,
        settled_at,
        sold: true,
    })
}
