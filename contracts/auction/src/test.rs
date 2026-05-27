#[cfg(test)] ///////
mod tests {
    use soroban_sdk::{testutils::Address as _, Address, Env, String};

    use crate::{AuctionContract, AuctionContractClient};

    #[test]
    fn stores_auctions_in_contract_storage() {
        let env = Env::default();
        let contract_id = env.register(AuctionContract, ());
        let client = AuctionContractClient::new(&env, &contract_id);

        let alice = Address::generate(&env);
        let bob = Address::generate(&env);
        let name = String::from_str(&env, "vip.xlm");

        client.create_auction(&name, &200, &10, &20);
        client.place_bid(&name, &alice, &500, &12);
        client.place_bid(&name, &bob, &300, &13);

        let settlement = client.settle(&name, &21).unwrap();
        assert_eq!(settlement.winner, Some(alice));
        assert_eq!(settlement.clearing_price, 300);
        assert!(settlement.sold);
    } //

    #[test]
    fn test_auction_no_bids() {
        let env = Env::default();
        let contract_id = env.register(AuctionContract, ());
        let client = AuctionContractClient::new(&env, &contract_id);

        let name = String::from_str(&env, "ghost.xlm");
        client.create_auction(&name, &100, &10, &20);

        let settlement = client.settle(&name, &21);
        assert!(settlement.is_none());
    }

    #[test]
    fn test_auction_reserve_not_met() {
        let env = Env::default();
        let contract_id = env.register(AuctionContract, ());
        let client = AuctionContractClient::new(&env, &contract_id);

        let alice = Address::generate(&env);
        let name = String::from_str(&env, "cheap.xlm");
        client.create_auction(&name, &1000, &10, &20);
        client.place_bid(&name, &alice, &500, &15);

        let settlement = client.settle(&name, &21).unwrap();
        assert_eq!(settlement.winner, None);
        assert_eq!(settlement.clearing_price, 0);
        assert!(!settlement.sold);
    }

    #[test]
    fn test_auction_tie_behavior() {
        let env = Env::default();
        let contract_id = env.register(AuctionContract, ());
        let client = AuctionContractClient::new(&env, &contract_id);

        let alice = Address::generate(&env);
        let bob = Address::generate(&env);
        let name = String::from_str(&env, "tie.xlm");
        client.create_auction(&name, &100, &10, &20);

        client.place_bid(&name, &alice, &500, &12);
        client.place_bid(&name, &bob, &500, &13);

        let settlement = client.settle(&name, &21).unwrap();
        // First bidder wins in case of tie in current implementation
        assert_eq!(settlement.winner, Some(alice));
        assert_eq!(settlement.clearing_price, 500);
        assert!(settlement.sold);
    }

    #[test]
    fn test_auction_clearing_price_logic() {
        let env = Env::default();
        let contract_id = env.register(AuctionContract, ());
        let client = AuctionContractClient::new(&env, &contract_id);

        let alice = Address::generate(&env);
        let bob = Address::generate(&env);
        let charlie = Address::generate(&env);
        let name = String::from_str(&env, "multi.xlm");
        client.create_auction(&name, &100, &10, &20);

        client.place_bid(&name, &alice, &1000, &12);
        client.place_bid(&name, &bob, &500, &13);
        client.place_bid(&name, &charlie, &750, &14);

        let settlement = client.settle(&name, &21).unwrap();
        assert_eq!(settlement.winner, Some(alice));
        assert_eq!(settlement.clearing_price, 750); // Second highest bid
        assert!(settlement.sold);
    }

    // ── #157: auction discovery query helpers ──────────────────────────────

    #[test]
    fn discovery_queries_handle_empty_state() {
        let env = Env::default();
        let client = AuctionContractClient::new(&env, &env.register(AuctionContract, ()));
        assert_eq!(client.auction_names().len(), 0);
        assert_eq!(client.active_auctions(&100).len(), 0);
        assert_eq!(client.settled_auctions().len(), 0);
    }

    #[test]
    fn discovery_queries_filter_active_and_settled() {
        let env = Env::default();
        let alice = Address::generate(&env);
        let client = AuctionContractClient::new(&env, &env.register(AuctionContract, ()));

        let a = String::from_str(&env, "alpha.xlm");
        let b = String::from_str(&env, "bravo.xlm");
        let c = String::from_str(&env, "charlie.xlm");
        client.create_auction(&a, &100, &10, &20);
        client.create_auction(&b, &100, &10, &20);
        client.create_auction(&c, &100, &100, &200);

        // Index records every created auction, in creation order.
        let names = client.auction_names();
        assert_eq!(names.len(), 3);
        assert_eq!(names.get(0), Some(a.clone()));

        // At t=15: a and b are open; c hasn't started.
        let active = client.active_auctions(&15);
        assert_eq!(active.len(), 2);

        // Settle `a`, then it must move out of active and into settled.
        client.place_bid(&a, &alice, &500, &12);
        client.settle(&a, &21).unwrap();

        let active_after = client.active_auctions(&15);
        assert_eq!(active_after.len(), 1); // only b remains active
        assert_eq!(active_after.get(0).unwrap().name, b);

        let settled = client.settled_auctions();
        assert_eq!(settled.len(), 1);
        assert_eq!(settled.get(0).unwrap().name, a);
    }
}
