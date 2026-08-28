fn scramble_bits(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9E37_79B9_7F4A_7C15);
    value = (value ^ (value >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    value ^ (value >> 31)
}

fn ordering_key(seed: u64, card_id: i64) -> u64 {
    scramble_bits(seed ^ scramble_bits(card_id as u64))
}

pub fn mock_order(card_ids: &[i64], seed: u64) -> Vec<i64> {
    let mut ordered = card_ids.to_vec();
    ordered.sort_unstable_by_key(|card_id| (ordering_key(seed, *card_id), *card_id));
    ordered
}

pub fn first_unanswered(card_ids: &[i64], seed: u64) -> Option<i64> {
    mock_order(card_ids, seed).first().copied()
}

#[cfg(test)]
mod tests {
    use super::{first_unanswered, mock_order, ordering_key};
    use std::collections::{HashMap, HashSet};

    fn pool_of(size: i64) -> Vec<i64> {
        (1..=size).collect()
    }

    #[test]
    fn the_same_pool_and_seed_always_produce_the_same_order() {
        let pool = pool_of(10);
        let expected = mock_order(&pool, 7);
        for _ in 0..100 {
            assert_eq!(mock_order(&pool, 7), expected);
        }
    }

    #[test]
    fn the_order_is_a_permutation_of_the_pool() {
        let pool = pool_of(25);
        let ordered = mock_order(&pool, 42);

        assert_eq!(ordered.len(), pool.len());
        let ordered_set: HashSet<i64> = ordered.iter().copied().collect();
        let pool_set: HashSet<i64> = pool.iter().copied().collect();
        assert_eq!(ordered_set, pool_set);
    }

    #[test]
    fn the_order_does_not_depend_on_the_arrival_order_of_the_pool() {
        let ascending = pool_of(12);
        let expected = mock_order(&ascending, 99);

        let mut descending = ascending.clone();
        descending.reverse();
        assert_eq!(mock_order(&descending, 99), expected);

        let mut rotated = ascending.clone();
        rotated.rotate_left(5);
        assert_eq!(mock_order(&rotated, 99), expected);

        let shuffled = vec![7, 1, 12, 3, 9, 2, 11, 5, 8, 4, 10, 6];
        assert_eq!(mock_order(&shuffled, 99), expected);
    }

    #[test]
    fn removing_a_card_preserves_the_relative_order_of_the_rest() {
        let pool = pool_of(20);
        let full_order = mock_order(&pool, 1234);

        for removed in &pool {
            let remaining: Vec<i64> =
                pool.iter().copied().filter(|card_id| card_id != removed).collect();
            let expected: Vec<i64> =
                full_order.iter().copied().filter(|card_id| card_id != removed).collect();
            assert_eq!(mock_order(&remaining, 1234), expected, "removing {removed}");
        }
    }

    #[test]
    fn adding_a_card_preserves_the_relative_order_of_the_rest() {
        let pool = pool_of(15);
        let before = mock_order(&pool, 555);

        let mut grown = pool.clone();
        grown.push(16);
        let after = mock_order(&grown, 555);

        let after_without_the_new_card: Vec<i64> =
            after.iter().copied().filter(|card_id| *card_id != 16).collect();
        assert_eq!(after_without_the_new_card, before);
    }

    #[test]
    fn consecutive_seeds_produce_different_orders() {
        let pool = pool_of(10);
        let orders: HashSet<Vec<i64>> = (1..=10).map(|seed| mock_order(&pool, seed)).collect();
        assert_eq!(orders.len(), 10, "consecutive seeds collided");
    }

    #[test]
    fn every_permutation_of_a_three_card_pool_appears_about_equally_often() {
        let pool = pool_of(3);
        let mut counts: HashMap<Vec<i64>, usize> = HashMap::new();
        for seed in 1..=6000 {
            *counts.entry(mock_order(&pool, seed)).or_insert(0) += 1;
        }

        assert_eq!(counts.len(), 6, "not every permutation appeared: {counts:?}");
        for (permutation, count) in &counts {
            assert!(
                (750..=1250).contains(count),
                "permutation {permutation:?} appeared {count} times, expected about 1000",
            );
        }
    }

    #[test]
    fn an_empty_pool_has_an_empty_order_and_no_first_card() {
        assert!(mock_order(&[], 1).is_empty());
        assert_eq!(first_unanswered(&[], 1), None);
    }

    #[test]
    fn a_single_card_pool_orders_and_serves_that_card() {
        assert_eq!(mock_order(&[42], 9), vec![42]);
        assert_eq!(first_unanswered(&[42], 9), Some(42));
    }

    #[test]
    fn first_unanswered_serves_the_head_of_the_order() {
        let pool = pool_of(8);
        let ordered = mock_order(&pool, 31);
        assert_eq!(first_unanswered(&pool, 31), Some(ordered[0]));
    }

    #[test]
    fn the_ordering_key_depends_on_both_the_seed_and_the_card() {
        assert_ne!(ordering_key(1, 1), ordering_key(2, 1));
        assert_ne!(ordering_key(1, 1), ordering_key(1, 2));
    }

    #[test]
    fn ordering_keys_do_not_collide_across_a_grid_of_seeds_and_cards() {
        let keys: HashSet<u64> = (1..=64u64)
            .flat_map(|seed| (1..=64i64).map(move |card_id| ordering_key(seed, card_id)))
            .collect();

        assert!(
            keys.len() > 4_000,
            "only {} distinct keys across a 4096-cell grid: sessions would share an order",
            keys.len(),
        );
    }

    #[test]
    fn a_negative_card_id_is_ordered_without_panicking() {
        let pool = vec![-3, -1, 0, 1, 3];
        let ordered = mock_order(&pool, 17);
        assert_eq!(ordered.len(), 5);
        let ordered_set: HashSet<i64> = ordered.iter().copied().collect();
        assert_eq!(ordered_set, pool.iter().copied().collect::<HashSet<i64>>());
    }
}
